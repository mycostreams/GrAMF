//! Per-vertex colours for the colour-by modes.

use crate::store::Store;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ColourBy {
    Uniform,
    TFirst,
    Width,
    Hyperedge,
}

impl ColourBy {
    pub const ALL: [ColourBy; 4] = [ColourBy::Uniform, ColourBy::TFirst, ColourBy::Width, ColourBy::Hyperedge];

    pub fn name(self) -> &'static str {
        match self {
            ColourBy::Uniform => "uniform",
            ColourBy::TFirst => "t_first",
            ColourBy::Width => "width",
            ColourBy::Hyperedge => "hyperedge",
        }
    }
}

const UNIFORM: [f32; 4] = [0.75, 0.9, 0.6, 1.0];
const MISSING: [f32; 4] = [0.35, 0.35, 0.35, 1.0];
pub const HIGHLIGHT: [f32; 4] = [1.0, 1.0, 1.0, 1.0];

/// Viridis at 5 stops, linearly interpolated; `x` in [0, 1].
pub fn viridis(x: f32) -> [f32; 4] {
    const STOPS: [[f32; 3]; 5] = [
        [0.267, 0.005, 0.329],
        [0.231, 0.322, 0.545],
        [0.129, 0.569, 0.549],
        [0.369, 0.788, 0.384],
        [0.993, 0.906, 0.144],
    ];
    let x = x.clamp(0.0, 1.0) * 4.0;
    let i = (x as usize).min(3);
    let f = x - i as f32;
    let [a, b] = [STOPS[i], STOPS[i + 1]];
    [a[0] + f * (b[0] - a[0]), a[1] + f * (b[1] - a[1]), a[2] + f * (b[2] - a[2]), 1.0]
}

/// Distinct hue per id (golden-ratio steps), full saturation.
fn id_colour(id: i64) -> [f32; 4] {
    let h = (id as f64 * 0.618_033_988_75).fract() as f32 * 6.0;
    let x = 1.0 - (h % 2.0 - 1.0).abs();
    let [r, g, b] = match h as u32 {
        0 => [1.0, x, 0.0],
        1 => [x, 1.0, 0.0],
        2 => [0.0, 1.0, x],
        3 => [0.0, x, 1.0],
        4 => [x, 0.0, 1.0],
        _ => [1.0, 0.0, x],
    };
    [0.3 + 0.7 * r, 0.3 + 0.7 * g, 0.3 + 0.7 * b, 1.0]
}

/// Value at quantile `q` of the finite values.
fn quantile(values: impl Iterator<Item = f32>, q: f32) -> Option<f32> {
    let mut v: Vec<f32> = values.filter(|x| x.is_finite()).collect();
    v.sort_by(f32::total_cmp);
    v.get(((v.len() as f32 - 1.0) * q).round() as usize).copied()
}

/// Width colour range: 5-95 % of measured widths, so outliers don't flatten the scale.
pub fn width_range(store: &Store) -> Option<(f32, f32)> {
    let widths = || store.width.iter().flatten().map(|w| w.0);
    Some((quantile(widths(), 0.05)?, quantile(widths(), 0.95)?))
}

/// One colour per arc vertex.
pub fn vertex_colours(store: &Store, by: ColourBy, n_timepoints: usize) -> Vec<[f32; 4]> {
    let per_edge = |edge_colour: &dyn Fn(usize) -> [f32; 4]| -> Vec<[f32; 4]> {
        store
            .arcs
            .offsets
            .windows(2)
            .enumerate()
            .flat_map(|(e, r)| std::iter::repeat_n(edge_colour(e), r[1] - r[0]))
            .collect()
    };
    match by {
        ColourBy::Uniform => vec![UNIFORM; store.arcs.points.len()],
        ColourBy::TFirst => {
            let last = n_timepoints.saturating_sub(1).max(1) as f32;
            store.vertex_t.iter().map(|&t| viridis(t / last)).collect()
        }
        ColourBy::Width => {
            let Some((lo, hi)) = width_range(store) else {
                return vec![MISSING; store.arcs.points.len()];
            };
            per_edge(&|e| match store.width[e] {
                Some((w, _)) => viridis((w - lo) / (hi - lo).max(1e-6)),
                None => MISSING,
            })
        }
        ColourBy::Hyperedge => per_edge(&|e| match store.hyperedge[e] {
            h if h >= 0 => id_colour(h),
            _ => MISSING,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::{quantile, viridis};

    #[test]
    fn colourmap_and_quantile() {
        let close = |a: [f32; 4], b: [f32; 4]| a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-5);
        assert!(close(viridis(0.0), [0.267, 0.005, 0.329, 1.0]));
        assert!(close(viridis(1.0), [0.993, 0.906, 0.144, 1.0]));
        assert_eq!(viridis(2.0), viridis(1.0)); // clamped
        assert_eq!(quantile([3.0, f32::NAN, 1.0, 2.0].into_iter(), 0.5), Some(2.0));
    }
}
