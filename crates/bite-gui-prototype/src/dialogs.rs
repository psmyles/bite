use std::path::{Path, PathBuf};

#[cfg(windows)]
fn selected_path(item: &windows::Win32::UI::Shell::IShellItem) -> Result<PathBuf, String> {
    use windows::Win32::{System::Com::CoTaskMemFree, UI::Shell::SIGDN_FILESYSPATH};
    unsafe {
        let value = item
            .GetDisplayName(SIGDN_FILESYSPATH)
            .map_err(|error| error.to_string())?;
        let text = value.to_string().map_err(|error| error.to_string())?;
        CoTaskMemFree(Some(value.as_ptr().cast()));
        Ok(PathBuf::from(text))
    }
}

#[cfg(windows)]
fn initialize_com() -> Result<(), String> {
    use windows::Win32::System::Com::{CoInitializeEx, COINIT_APARTMENTTHREADED};
    static RESULT: std::sync::OnceLock<Result<(), String>> = std::sync::OnceLock::new();
    RESULT
        .get_or_init(|| unsafe {
            match CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok() {
                Ok(()) => Ok(()),
                Err(error) if error.code().0 == 0x80010106u32 as i32 => Ok(()),
                Err(error) => Err(error.to_string()),
            }
        })
        .clone()
}

#[cfg(windows)]
pub fn open_workflow() -> Result<Option<PathBuf>, String> {
    use windows::{
        core::HSTRING,
        Win32::{
            System::Com::{CoCreateInstance, CLSCTX_INPROC_SERVER},
            UI::Shell::{Common::COMDLG_FILTERSPEC, FileOpenDialog, IFileOpenDialog},
        },
    };
    initialize_com()?;
    unsafe {
        let dialog: IFileOpenDialog = CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER)
            .map_err(|error| error.to_string())?;
        let name = HSTRING::from("BITE workflows");
        let pattern = HSTRING::from("*.bite");
        dialog
            .SetFileTypes(&[COMDLG_FILTERSPEC {
                pszName: windows::core::PCWSTR(name.as_ptr()),
                pszSpec: windows::core::PCWSTR(pattern.as_ptr()),
            }])
            .map_err(|error| error.to_string())?;
        match dialog.Show(None) {
            Ok(()) => dialog
                .GetResult()
                .map_err(|error| error.to_string())
                .and_then(|item| selected_path(&item))
                .map(Some),
            Err(error) if error.code().0 == 0x800704c7u32 as i32 => Ok(None),
            Err(error) => Err(error.to_string()),
        }
    }
}

#[cfg(windows)]
pub fn save_file(
    default: &Path,
    extension: &str,
    description: &str,
) -> Result<Option<PathBuf>, String> {
    use windows::{
        core::HSTRING,
        Win32::{
            System::Com::{CoCreateInstance, CLSCTX_INPROC_SERVER},
            UI::Shell::{Common::COMDLG_FILTERSPEC, FileSaveDialog, IFileSaveDialog},
        },
    };
    initialize_com()?;
    unsafe {
        let dialog: IFileSaveDialog = CoCreateInstance(&FileSaveDialog, None, CLSCTX_INPROC_SERVER)
            .map_err(|error| error.to_string())?;
        let name = HSTRING::from(description);
        let pattern = HSTRING::from(format!("*.{extension}"));
        dialog
            .SetFileTypes(&[COMDLG_FILTERSPEC {
                pszName: windows::core::PCWSTR(name.as_ptr()),
                pszSpec: windows::core::PCWSTR(pattern.as_ptr()),
            }])
            .map_err(|error| error.to_string())?;
        let extension = HSTRING::from(extension);
        dialog
            .SetDefaultExtension(&extension)
            .map_err(|error| error.to_string())?;
        if let Some(file_name) = default.file_name().and_then(|name| name.to_str()) {
            dialog
                .SetFileName(&HSTRING::from(file_name))
                .map_err(|error| error.to_string())?;
        }
        match dialog.Show(None) {
            Ok(()) => dialog
                .GetResult()
                .map_err(|error| error.to_string())
                .and_then(|item| selected_path(&item))
                .map(Some),
            Err(error) if error.code().0 == 0x800704c7u32 as i32 => Ok(None),
            Err(error) => Err(error.to_string()),
        }
    }
}

