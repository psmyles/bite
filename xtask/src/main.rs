//! `cargo xtask <command>`: the project's automation.
//!
//! * `icons`: redraw `build/icon.ico` and the window icon from `build/icons/icon.png`.
//! * `package-windows`: the Inno Setup installer in `dist/`.
//!
//! macOS packaging stays in `packaging/build-mac-*.sh`: its icon is compiled from the Icon
//! Composer document (`build/icons/bite.icon`), which needs Xcode.

mod icons;
mod package;
mod util;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some((cmd, rest)) = args.split_first() else {
        usage();
        std::process::exit(2);
    };
    let result = match cmd.as_str() {
        "icons" => icons::run_icons(rest),
        "package-windows" => package::run_windows(rest),
        "help" | "--help" | "-h" => {
            usage();
            Ok(())
        }
        other => Err(format!("unknown command `{other}`")),
    };
    if let Err(e) = result {
        eprintln!("xtask: {e}");
        std::process::exit(1);
    }
}

fn usage() {
    eprintln!(
        "usage: cargo xtask <command>

  icons [--force] [--preview DIR]
                                redraw build/icon.ico and crates/bite-gui/assets/icon-256.png
                                from build/icons/icon.png if it changed; --preview also writes
                                Setup's wizard images to DIR
  package-windows [--no-build]  sync the crate version to product.json, redraw the icons, build
                                the release binaries and compile packaging/bite.iss into
                                dist/Bite-Windows-<version>-Setup.exe (--no-build reuses
                                target/release and the icons as they are)"
    );
}
