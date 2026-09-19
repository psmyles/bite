#[derive(Debug)]
pub struct UpdateInfo {
    pub available: bool,
    pub version: String,
    pub body: String,
    pub url: String,
}

fn version_parts(value: &str) -> Vec<u64> {
    value
        .trim_start_matches(['v', 'V'])
        .split('.')
        .map(|part| {
            part.chars()
                .take_while(char::is_ascii_digit)
                .collect::<String>()
                .parse()
                .unwrap_or(0)
        })
        .collect()
}

fn newer(latest: &str, current: &str) -> bool {
    let mut latest = version_parts(latest);
    let mut current = version_parts(current);
    let length = latest.len().max(current.len());
    latest.resize(length, 0);
    current.resize(length, 0);
    latest > current
}

fn current_version() -> String {
    serde_json::from_str::<serde_json::Value>(include_str!("../../../package.json"))
        .ok()
        .and_then(|package| package["version"].as_str().map(str::to_owned))
        .unwrap_or_else(|| env!("CARGO_PKG_VERSION").into())
}

#[cfg(windows)]
pub fn check() -> Result<UpdateInfo, String> {
    use windows::{
        core::{w, PCWSTR},
        Win32::Networking::WinHttp::{
            WinHttpCloseHandle, WinHttpConnect, WinHttpOpen, WinHttpOpenRequest,
            WinHttpQueryHeaders, WinHttpReadData, WinHttpReceiveResponse, WinHttpSendRequest,
            INTERNET_DEFAULT_HTTPS_PORT, WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY, WINHTTP_FLAG_SECURE,
            WINHTTP_QUERY_FLAG_NUMBER, WINHTTP_QUERY_STATUS_CODE,
        },
    };
    struct Handle(*mut std::ffi::c_void);
    impl Drop for Handle {
        fn drop(&mut self) {
            if !self.0.is_null() {
                unsafe {
                    let _ = WinHttpCloseHandle(self.0);
                }
            }
        }
    }
    unsafe {
        let session = Handle(WinHttpOpen(
            w!("bite-updater"),
            WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY,
            PCWSTR::null(),
            PCWSTR::null(),
            0,
        ));
        if session.0.is_null() {
            return Err("Could not create the update HTTP session".into());
        }
        let connection = Handle(WinHttpConnect(
            session.0,
            w!("api.github.com"),
            INTERNET_DEFAULT_HTTPS_PORT,
            0,
        ));
        if connection.0.is_null() {
            return Err("Could not connect to GitHub".into());
        }
        let request = Handle(WinHttpOpenRequest(
            connection.0,
            w!("GET"),
            w!("/repos/psmyles/bite/releases/latest"),
            PCWSTR::null(),
            PCWSTR::null(),
            std::ptr::null(),
            WINHTTP_FLAG_SECURE,
        ));
        if request.0.is_null() {
            return Err("Could not create the update request".into());
        }
        let headers: Vec<u16> =
            "User-Agent: bite-updater\r\nAccept: application/vnd.github+json\r\n"
                .encode_utf16()
                .collect();
        WinHttpSendRequest(request.0, Some(&headers), None, 0, 0, 0)
            .map_err(|error| error.to_string())?;
        WinHttpReceiveResponse(request.0, std::ptr::null_mut())
            .map_err(|error| error.to_string())?;
        let mut status = 0u32;
        let mut status_size = size_of::<u32>() as u32;
        let mut index = 0u32;
        WinHttpQueryHeaders(
            request.0,
            WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
            PCWSTR::null(),
            Some((&mut status as *mut u32).cast()),
            &mut status_size,
            &mut index,
        )
        .map_err(|error| error.to_string())?;
        if status != 200 {
            return Err(format!("GitHub returned HTTP {status}"));
        }
        let mut response = Vec::new();
        loop {
            let mut buffer = [0u8; 8192];
            let mut read = 0u32;
            WinHttpReadData(
                request.0,
                buffer.as_mut_ptr().cast(),
                buffer.len() as u32,
                &mut read,
            )
            .map_err(|error| error.to_string())?;
            if read == 0 {
                break;
            }
            response.extend_from_slice(&buffer[..read as usize]);
            if response.len() > 2 * 1024 * 1024 {
                return Err("GitHub update response was too large".into());
            }
        }
        let release: serde_json::Value =
            serde_json::from_slice(&response).map_err(|error| error.to_string())?;
        let version = release["tag_name"].as_str().unwrap_or_default().to_owned();
        if version.is_empty() {
            return Err("GitHub release did not include a version".into());
        }
        Ok(UpdateInfo {
            available: newer(&version, &current_version()),
            version,
            body: release["body"].as_str().unwrap_or_default().to_owned(),
            url: release["html_url"]
                .as_str()
                .unwrap_or("https://github.com/psmyles/bite/releases")
                .to_owned(),
        })
    }
}

#[cfg(not(windows))]
pub fn check() -> Result<UpdateInfo, String> {
    Err("Update checks are not implemented on this platform yet".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semantic_version_comparison_handles_tags_and_missing_parts() {
        assert!(newer("v1.2.0", "1.1.9"));
        assert!(!newer("v1.2", "1.2.0"));
        assert!(!newer("v1.2.0-beta", "1.2.0"));
        assert_eq!(current_version(), "0.5.0");
    }
}
