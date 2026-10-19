//! The time axis of a loaded store and how its positions are labelled (DESIGN.md §4).

use chrono::DateTime;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum TimeUnit {
    Frame,
    Utc,
    HoursSinceInoculation,
    HoursSinceCrossing,
}

impl TimeUnit {
    pub const ALL: [TimeUnit; 4] = [
        TimeUnit::Frame,
        TimeUnit::Utc,
        TimeUnit::HoursSinceInoculation,
        TimeUnit::HoursSinceCrossing,
    ];

    pub fn name(self) -> &'static str {
        match self {
            TimeUnit::Frame => "frame",
            TimeUnit::Utc => "UTC",
            TimeUnit::HoursSinceInoculation => "h since inoculation",
            TimeUnit::HoursSinceCrossing => "h since crossing",
        }
    }
}

pub struct TimeAxis {
    /// Epoch seconds per timepoint.
    pub timepoints: Vec<i64>,
    /// 0 = gated frame.
    pub quality: Vec<f64>,
    /// Epoch seconds, from the plate registry's `Start` column.
    pub inoculation: Option<i64>,
    /// Epoch seconds, and whether it is only approximate (the store's first timepoint,
    /// used when the registry has no `Crossed` date).
    pub crossing: (i64, bool),
}

impl TimeAxis {
    pub fn new(timepoints: Vec<i64>, quality: Vec<f64>, inoculation: Option<i64>, crossed: Option<i64>) -> Option<Self> {
        let first = *timepoints.first()?;
        Some(Self {
            crossing: crossed.map_or((first, true), |c| (c, false)),
            timepoints,
            quality,
            inoculation,
        })
    }

    pub fn len(&self) -> usize {
        self.timepoints.len()
    }

    pub fn is_gated(&self, i: usize) -> bool {
        self.quality[i] == 0.0
    }

    pub fn available(&self, unit: TimeUnit) -> bool {
        unit != TimeUnit::HoursSinceInoculation || self.inoculation.is_some()
    }

    pub fn label(&self, i: usize, unit: TimeUnit) -> String {
        let t = self.timepoints[i];
        let hours = |since: i64| (t - since) as f64 / 3600.0;
        match unit {
            TimeUnit::Frame => format!("frame {i} / {}", self.len() - 1),
            TimeUnit::Utc => DateTime::from_timestamp(t, 0)
                .map_or_else(|| t.to_string(), |d| d.format("%Y-%m-%d %H:%M UTC").to_string()),
            TimeUnit::HoursSinceInoculation => match self.inoculation {
                Some(start) => format!("{:.1} h since inoculation", hours(start)),
                None => "inoculation date unknown".into(),
            },
            TimeUnit::HoursSinceCrossing => {
                let (crossed, approx) = self.crossing;
                let mark = if approx { "≈ " } else { "" };
                format!("{mark}{:.1} h since crossing", hours(crossed))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{TimeAxis, TimeUnit};

    #[test]
    fn labels() {
        let axis = TimeAxis::new(vec![7200, 10800], vec![0.0, 1.0], Some(0), None).unwrap();
        assert_eq!(axis.label(1, TimeUnit::Frame), "frame 1 / 1");
        assert_eq!(axis.label(1, TimeUnit::Utc), "1970-01-01 03:00 UTC");
        assert_eq!(axis.label(1, TimeUnit::HoursSinceInoculation), "3.0 h since inoculation");
        // no registry crossing date: falls back to the first timepoint, marked approximate
        assert_eq!(axis.label(1, TimeUnit::HoursSinceCrossing), "≈ 1.0 h since crossing");
        assert!(axis.is_gated(0) && !axis.is_gated(1));
    }
}
