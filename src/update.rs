//! Startup check for a newer GitHub release. Runs on a background thread, never installs
//! anything, and stays silent on any failure (offline, rate-limited, no releases yet).

use std::sync::mpsc::{Receiver, channel};

pub const RELEASES_URL: &str = "https://github.com/mycostreams/GrAMF/releases/latest";
const API_URL: &str = "https://api.github.com/repos/mycostreams/GrAMF/releases/latest";

/// Sends the latest release tag if it is newer than this build. `GRAMF_NO_UPDATE_CHECK`
/// turns the check off.
pub fn spawn_check() -> Receiver<String> {
    let (tx, rx) = channel();
    if std::env::var_os("GRAMF_NO_UPDATE_CHECK").is_none() {
        std::thread::spawn(move || {
            let body = ureq::get(API_URL)
                .header("User-Agent", "GrAMF")
                .header("Accept", "application/vnd.github+json")
                .call()
                .ok()
                .and_then(|mut r| r.body_mut().read_to_string().ok());
            if let Some(tag) = body.as_deref().and_then(tag_name) {
                if is_newer(&tag, env!("CARGO_PKG_VERSION")) {
                    let _ = tx.send(tag);
                }
            }
        });
    }
    rx
}

/// `"tag_name": "v0.2.0"` from the API response, without a JSON dependency.
fn tag_name(json: &str) -> Option<String> {
    let rest = &json[json.find("\"tag_name\"")? + "\"tag_name\"".len()..];
    let rest = &rest[rest.find('"')? + 1..];
    Some(rest[..rest.find('"')?].to_string())
}

fn parse(v: &str) -> Option<(u64, u64, u64)> {
    let mut parts = v.trim_start_matches('v').split('.').map(|p| p.parse::<u64>().ok());
    Some((parts.next()??, parts.next()??, parts.next().flatten().unwrap_or(0)))
}

fn is_newer(tag: &str, current: &str) -> bool {
    matches!((parse(tag), parse(current)), (Some(a), Some(b)) if a > b)
}

#[cfg(test)]
mod tests {
    use super::{is_newer, tag_name};

    #[test]
    fn versions() {
        assert_eq!(tag_name(r#"{"url":"x","tag_name": "v0.2.0","name":"y"}"#).as_deref(), Some("v0.2.0"));
        assert_eq!(tag_name("{}"), None);
        assert!(is_newer("v0.2.0", "0.1.0"));
        assert!(is_newer("v0.10.0", "0.9.3"));
        assert!(!is_newer("v0.1.0", "0.1.0"));
        assert!(!is_newer("nightly", "0.1.0"));
    }
}
