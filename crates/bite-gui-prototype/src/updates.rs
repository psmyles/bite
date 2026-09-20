//! The release check behind Help, Check for Updates.
//!
//! The Electron build queries the GitHub releases endpoint at startup and reports only when
//! a newer release exists; a manual check also reports the up to date and failed states.

/// One release, as the update dialog presents it.
#[derive(Debug, Clone, Default)]
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

/// The version this build reports, taken from the shared package manifest.
pub fn current_version() -> String {
    serde_json::from_str::<serde_json::Value>(include_str!("../../../package.json"))
        .ok()
        .and_then(|package| package["version"].as_str().map(str::to_owned))
        .unwrap_or_else(|| env!("CARGO_PKG_VERSION").into())
}

use std::io::Read;

/// The releases endpoint the Electron main process uses.
const RELEASES_URL: &str = "https://api.github.com/repos/psmyles/bite/releases/latest";
/// The response cap the Electron check applies, so a malformed reply cannot exhaust memory.
const MAX_RESPONSE_BYTES: usize = 2 * 1024 * 1024;

/// Fetches the latest release and compares it with this build.
pub fn check() -> Result<UpdateInfo, String> {
    let response = ureq::get(RELEASES_URL)
        .set("User-Agent", "BITE")
        .set("Accept", "application/vnd.github+json")
        .timeout(std::time::Duration::from_secs(15))
        .call()
        .map_err(|error| error.to_string())?;
    let mut body = String::new();
    response
        .into_reader()
        .take(MAX_RESPONSE_BYTES as u64)
        .read_to_string(&mut body)
        .map_err(|error| error.to_string())?;
    parse(&body, &current_version())
}

/// Turns a release document into the state the dialog shows.
pub fn parse(body: &str, current: &str) -> Result<UpdateInfo, String> {
    let release: serde_json::Value =
        serde_json::from_str(body).map_err(|error| error.to_string())?;
    let tag = release["tag_name"].as_str().unwrap_or_default();
    if tag.is_empty() {
        return Err("The release response had no version tag".into());
    }
    Ok(UpdateInfo {
        available: newer(tag, current),
        version: tag.trim_start_matches(['v', 'V']).to_owned(),
        body: release["body"].as_str().unwrap_or_default().to_owned(),
        url: release["html_url"]
            .as_str()
            .unwrap_or("https://github.com/psmyles/bite/releases")
            .to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semantic_version_comparison_handles_tags_and_missing_parts() {
        assert!(newer("v1.2.0", "1.1.9"));
        assert!(!newer("v1.2", "1.2.0"));
        assert!(!newer("v1.2.0-beta", "1.2.0"));
        assert!(newer("2.0.0", "1.9.9"));
    }

    #[test]
    fn a_newer_tag_is_reported_as_available() {
        let body = r#"{"tag_name":"v9.9.9","body":"notes","html_url":"https://example.invalid/r"}"#;
        let info = parse(body, "0.5.0").unwrap();
        assert!(info.available);
        assert_eq!(info.version, "9.9.9");
        assert_eq!(info.body, "notes");
        assert_eq!(info.url, "https://example.invalid/r");
    }

    #[test]
    fn the_same_tag_is_reported_as_up_to_date() {
        let body = r#"{"tag_name":"v0.5.0","body":"","html_url":"https://example.invalid/r"}"#;
        let info = parse(body, "0.5.0").unwrap();
        assert!(!info.available);
        assert_eq!(info.version, "0.5.0");
    }

    #[test]
    fn a_response_without_a_tag_is_an_error() {
        assert!(parse(r#"{"body":"x"}"#, "0.5.0").is_err());
        assert!(parse("not json", "0.5.0").is_err());
    }

    #[test]
    fn the_reported_version_comes_from_the_shared_manifest() {
        assert!(!current_version().is_empty());
    }
}