#[cfg(windows)]
pub fn select_folder() -> Result<Option<PathBuf>, String> {
    use windows::Win32::{
        System::Com::{CoCreateInstance, CLSCTX_INPROC_SERVER},
        UI::Shell::{FileOpenDialog, IFileOpenDialog, FOS_PICKFOLDERS},
    };
    initialize_com()?;
    unsafe {
        let dialog: IFileOpenDialog = CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER)
            .map_err(|error| error.to_string())?;
        let options = dialog.GetOptions().map_err(|error| error.to_string())?;
        dialog
            .SetOptions(options | FOS_PICKFOLDERS)
            .map_err(|error| error.to_string())?;
        match dialog.Show(None) {
            Ok(()) => dialog
                .GetResult()
                .map_err(|error| error.to_string())
                .and_then(|item| selected_path(&item))
                .map(Some),
            Err(error) if error.code().0 == 0x800704c7u32 as i32 => Ok(None),
            Err(error) => Err(error.to_string()),
        }
    }
}

#[cfg(windows)]
pub fn write_clipboard(text: &str) -> Result<(), String> {
    use windows::Win32::{
        Foundation::HANDLE,
        System::{
            DataExchange::{CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData},
            Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE},
        },
    };
    let mut wide: Vec<u16> = text.encode_utf16().collect();
    wide.push(0);
    unsafe {
        OpenClipboard(None).map_err(|error| error.to_string())?;
        let result = (|| {
            EmptyClipboard().map_err(|error| error.to_string())?;
            let memory = GlobalAlloc(GMEM_MOVEABLE, wide.len() * size_of::<u16>())
                .map_err(|error| error.to_string())?;
            let target = GlobalLock(memory).cast::<u16>();
            if target.is_null() {
                return Err("Could not lock clipboard memory".into());
            }
            std::ptr::copy_nonoverlapping(wide.as_ptr(), target, wide.len());
            let _ = GlobalUnlock(memory);
            SetClipboardData(13, Some(HANDLE(memory.0))).map_err(|error| error.to_string())?;
            Ok(())
        })();
        let _ = CloseClipboard();
        result
    }
}

#[cfg(windows)]
pub fn read_clipboard() -> Result<Option<String>, String> {
    use windows::Win32::{
        Foundation::HGLOBAL,
        System::{
            DataExchange::{CloseClipboard, GetClipboardData, OpenClipboard},
            Memory::{GlobalLock, GlobalSize, GlobalUnlock},
        },
    };
    unsafe {
        OpenClipboard(None).map_err(|error| error.to_string())?;
        let result = (|| {
            let handle = match GetClipboardData(13) {
                Ok(handle) => handle,
                Err(_) => return Ok(None),
            };
            let memory = HGLOBAL(handle.0);
            let pointer = GlobalLock(memory).cast::<u16>();
            if pointer.is_null() {
                return Ok(None);
            }
            let capacity = GlobalSize(memory) / size_of::<u16>();
            let values = std::slice::from_raw_parts(pointer, capacity);
            let length = values
                .iter()
                .position(|value| *value == 0)
                .unwrap_or(capacity);
            let text = String::from_utf16(&values[..length]).map_err(|error| error.to_string())?;
            let _ = GlobalUnlock(memory);
            Ok(Some(text))
        })();
        let _ = CloseClipboard();
        result
    }
}

#[cfg(not(windows))]
pub fn open_workflow() -> Result<Option<PathBuf>, String> {
    Err("Native file dialogs are not implemented on this platform yet".into())
}

#[cfg(not(windows))]
pub fn save_file(
    _default: &Path,
    _extension: &str,
    _description: &str,
) -> Result<Option<PathBuf>, String> {
    Err("Native file dialogs are not implemented on this platform yet".into())
}

#[cfg(not(windows))]
pub fn select_folder() -> Result<Option<PathBuf>, String> {
    Err("Native file dialogs are not implemented on this platform yet".into())
}

#[cfg(not(windows))]
pub fn write_clipboard(_text: &str) -> Result<(), String> {
    Ok(())
}

#[cfg(not(windows))]
pub fn read_clipboard() -> Result<Option<String>, String> {
    Ok(None)
}
