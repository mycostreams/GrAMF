//! User settings, kept in `~/.gramf/settings` as `key = value` lines (hand-editable).

use crate::placement::Similarity;
use std::collections::BTreeMap;
use std::path::PathBuf;

/// A setup's default stage-to-STG transform and camera orientation (DESIGN.md §10).
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct SetupDefault {
    pub transform: Similarity,
    /// `placement::camera_orient` option, 0-7.
    pub camera: u8,
}

/// ARETHA's built-in default, fitted from FLW2605A003's six registrator anchors (residuals
/// 1-12 µm; camera option 3 scored 6 µm against >= 19 µm for the others). Replaced by
/// "Save as ARETHA default" in the registration workspace.
const ARETHA_SEED: SetupDefault = SetupDefault {
    transform: Similarity { scale: 1008.137, theta: 0.015065, mirror: true, t: [41650.42, 42469.49] },
    camera: 3,
};

#[derive(Clone, PartialEq, Debug)]
pub struct Settings {
    /// Names the user's annotation file, `annotations.<author>.h5`.
    pub author: String,
    /// The lab plate registry (`AMF-PlateList`); `GRAMF_REGISTRY` overrides it.
    pub registry_dir: PathBuf,
    /// Video runs further apart than this start a new registration session (DESIGN.md §10).
    pub session_gap_minutes: f32,
    /// Per setup name (`Setup::name`): default transform and camera orientation.
    pub setup_defaults: BTreeMap<String, SetupDefault>,
}

impl Default for Settings {
    fn default() -> Self {
        let user = std::env::var("USER").or_else(|_| std::env::var("USERNAME")).unwrap_or_default();
        let author = crate::annotations::sanitize_author(&user);
        Self {
            author: if author.is_empty() { "anonymous".into() } else { author },
            registry_dir: home().map_or_else(PathBuf::new, |h| h.join("AMOLF-SHIMIZU Dropbox/AMF-PlateList")),
            session_gap_minutes: 15.0,
            setup_defaults: BTreeMap::from([("ARETHA".to_string(), ARETHA_SEED)]),
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
                k if k.starts_with("setup.") => {
                    let f: Vec<f64> = value.split(',').filter_map(|x| x.trim().parse().ok()).collect();
                    if let [scale, theta, mirror, tx, ty, camera] = f[..] {
                        let transform = Similarity { scale, theta, mirror: mirror != 0.0, t: [tx, ty] };
                        let camera = (camera as u8).min(7);
                        s.setup_defaults.insert(k["setup.".len()..].to_string(), SetupDefault { transform, camera });
                    }
                }
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
        let mut text = format!(
            "# GrAMF settings (also editable in the app: Settings)\nauthor = {}\nregistry_dir = {}\nsession_gap_minutes = {}\n",
            self.author,
            self.registry_dir.display(),
            self.session_gap_minutes
        );
        text += "# setup.<name> = scale (um/mm), theta (rad), mirror (0/1), tx, ty (um), camera (0-7)\n";
        for (name, d) in &self.setup_defaults {
            let t = d.transform;
            text += &format!(
                "setup.{name} = {}, {}, {}, {}, {}, {}\n",
                t.scale, t.theta, t.mirror as u8, t.t[0], t.t[1], d.camera
            );
        }
        text
    }

    /// The default for `setup`, if any.
    pub fn setup_default(&self, setup: &str) -> Option<SetupDefault> {
        self.setup_defaults.get(setup).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::{Settings, Similarity};

    #[test]
    fn round_trip_and_bad_lines() {
        let mut s = Settings {
            author: "anna".into(),
            registry_dir: "/x/AMF-PlateList".into(),
            session_gap_minutes: 30.0,
            ..Settings::default()
        };
        s.setup_defaults.insert(
            "MORRISON".into(),
            super::SetupDefault { transform: Similarity { scale: 995.5, theta: -0.25, mirror: false, t: [1.5, -2.0] }, camera: 6 },
        );
        assert_eq!(Settings::parse(&s.to_text()), s);
        let t = Settings::parse("author = Ann a./\nsession_gap_minutes = soon\nnonsense\n");
        assert_eq!(t.author, "Anna");
        assert_eq!(t.session_gap_minutes, 15.0);
        assert!(t.setup_default("ARETHA").is_some(), "built-in seed survives a file without it");
    }
}
