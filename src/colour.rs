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

/// A colormap and its direction.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub struct Scale {
    pub cmap: Cmap,
    pub reversed: bool,
}

impl Scale {
    pub fn sample(self, x: f32) -> [f32; 4] {
        self.cmap.sample(if self.reversed { 1.0 - x } else { x })
    }
}

const UNIFORM: [f32; 4] = [0.75, 0.9, 0.6, 1.0];
const MISSING: [f32; 4] = [0.35, 0.35, 0.35, 1.0];
pub const HIGHLIGHT: [f32; 4] = [1.0, 1.0, 1.0, 1.0];

/// Continuous colormaps, sampled from matplotlib at 9 evenly spaced stops.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum Cmap {
    #[default]
    Viridis,
    Plasma,
    Magma,
    Inferno,
    Cividis,
    Turbo,
    Coolwarm,
    RdYlBu,
}

impl Cmap {
    pub const ALL: [Cmap; 8] = [
        Cmap::Viridis,
        Cmap::Plasma,
        Cmap::Magma,
        Cmap::Inferno,
        Cmap::Cividis,
        Cmap::Turbo,
        Cmap::Coolwarm,
        Cmap::RdYlBu,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Cmap::Viridis => "viridis",
            Cmap::Plasma => "plasma",
            Cmap::Magma => "magma",
            Cmap::Inferno => "inferno",
            Cmap::Cividis => "cividis",
            Cmap::Turbo => "turbo",
            Cmap::Coolwarm => "coolwarm",
            Cmap::RdYlBu => "RdYlBu_r",
        }
    }

    fn stops(self) -> &'static [[f32; 3]; 9] {
        match self {
            Cmap::Viridis => &[[0.267, 0.005, 0.329], [0.279, 0.175, 0.483], [0.230, 0.322, 0.546], [0.173, 0.449, 0.558], [0.128, 0.567, 0.551], [0.158, 0.684, 0.502], [0.369, 0.789, 0.383], [0.678, 0.864, 0.190], [0.993, 0.906, 0.144]],
            Cmap::Plasma => &[[0.050, 0.030, 0.528], [0.300, 0.010, 0.632], [0.495, 0.012, 0.658], [0.665, 0.139, 0.586], [0.798, 0.280, 0.470], [0.902, 0.425, 0.360], [0.973, 0.586, 0.252], [0.993, 0.772, 0.155], [0.940, 0.975, 0.131]],
            Cmap::Magma => &[[0.001, 0.000, 0.014], [0.113, 0.065, 0.277], [0.317, 0.072, 0.485], [0.513, 0.148, 0.508], [0.716, 0.215, 0.475], [0.904, 0.320, 0.388], [0.987, 0.536, 0.382], [0.997, 0.770, 0.535], [0.987, 0.991, 0.750]],
            Cmap::Inferno => &[[0.001, 0.000, 0.014], [0.129, 0.047, 0.291], [0.342, 0.062, 0.429], [0.541, 0.135, 0.415], [0.736, 0.216, 0.330], [0.894, 0.353, 0.194], [0.978, 0.558, 0.035], [0.975, 0.798, 0.206], [0.988, 0.998, 0.645]],
            Cmap::Cividis => &[[0.000, 0.135, 0.305], [0.103, 0.220, 0.436], [0.264, 0.308, 0.423], [0.381, 0.395, 0.436], [0.489, 0.485, 0.471], [0.609, 0.580, 0.464], [0.736, 0.681, 0.424], [0.871, 0.790, 0.343], [0.996, 0.909, 0.218]],
            Cmap::Turbo => &[[0.190, 0.072, 0.232], [0.276, 0.421, 0.891], [0.158, 0.736, 0.923], [0.197, 0.949, 0.595], [0.644, 0.990, 0.234], [0.933, 0.812, 0.227], [0.984, 0.493, 0.128], [0.816, 0.185, 0.018], [0.480, 0.016, 0.011]],
            Cmap::Coolwarm => &[[0.230, 0.299, 0.754], [0.384, 0.510, 0.918], [0.554, 0.690, 0.996], [0.724, 0.815, 0.976], [0.867, 0.864, 0.863], [0.960, 0.767, 0.674], [0.957, 0.598, 0.477], [0.865, 0.371, 0.296], [0.706, 0.016, 0.150]],
            Cmap::RdYlBu => &[[0.192, 0.212, 0.584], [0.318, 0.515, 0.735], [0.565, 0.766, 0.868], [0.830, 0.929, 0.959], [1.000, 0.998, 0.745], [0.995, 0.825, 0.514], [0.973, 0.547, 0.318], [0.868, 0.240, 0.177], [0.647, 0.000, 0.149]],
        }
    }

    /// Linear interpolation between the stops; `x` is clamped to [0, 1].
    pub fn sample(self, x: f32) -> [f32; 4] {
        let stops = self.stops();
        let x = x.clamp(0.0, 1.0) * 8.0;
        let i = (x as usize).min(7);
        let f = x - i as f32;
        let [a, b] = [stops[i], stops[i + 1]];
        [a[0] + f * (b[0] - a[0]), a[1] + f * (b[1] - a[1]), a[2] + f * (b[2] - a[2]), 1.0]
    }
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

