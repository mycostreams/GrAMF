//! User settings, kept in `~/.gramf/settings` as `key = value` lines (hand-editable).

use std::path::PathBuf;

#[derive(Clone, PartialEq, Debug)]
pub struct Settings {
    /// Names the user's annotation file, `annotations.<author>.h5`.
    pub author: String,
    /// The lab plate registry (`AMF-PlateList`); `GRAMF_REGISTRY` overrides it.
    pub registry_dir: PathBuf,
    /// Video runs further apart than this start a new registration session (DESIGN.md §10).
    pub session_gap_minutes: f32,
}

impl Default for Settings {
    fn default() -> Self {
        let user = std::env::var("USER").or_else(|_| std::env::var("USERNAME")).unwrap_or_default();
        let author = crate::annotations::sanitize_author(&user);
        Self {
            author: if author.is_empty() { "anonymous".into() } else { author },
            registry_dir: home().map_or_else(PathBuf::new, |h| h.join("AMOLF-SHIMIZU Dropbox/AMF-PlateList")),
            session_gap_minutes: 15.0,
        }
    }
}

fn home() -> Option<PathBuf> {
    std::env::home_dir()
}

fn dir() -> Option<PathBuf> {
    home().map(|h| h.join(".gramf"))
}

impl Settings {
    /// Settings from disk, falling back to defaults per key. Reads the older `~/.gramf/author`
    /// file when `settings` has no author yet.
    pub fn load() -> Self {
        let dir = dir();
        let text = dir.as_ref().and_then(|d| std::fs::read_to_string(d.join("settings")).ok());
        let mut s = Self::parse(text.as_deref().unwrap_or(""));
        if !text.as_deref().unwrap_or("").contains("author") {
            if let Some(old) = dir.and_then(|d| std::fs::read_to_string(d.join("author")).ok()) {
                let old = crate::annotations::sanitize_author(old.trim());
                if !old.is_empty() {
                    s.author = old;
                }
            }
        }
        s
    }

    pub fn save(&self) -> Result<(), String> {
        let dir = dir().ok_or("no home folder")?;
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        std::fs::write(dir.join("settings"), self.to_text()).map_err(|e| e.to_string())
    }

    /// The registry folder actually used: `GRAMF_REGISTRY` when set, else the setting.
    pub fn registry(&self) -> PathBuf {
        std::env::var_os("GRAMF_REGISTRY").map_or_else(|| self.registry_dir.clone(), PathBuf::from)
    }

    fn parse(text: &str) -> Self {
        let mut s = Self::default();
        for line in text.lines() {
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let value = value.trim();
            match key.trim() {
                "author" => {
                    let a = crate::annotations::sanitize_author(value);
                    if !a.is_empty() {
                        s.author = a;
                    }
                }
                "registry_dir" if !value.is_empty() => s.registry_dir = PathBuf::from(value),
                "session_gap_minutes" => {
                    if let Ok(m) = value.parse::<f32>() {
                        s.session_gap_minutes = m.max(0.0);
                    }
                }
                _ => (),
            }
        }
        s
    }

    fn to_text(&self) -> String {
        format!(
            "# GrAMF settings (also editable in the app: Settings)\nauthor = {}\nregistry_dir = {}\nsession_gap_minutes = {}\n",
            self.author,
            self.registry_dir.display(),
            self.session_gap_minutes
        )
    }
}

#[cfg(test)]
mod tests {
    use super::Settings;

    #[test]
    fn round_trip_and_bad_lines() {
        let s = Settings {
            author: "anna".into(),
            registry_dir: "/x/AMF-PlateList".into(),
            session_gap_minutes: 30.0,
        };
        assert_eq!(Settings::parse(&s.to_text()), s);
        let t = Settings::parse("author = Ann a./\nsession_gap_minutes = soon\nnonsense\n");
        assert_eq!(t.author, "Anna");
        assert_eq!(t.session_gap_minutes, 15.0);
    }
}
