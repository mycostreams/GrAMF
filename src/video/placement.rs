//! Placing videos on the STG (DESIGN.md §10): a session's stage-to-STG similarity, fitted live
//! from anchors on top of the setup default, and the camera's orientation relative to the stage.

/// `world = scale * R(theta) * M * stage + t`, with `M` mirroring the second axis when
/// `mirror`. Stage in mm, world (STG) in µm.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Similarity {
    pub scale: f64,
    pub theta: f64,
    pub mirror: bool,
    pub t: [f64; 2],
}

impl Default for Similarity {
    /// No setup default yet: stage mm to µm, axes as they are.
    fn default() -> Self {
        Self { scale: 1000.0, theta: 0.0, mirror: false, t: [0.0; 2] }
    }
}

impl Similarity {
    /// Rotation and mirror only (no scale, no translation).
    pub fn orient(&self, v: [f64; 2]) -> [f64; 2] {
        let v = if self.mirror { [v[0], -v[1]] } else { v };
        let (s, c) = self.theta.sin_cos();
        [c * v[0] - s * v[1], s * v[0] + c * v[1]]
    }

    pub fn apply(&self, stage: [f64; 2]) -> [f64; 2] {
        let o = self.orient(stage);
        [self.scale * o[0] + self.t[0], self.scale * o[1] + self.t[1]]
    }
}

/// One of the 8 rotations / mirrors of the camera image relative to the stage axes:
/// `k % 4` quarter turns after mirroring the second axis when `k >= 4`.
pub fn camera_orient(k: u8, v: [f64; 2]) -> [f64; 2] {
    let mut v = if k >= 4 { [v[0], -v[1]] } else { v };
    for _ in 0..k % 4 {
        v = [-v[1], v[0]];
    }
    v
}

/// How a session's fit came about.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FitKind {
    /// No anchors: the setup default.
    Default,
    /// One anchor fixes the offset.
    Offset,
    /// Two anchors also fix rotation and scale.
    TwoPoint,
    /// Three or more: least squares, mirror chosen by residual.
    LeastSquares,
}

/// Fits stage -> world from `anchors` (`(stage_mm, world_um)`), starting from `default`.
/// Returns the transform, how it was fitted and each anchor's residual (µm).
pub fn fit(anchors: &[([f64; 2], [f64; 2])], default: Similarity) -> (Similarity, FitKind, Vec<f64>) {
    let (sim, kind) = match anchors.len() {
        0 => (default, FitKind::Default),
        1 => {
            let (a, b) = anchors[0];
            let p = Similarity { t: [0.0; 2], ..default }.apply(a);
            (Similarity { t: [b[0] - p[0], b[1] - p[1]], ..default }, FitKind::Offset)
        }
        2 => (least_squares(anchors, default.mirror), FitKind::TwoPoint),
        _ => {
            let options = [least_squares(anchors, false), least_squares(anchors, true)];
            let best = options
                .into_iter()
                .min_by(|x, y| rms(anchors, x).total_cmp(&rms(anchors, y)))
                .expect("two options");
            (best, FitKind::LeastSquares)
        }
    };
    let residuals = anchors.iter().map(|&(a, b)| dist(sim.apply(a), b)).collect();
    (sim, kind, residuals)
}

fn dist(p: [f64; 2], q: [f64; 2]) -> f64 {
    ((p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2)).sqrt()
}

fn rms(anchors: &[([f64; 2], [f64; 2])], s: &Similarity) -> f64 {
    let sum: f64 = anchors.iter().map(|&(a, b)| dist(s.apply(a), b).powi(2)).sum();
    (sum / anchors.len() as f64).sqrt()
}

/// Least-squares similarity (2D Umeyama, via complex numbers): `z = k * m(a) + t` with `k`
/// complex, `m` the optional mirror.
fn least_squares(anchors: &[([f64; 2], [f64; 2])], mirror: bool) -> Similarity {
    let n = anchors.len() as f64;
    let m = |a: [f64; 2]| if mirror { [a[0], -a[1]] } else { a };
    let mean = |f: &dyn Fn(usize) -> [f64; 2]| {
        let s = (0..anchors.len()).fold([0.0, 0.0], |acc, i| [acc[0] + f(i)[0], acc[1] + f(i)[1]]);
        [s[0] / n, s[1] / n]
    };
    let ma = mean(&|i| m(anchors[i].0));
    let mb = mean(&|i| anchors[i].1);
    // k = sum(conj(a') * b') / sum(|a'|^2)
    let (mut re, mut im, mut norm) = (0.0, 0.0, 0.0);
    for &(a, b) in anchors {
        let a = m(a);
        let (ax, ay, bx, by) = (a[0] - ma[0], a[1] - ma[1], b[0] - mb[0], b[1] - mb[1]);
        re += ax * bx + ay * by;
        im += ax * by - ay * bx;
        norm += ax * ax + ay * ay;
    }
    let (kr, ki) = if norm > 0.0 { (re / norm, im / norm) } else { (1.0, 0.0) };
    let rotated = [kr * ma[0] - ki * ma[1], ki * ma[0] + kr * ma[1]];
    Similarity {
        scale: kr.hypot(ki),
        theta: ki.atan2(kr),
        mirror,
        t: [mb[0] - rotated[0], mb[1] - rotated[1]],
    }
}

