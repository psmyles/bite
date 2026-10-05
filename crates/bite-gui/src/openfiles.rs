//! Finder opens on macOS: `application:openURLs:`.
//!
//! On Windows a double-clicked `.bite` file arrives as `argv[1]`, which is what `main` reads.
//! macOS does not work that way. Finder, `open(1)` and a drop on the Dock icon all go through
//! Launch Services, which delivers the file as an **Apple event** to the app: a fresh launch gets
//! no arguments at all, and an app that is already running gets no new process, so nothing on the
//! command line ever sees it. Without this hook the bundle opens blank from Finder and ignores
//! every later open, however the file association in `Info.plist` is written.
//!
//! ## Why the delegate method is added at run time
//!
//! AppKit turns that Apple event into `application:openURLs:` on the `NSApplicationDelegate` -
//! but winit owns the delegate, and implements neither that method nor a way to extend it. The
//! three ways out are: replace the delegate (winit's own code assumes the app's delegate is its
//! class, so this breaks on the next event), register an `NSAppleEventManager` handler
//! (`NSApplication` installs its own during `finishLaunching`, i.e. after anything reachable from
//! `run`, so ours would be replaced), or add the one missing method to winit's delegate class.
//! The last is the only one that leaves winit's dispatch intact, so that is what this does -
//! `class_addMethod` on the class the live delegate belongs to, adding a selector winit does not
//! define.
//!
//! The delegate is then re-set on `NSApplication`. `setDelegate:` caches which delegate methods
//! exist at the moment it is called, and winit calls it before this runs; without the re-set
//! AppKit would go on believing the delegate cannot open files. It is the same object, which
//! winit keeps alive for the life of the event loop (`NSApplication` holds it weakly).
//!
//! ## Ordering
//!
//! A launch-by-open delivers the event *between* `applicationWillFinishLaunching:` and
//! `applicationDidFinishLaunching:` - that is, before winit reports `resumed` and therefore
//! before the window exists. Those opens are held in [`PENDING`] and handed over by [`start`],
//! so the editor comes up showing the workflow instead of coming up blank and loading it a frame
//! later. Everything after `start` goes to the event loop as [`AppEvent::Open`], which is the
//! same path a file dropped on the window takes.
//!
//! [`PENDING`]: HOOK

use std::cell::RefCell;
use std::path::PathBuf;

use objc2::rc::Retained;
use objc2::runtime::{AnyClass, AnyObject, Bool, Sel};
use objc2::{ffi, sel};
use objc2_app_kit::NSApplication;
use objc2_foundation::{MainThreadMarker, NSArray, NSURL};
use winit::event_loop::EventLoopProxy;

use crate::platform::AppEvent;

/// Where an open goes.
///
/// Main-thread only by construction: `application:openURLs:` is delivered on the main thread and
/// so is every reader below, which is why a thread-local suffices and no lock is involved.
struct Hook {
    /// `None` until [`install`], and after it the loop's proxy. Sending is how an open reaches
    /// the editor: the event loop is the only place its state may be touched.
    proxy: Option<EventLoopProxy<AppEvent>>,
    /// Opens that arrived before there was a window to put them in. Drained by [`start`].
    pending: Vec<PathBuf>,
    /// Whether the window exists yet - that is, whether `pending` is still the right answer.
    live: bool,
}

thread_local! {
    static HOOK: RefCell<Hook> = const {
        RefCell::new(Hook { proxy: None, pending: Vec::new(), live: false })
    };
}