/// Width colour range: 5-95 % of measured widths (per-timestep when the store has them), so
/// outliers don't flatten the scale. Fixed over time, so colours compare across frames.
pub fn width_range(store: &Store) -> Option<(f32, f32)> {
    if let Some([lo, _, hi]) = store.width_t_quantiles {
        return Some((lo, hi));
    }
    let widths = || store.width.iter().flatten().map(|w| w.0);
    Some((quantile(widths(), 0.05)?, quantile(widths(), 0.95)?))
}

/// Typical width (µm), drawn at the base line thickness when thickness follows width.
pub fn reference_width(store: &Store) -> f32 {
    store
        .width_t_quantiles
        .map(|q| q[1])
        .or_else(|| quantile(store.width.iter().flatten().map(|w| w.0), 0.5))
        .unwrap_or(0.0)
}

/// Per edge `[r, g, b, width_um]` at timepoint `t`: the colour is used in width mode, the
/// width for line thickness. Unknown width is 0 (drawn at base thickness, grey).
pub fn edge_styles(store: &Store, t: Option<usize>, scale: Scale) -> Vec<[f32; 4]> {
    let range = width_range(store);
    (0..store.n_edges())
        .map(|e| match (store.width_at(e, t), range) {
            (Some(w), Some((lo, hi))) => {
                let [r, g, b, _] = scale.sample((w - lo) / (hi - lo).max(1e-6));
                [r, g, b, w]
            }
            (Some(w), None) => [MISSING[0], MISSING[1], MISSING[2], w],
            (None, _) => [MISSING[0], MISSING[1], MISSING[2], 0.0],
        })
        .collect()
}

/// One colour per arc vertex (all modes but width).
pub fn vertex_colours(store: &Store, by: ColourBy, cmap: Scale, n_timepoints: usize) -> Vec<[f32; 4]> {
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
            store.vertex_t.iter().map(|&t| cmap.sample(t / last)).collect()
        }
        // Width changes over time: coloured per edge and frame by `edge_styles` instead.
        ColourBy::Width => vec![MISSING; store.arcs.points.len()],
        ColourBy::Hyperedge => per_edge(&|e| match store.hyperedge[e] {
            h if h >= 0 => id_colour(h),
            _ => MISSING,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::{Cmap, quantile};

    #[test]
    fn colourmap_and_quantile() {
        let close = |a: [f32; 4], b: [f32; 4]| a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-5);
        let v = Cmap::Viridis;
        assert!(close(v.sample(0.0), [0.267, 0.005, 0.329, 1.0]));
        assert!(close(v.sample(0.5), [0.128, 0.567, 0.551, 1.0]));
        assert!(close(v.sample(1.0), [0.993, 0.906, 0.144, 1.0]));
        assert_eq!(v.sample(2.0), v.sample(1.0)); // clamped
        assert_eq!(quantile([3.0, f32::NAN, 1.0, 2.0].into_iter(), 0.5), Some(2.0));
    }
}
