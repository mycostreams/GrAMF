//! Plate lookup in the lab registry: one `.xlsx` per experiment under
//! `AMF-PlateList/{NOW,OLD}_Experiments` (DESIGN.md §4, "Registry join").

use calamine::{Data, Reader};
use std::path::{Path, PathBuf};

pub struct PlateRecord {
    pub ui: String,
    /// Epoch seconds (midnight UTC of the registry date).
    pub inoculation: Option<i64>,
    pub crossed: Option<i64>,
    pub source: PathBuf,
}

/// `GRAMF_REGISTRY`, else the default Dropbox location.
pub fn default_dir() -> Option<PathBuf> {
    std::env::var_os("GRAMF_REGISTRY")
        .map(PathBuf::from)
        .or_else(|| std::env::home_dir().map(|h| h.join("AMOLF-SHIMIZU Dropbox/AMF-PlateList")))
}

/// `AAA0000A000`: experiment code, year+month, batch letter, plate number.
fn is_plate_id(s: &[u8]) -> bool {
    s.len() == 11
        && s[..3].iter().all(u8::is_ascii_uppercase)
        && s[3..7].iter().all(u8::is_ascii_digit)
        && s[7].is_ascii_uppercase()
        && s[8..].iter().all(u8::is_ascii_digit)
}

/// Plate ids a store path may name, deepest folder first: `FLW2605A003` anywhere in a
/// folder name (`FLW2605A003_t37`), or `127_HHE2601A` read as `HHE2601A127`.
pub fn plate_candidates(path: &Path) -> Vec<String> {
    let mut out = Vec::new();
    for part in path.iter().rev().filter_map(|p| p.to_str()) {
        let b = part.as_bytes();
        if b.len() >= 12 && b[..3].iter().all(u8::is_ascii_digit) && b[3] == b'_' {
            let id = format!("{}{}", &part[4..12], &part[..3]);
            if is_plate_id(id.as_bytes()) {
                out.push(id);
            }
        }
        for w in b.windows(11).filter(|w| is_plate_id(w)) {
            out.push(String::from_utf8_lossy(w).into_owned());
        }
    }
    out
}

/// First match in `NOW_Experiments`, then `OLD_Experiments`, on the `UI` or `old UI`
/// column. Columns are found by header name; Dropbox conflicted copies are skipped.
pub fn lookup(dir: &Path, candidates: &[String]) -> Option<PlateRecord> {
    for sub in ["NOW_Experiments", "OLD_Experiments"] {
        let Ok(entries) = std::fs::read_dir(dir.join(sub)) else {
            continue;
        };
        let mut files: Vec<PathBuf> = entries
            .filter_map(|e| Some(e.ok()?.path()))
            .filter(|p| {
                let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
                name.ends_with(".xlsx") && !name.starts_with('~') && !name.contains("conflicted copy")
            })
            .collect();
        files.sort();
        for file in files {
            if let Some(record) = lookup_in_file(&file, candidates) {
                return Some(record);
            }
        }
    }
    None
}

fn lookup_in_file(path: &Path, candidates: &[String]) -> Option<PlateRecord> {
    let mut workbook = calamine::open_workbook_auto(path).ok()?;
    let range = workbook.worksheet_range_at(0)?.ok()?;
    let mut rows = range.rows();
    let header: Vec<String> = rows.next()?.iter().map(|c| c.to_string().trim().to_string()).collect();
    let col = |name: &str| header.iter().position(|h| h == name);
    let (ui, old_ui) = (col("UI"), col("old UI"));
    ui.or(old_ui)?;

    for row in rows {
        let cell = |c: Option<usize>| c.and_then(|c| row.get(c)).map(|d| d.to_string().trim().to_string());
        let ids = [cell(ui), cell(old_ui)];
        if ids.iter().flatten().any(|id| candidates.contains(id)) {
            let date = |name| col(name).and_then(|c| row.get(c)).and_then(cell_date);
            return Some(PlateRecord {
                ui: cell(ui).or(cell(old_ui)).unwrap_or_default(),
                inoculation: date("Start"),
                crossed: date("Crossed"),
                source: path.to_path_buf(),
            });
        }
    }
    None
}

/// Excel serial day number or an ISO `YYYY-MM-DD…` string, as epoch seconds.
fn cell_date(cell: &Data) -> Option<i64> {
    const EXCEL_EPOCH_TO_UNIX_DAYS: f64 = 25569.0;
    let serial = match cell {
        Data::DateTime(d) => d.as_f64(),
        Data::Float(f) => *f,
        Data::DateTimeIso(s) | Data::String(s) => {
            let date = chrono::NaiveDate::parse_from_str(s.get(..10)?, "%Y-%m-%d").ok()?;
            return Some(date.and_hms_opt(0, 0, 0)?.and_utc().timestamp());
        }
        _ => return None,
    };
    Some(((serial - EXCEL_EPOCH_TO_UNIX_DAYS) * 86400.0).round() as i64)
}

#[cfg(test)]
mod tests {
    use super::{cell_date, plate_candidates};
    use calamine::Data;
    use std::path::Path;

    #[test]
    fn candidates_from_paths() {
        let c = |p: &str| plate_candidates(Path::new(p));
        assert_eq!(c("/x/temp/FLW/FLW2605A003/stg_store.h5"), ["FLW2605A003"]);
        assert_eq!(c("/DATA/NETWORK_VISU/127_HHE2601A/data/stg_store.h5"), ["HHE2601A127"]);
        assert_eq!(c("/paper_plates/PNF2307A072_t37_arcunit/stg_store.h5"), ["PNF2307A072"]);
        assert!(c("/tmp/stg_store.h5").is_empty());
    }

    #[test]
    fn dates() {
        // 2026-05-11 is Excel serial 46153
        let may11 = 1_778_457_600;
        assert_eq!(cell_date(&Data::Float(46153.0)), Some(may11));
        assert_eq!(cell_date(&Data::String("2026-05-11 00:00:00".into())), Some(may11));
        assert_eq!(cell_date(&Data::Empty), None);
    }
}