/// Adds `application:openURLs:` to winit's application delegate so Launch Services opens reach
/// the editor.
///
/// Returns `false` if the method could not be added, which leaves the editor working exactly as
/// it did before: openable from the command line, deaf to Finder.
///
/// Call after the event loop is built - the delegate has to exist - and before it runs.
pub fn install(proxy: EventLoopProxy<AppEvent>) -> bool {
    HOOK.with(|hook| hook.borrow_mut().proxy = Some(proxy));

    let Some(mtm) = MainThreadMarker::new() else {
        return false;
    };
    let app = NSApplication::sharedApplication(mtm);
    // SAFETY: reading the delegate on the main thread, which is the only thread that sets it.
    let Some(delegate) = (unsafe { app.delegate() }) else {
        return false;
    };

    // The class of the delegate winit actually installed, rather than one looked up by name: it
    // is winit's private type either way, but read from the live object a rename upstream cannot
    // silently turn the hook off. `ProtocolObject` is `AnyObject` underneath, which is what makes
    // the cast sound.
    let object: &AnyObject = unsafe { &*(Retained::as_ptr(&delegate) as *const AnyObject) };
    let class: &AnyClass = object.class();

    // `v@:@@` - returns void, takes (self, _cmd, NSApplication*, NSArray*). `class_addMethod`
    // refuses only if the class already has the method, which would mean winit had grown its own
    // `application:openURLs:` - and then winit's is the one to keep, not ours.
    //
    // SAFETY: the function's signature matches the encoding, and both match what AppKit sends for
    // this selector. The transmute is the required cast to the type-erased `IMP`; the pointers
    // are otherwise identical.
    let added = Bool::from_raw(unsafe {
        let imp: unsafe extern "C" fn() = std::mem::transmute(
            open_urls as extern "C" fn(&AnyObject, Sel, &AnyObject, &NSArray<NSURL>),
        );
        ffi::class_addMethod(
            class as *const AnyClass as *mut ffi::objc_class,
            sel!(application:openURLs:).as_ptr(),
            Some(imp),
            c"v@:@@".as_ptr(),
        )
    });
    if !added.as_bool() {
        return false;
    }

    // Re-set the same delegate so AppKit re-reads which methods it has; see the module comment.
    app.setDelegate(Some(&delegate));
    true
}

/// The window is being created: take the opens that arrived before it existed, and route
/// everything after this through the event loop.
pub fn start() -> Vec<PathBuf> {
    HOOK.with(|hook| {
        let mut hook = hook.borrow_mut();
        hook.live = true;
        std::mem::take(&mut hook.pending)
    })
}

/// `application:openURLs:`. Runs on the main thread, called by AppKit.
///
/// Anything that is not a `file:` URL is dropped: the editor registers no URL scheme, so such a
/// URL is not its to open, and `path` is `None` for exactly those.
extern "C" fn open_urls(_this: &AnyObject, _cmd: Sel, _app: &AnyObject, urls: &NSArray<NSURL>) {
    for url in urls.iter() {
        // SAFETY: `path` reads an immutable property of an `NSURL` AppKit has just handed over.
        let Some(path) = (unsafe { url.path() }) else {
            continue;
        };
        deliver(PathBuf::from(path.to_string()));
    }
}

/// Queues an open for the window, or sends it to the event loop if the window exists.
///
/// Only workflows: the file association declares `.bite` and nothing else, so anything else here
/// was not opened with the editor on purpose, and `commands::open_path` would refuse it anyway.
fn deliver(path: PathBuf) {
    if path.extension().and_then(|extension| extension.to_str()) != Some("bite") {
        return;
    }
    // The proxy is taken out of the `RefCell` before it is used: sending wakes the run loop, and
    // a borrow still held across that is an "already borrowed" panic waiting for the first time
    // AppKit re-enters from inside the wake.
    let proxy = HOOK.with(|hook| {
        let mut hook = hook.borrow_mut();
        match (hook.live, &hook.proxy) {
            (true, Some(proxy)) => Some(proxy.clone()),
            _ => {
                hook.pending.push(path.clone());
                None
            }
        }
    });
    if let Some(proxy) = proxy {
        // A closed event loop means the editor is already leaving; a dropped open is right.
        let _ = proxy.send_event(AppEvent::Open(path));
    }
}