/// Where a video-graph point (µm from the frame centre, camera axes) lands on the STG for a
/// video filmed at `stage` mm: scaled by `video_scale` (STG µm per video µm, 1 when the
/// network's pixel size is right), the camera turned into stage axes, then the session's
/// rotation and mirror. The session's own scale only spaces positions; it never resizes videos.
pub fn place(sim: &Similarity, camera: u8, video_scale: f32, stage: [f64; 2], p_um: [f32; 2]) -> [f32; 2] {
    let centre = sim.apply(stage);
    let k = video_scale as f64;
    let o = sim.orient(camera_orient(camera, [p_um[0] as f64 * k, p_um[1] as f64 * k]));
    [(centre[0] + o[0]) as f32, (centre[1] + o[1]) as f32]
}

/// Distance cap for the fit score: points further than this from any arc count as this far.
pub const SCORE_CAP_UM: f32 = 50.0;

/// Mean distance (µm, capped at [`SCORE_CAP_UM`]) from `points` to the nearest STG arc, by
/// `nearest`. Lower is better; `None` without points.
pub fn fit_score(points: impl Iterator<Item = [f32; 2]>, nearest: impl Fn([f32; 2]) -> Option<f32>) -> Option<f32> {
    let (mut sum, mut n) = (0.0, 0usize);
    for p in points {
        sum += nearest(p).unwrap_or(SCORE_CAP_UM).min(SCORE_CAP_UM);
        n += 1;
    }
    (n > 0).then(|| sum / n as f32)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// FLW2605A003 (ARETHA): stage positions and the nodes `position_annotation.png` placed them
    /// on, from the registrator's earlier registration of this plate.
    const FLW: [([f64; 2], [f64; 2]); 6] = [
        ([-10.3797, 4.484], [31255.996, 37779.561]),
        ([-10.4763, 4.9869], [31167.964, 37288.724]),
        ([-10.5072, 4.862], [31134.187, 37408.669]),
        ([-10.6178, 4.2929], [31009.407, 37986.084]),
        ([-10.739, 3.875], [30882.549, 38400.090]),
        ([-10.8863, 3.999], [30738.284, 38275.418]),
    ];

    #[test]
    fn recovers_a_known_transform() {
        let truth = Similarity { scale: 990.0, theta: 0.3, mirror: true, t: [500.0, -200.0] };
        let anchors: Vec<_> = [[1.0, 2.0], [-3.0, 0.5], [4.0, -1.0], [0.0, 0.0]].iter().map(|&a| (a, truth.apply(a))).collect();
        let (s, kind, res) = fit(&anchors, Similarity::default());
        assert_eq!(kind, FitKind::LeastSquares);
        assert!(s.mirror && (s.scale - 990.0).abs() < 1e-6 && (s.theta - 0.3).abs() < 1e-9);
        assert!(res.iter().all(|r| *r < 1e-6));
        // one anchor keeps the default's scale and rotation, fixes the offset
        let (one, kind, res) = fit(&anchors[..1], truth);
        assert_eq!(kind, FitKind::Offset);
        assert!(res[0] < 1e-9 && one.scale == 990.0);
    }

    #[test]
    fn flw_anchors_fit_a_mirrored_mm_to_um_map() {
        let (s, _, res) = fit(&FLW, Similarity::default());
        assert!(s.mirror, "ARETHA stage y runs against STG y");
        assert!((s.scale - 1000.0).abs() < 30.0, "scale {}", s.scale);
        assert!(s.theta.abs() < 0.05, "theta {}", s.theta);
        assert!(res.iter().all(|r| *r < 40.0), "residuals {res:?}");
    }

    #[test]
    fn camera_orientations_are_the_8_symmetries() {
        let v = [1.0, 2.0];
        let all: Vec<_> = (0..8).map(|k| camera_orient(k, v)).collect();
        for i in 0..8 {
            for j in i + 1..8 {
                assert_ne!(all[i], all[j]);
            }
        }
        assert_eq!(camera_orient(1, v), [-2.0, 1.0]);
        assert_eq!(camera_orient(4, v), [1.0, -2.0]);
    }

    #[test]
    fn video_scale_scales_about_the_frame_centre() {
        let sim = Similarity { scale: 1146.0, theta: 0.3, mirror: true, t: [5.0, -7.0] };
        let stage = [-3.271, -3.167];
        let centre = place(&sim, 5, 1.0, stage, [0.0, 0.0]);
        // the frame centre does not move with the factor
        assert_eq!(place(&sim, 5, 1.1458, stage, [0.0, 0.0]), centre);
        // a point 100 µm out lands 114.58 µm out, in the same direction
        let (a, b) = (place(&sim, 5, 1.0, stage, [100.0, 0.0]), place(&sim, 5, 1.1458, stage, [100.0, 0.0]));
        let d = |p: [f32; 2]| ((p[0] - centre[0]).hypot(p[1] - centre[1]), (p[1] - centre[1]).atan2(p[0] - centre[0]));
        let ((ra, ta), (rb, tb)) = (d(a), d(b));
        assert!((ra - 100.0).abs() < 1e-2 && (rb - 114.58).abs() < 1e-2 && (ta - tb).abs() < 1e-5, "{ra} {rb} {ta} {tb}");
    }
}
