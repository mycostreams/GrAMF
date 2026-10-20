//! Snap and cover (DESIGN.md §10): a placed video graph is nudged onto the STG by a small rigid
//! fit, then each video edge is walked to find the chain of STG edges it lies on, with the
//! direction along each, and the stretches that lie on none (gaps).

use crate::store::{Store, segment_dist2};

/// A rigid correction applied to placed video points: rotate by `theta` about `centre`, then
/// shift by `shift` (µm).
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Rigid {
    pub centre: [f32; 2],
    pub theta: f32,
    pub shift: [f32; 2],
}

impl Rigid {
    pub fn apply(&self, p: [f32; 2]) -> [f32; 2] {
        let (s, c) = self.theta.sin_cos();
        let (x, y) = (p[0] - self.centre[0], p[1] - self.centre[1]);
        [self.centre[0] + c * x - s * y + self.shift[0], self.centre[1] + s * x + c * y + self.shift[1]]
    }
}

/// How far the snap may move a video before it is refused as implausible.
pub const MAX_SNAP_SHIFT_UM: f32 = 50.0;
pub const MAX_SNAP_ROTATION_DEG: f32 = 5.0;

/// How far the snap searches and may move: wide from a computed placement, local after the
/// user has nudged a run (so the snap refines the nudge instead of undoing it).
#[derive(Clone, Copy)]
pub struct SnapReach {
    pub first_radius_um: f32,
    pub final_radius_um: f32,
    pub max_shift_um: f32,
}

pub const SNAP_WIDE: SnapReach = SnapReach { first_radius_um: 30.0, final_radius_um: 12.0, max_shift_um: MAX_SNAP_SHIFT_UM };
pub const SNAP_LOCAL: SnapReach = SnapReach { first_radius_um: 10.0, final_radius_um: 6.0, max_shift_um: 15.0 };

/// A matched STG edge: a pipeline `edge_id`, or an added edge's id from an annotation file.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum EdgeRef {
    Pipeline(usize),
    Added(i64),
}

/// An added edge as a straight segment, for matching.
#[derive(Clone, Copy)]
pub struct AddedSegment {
    pub id: i64,
    pub a: [f32; 2],
    pub b: [f32; 2],
}

/// How one video edge lies on the STG.
#[derive(Clone, Debug)]
pub struct EdgeCover {
    pub key: [i64; 3],
    /// STG edges in order along the video edge (u to v), with the direction: +1 when the
    /// video edge runs node1 -> node2 of the STG edge (as `/edges` stores it), -1 against.
    pub chain: Vec<(EdgeRef, i8)>,
    /// Fraction of the video edge within the threshold of some STG edge.
    pub coverage: f32,
    /// Uncovered stretches (µm, placed), each at least `MIN_GAP_UM` long.
    pub gaps: Vec<Vec<[f32; 2]>>,
}

const STEP_UM: f32 = 2.0;
/// A chain member must be covered over at least this length; shorter touches are noise.
const MIN_RUN_UM: f32 = 6.0;
const MIN_GAP_UM: f32 = 5.0;

fn sub(a: [f32; 2], b: [f32; 2]) -> [f32; 2] {
    [a[0] - b[0], a[1] - b[1]]
}

fn len(v: [f32; 2]) -> f32 {
    v[0].hypot(v[1])
}

fn closest_on_segment(p: [f32; 2], a: [f32; 2], b: [f32; 2]) -> [f32; 2] {
    let ab = sub(b, a);
    let l2 = ab[0] * ab[0] + ab[1] * ab[1];
    let s = if l2 > 0.0 { (((p[0] - a[0]) * ab[0] + (p[1] - a[1]) * ab[1]) / l2).clamp(0.0, 1.0) } else { 0.0 };
    [a[0] + s * ab[0], a[1] + s * ab[1]]
}

/// Small rigid fit (ICP: rotation about the centroid plus translation) of `points` onto the
/// nearest STG arcs. Returns `None` when there is too little to fit on, or when the result
/// moves more than `reach.max_shift_um` / [`MAX_SNAP_ROTATION_DEG`].
pub fn snap(store: &Store, points: &[[f32; 2]], reach: SnapReach) -> Option<Rigid> {
    if points.len() < 10 {
        return None;
    }
    let n = points.len() as f32;
    let centre = points.iter().fold([0.0, 0.0], |a, p| [a[0] + p[0] / n, a[1] + p[1] / n]);
    let grid = store.grid();
    let mut total = Rigid { centre, ..Default::default() };
    for iteration in 0..20 {
        let radius = if iteration < 5 { reach.first_radius_um } else { reach.final_radius_um };
        let moved: Vec<[f32; 2]> = points.iter().map(|&p| total.apply(p)).collect();
        let pairs: Vec<([f32; 2], [f32; 2])> = moved
            .iter()
            .filter_map(|&p| {
                let (_, v) = grid.nearest(&store.arcs, p, radius)?;
                Some((p, closest_on_segment(p, store.arcs.points[v], store.arcs.points[v + 1])))
            })
            .collect();
        if pairs.len() < 10 {
            break;
        }
        let k = pairs.len() as f32;
        let mp = pairs.iter().fold([0.0, 0.0], |a, (p, _)| [a[0] + p[0] / k, a[1] + p[1] / k]);
        let mq = pairs.iter().fold([0.0, 0.0], |a, (_, q)| [a[0] + q[0] / k, a[1] + q[1] / k]);
        let (mut dot, mut cross) = (0.0, 0.0);
        for (p, q) in &pairs {
            let (p, q) = (sub(*p, mp), sub(*q, mq));
            dot += p[0] * q[0] + p[1] * q[1];
            cross += p[0] * q[1] - p[1] * q[0];
        }
        let dtheta = cross.atan2(dot);
        // the step: rotate about mp by dtheta, then move mp onto mq; fold it into `total`
        let step = Rigid { centre: mp, theta: dtheta, shift: sub(mq, mp) };
        let new_centre_image = step.apply(total.apply(centre));
        total = Rigid { centre, theta: total.theta + dtheta, shift: sub(new_centre_image, centre) };
        if dtheta.abs() < 1e-5 && len(sub(mq, mp)) < 0.01 {
            break;
        }
    }
    let plausible = len(total.shift) <= reach.max_shift_um && total.theta.abs().to_degrees() <= MAX_SNAP_ROTATION_DEG;
    plausible.then_some(total)
}

/// Walks a placed video edge (`points`, u to v) and finds the STG edges it lies on within
/// `threshold` µm: pipeline arcs, and `added` edges.
pub fn cover(store: &Store, added: &[AddedSegment], key: [i64; 3], points: &[[f32; 2]], threshold: f32) -> EdgeCover {
    // resample at STEP_UM with the tangent at each sample
    let mut samples: Vec<([f32; 2], [f32; 2])> = vec![];
    let mut carry = 0.0;
    for w in points.windows(2) {
        let d = sub(w[1], w[0]);
        let l = len(d);
        if l == 0.0 {
            continue;
        }
        let t = [d[0] / l, d[1] / l];
        let mut s = carry;
        while s < l {
            samples.push(([w[0][0] + t[0] * s, w[0][1] + t[1] * s], t));
            s += STEP_UM;
        }
        carry = s - l;
    }

    let grid = store.grid();
    let offsets = &store.arcs.offsets;
    // per sample: the STG edge under it and whether the video runs along that edge's arc
    let hits: Vec<Option<(EdgeRef, bool)>> = samples
        .iter()
        .map(|&(p, t)| {
            let pipeline = grid.nearest(&store.arcs, p, threshold).map(|(d, v)| {
                let e = offsets.partition_point(|&o| o <= v) - 1;
                let seg = sub(store.arcs.points[v + 1], store.arcs.points[v]);
                (d, EdgeRef::Pipeline(e), t[0] * seg[0] + t[1] * seg[1] >= 0.0)
            });
            let extra = added
                .iter()
                .map(|a| {
                    let seg = sub(a.b, a.a);
                    (segment_dist2(p, a.a, a.b).sqrt(), EdgeRef::Added(a.id), t[0] * seg[0] + t[1] * seg[1] >= 0.0)
                })
                .filter(|x| x.0 <= threshold)
                .min_by(|x, y| x.0.total_cmp(&y.0));
            [pipeline, extra].into_iter().flatten().min_by(|x, y| x.0.total_cmp(&y.0)).map(|(_, e, along)| (e, along))
        })
        .collect();

    // runs of the same edge, in order: (edge, length in samples, along-votes)
    let mut runs: Vec<(EdgeRef, usize, i32)> = vec![];
    let mut i = 0;
    while i < hits.len() {
        let Some((e, _)) = hits[i] else {
            i += 1;
            continue;
        };
        let start = i;
        let mut votes = 0;
        while i < hits.len() && hits[i].is_some_and(|h| h.0 == e) {
            votes += if hits[i].is_some_and(|h| h.1) { 1 } else { -1 };
            i += 1;
        }
        runs.push((e, i - start, votes));
    }
    // short runs are noise, unless nothing else is left (a video edge shorter than that)
    let longest = runs.iter().map(|r| r.1).max().unwrap_or(0);
    let min_samples = ((MIN_RUN_UM / STEP_UM) as usize).min(longest);
    let mut chain: Vec<(EdgeRef, i32)> = vec![]; // (edge, along-votes)
    for (e, _, votes) in runs.into_iter().filter(|r| r.1 >= min_samples.max(1)) {
        match chain.last_mut() {
            Some(last) if last.0 == e => last.1 += votes,
            _ => chain.push((e, votes)),
        }
    }
    let chain = chain
        .into_iter()
        .map(|(e, votes)| {
            // votes say "along the arc"; convert to "along node1 -> node2"
            let along_arc = votes >= 0;
            let along_nodes = match e {
                EdgeRef::Pipeline(id) => along_arc != store.arc_reversed(id),
                EdgeRef::Added(_) => along_arc,
            };
            (e, if along_nodes { 1 } else { -1 })
        })
        .collect();

    let mut gaps = vec![];
    let mut i = 0;
    while i < hits.len() {
        if hits[i].is_some() {
            i += 1;
            continue;
        }
        let start = i;
        while i < hits.len() && hits[i].is_none() {
            i += 1;
        }
        if (i - start) as f32 * STEP_UM >= MIN_GAP_UM {
            gaps.push(samples[start..i].iter().map(|s| s.0).collect());
        }
    }
    let covered = hits.iter().filter(|h| h.is_some()).count();
    EdgeCover { key, chain, coverage: if hits.is_empty() { 0.0 } else { covered as f32 / hits.len() as f32 }, gaps }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::{Arcs, Store};

    /// Two pipeline edges along the x axis: 0 = nodes 1-2 (0..100), 1 = nodes 3-2 with its arc
    /// stored 100..200 but `/edges` saying node 3 first (node 3 sits at 200).
    fn store() -> Store {
        let mut s = Store {
            arcs: Arcs {
                offsets: vec![0, 2, 4],
                points: vec![[0.0, 0.0], [100.0, 0.0], [100.0, 0.0], [200.0, 0.0]],
                min: [0.0, 0.0],
                max: [200.0, 0.0],
            },
            node_ids: vec![1, 2, 3],
            node_pos: vec![[0.0, 0.0], [100.0, 0.0], [200.0, 0.0]],
            edge_nodes: vec![[1, 2], [3, 2]],
            ..Default::default()
        };
        s.node_index = s.node_ids.iter().enumerate().map(|(i, &id)| (id, i)).collect();
        s
    }

    #[test]
    fn covers_a_chain_with_directions_and_a_gap() {
        let s = store();
        // a video edge running 10 -> 260 along x, 2 µm off; 200..260 is off the STG
        let pts = [[10.0, 2.0], [260.0, 2.0]];
        let c = cover(&s, &[], [0, 1, 0], &pts, 5.0);
        assert_eq!(c.chain, vec![(EdgeRef::Pipeline(0), 1), (EdgeRef::Pipeline(1), -1)]);
        assert_eq!(c.gaps.len(), 1);
        assert!((c.coverage - 0.76).abs() < 0.03, "coverage {}", c.coverage);
        // an added edge closes the gap
        let added = [AddedSegment { id: 1_000_000, a: [200.0, 0.0], b: [300.0, 0.0] }];
        let c = cover(&s, &added, [0, 1, 0], &pts, 5.0);
        assert_eq!(c.chain.last(), Some(&(EdgeRef::Added(1_000_000), 1)));
        assert!(c.gaps.is_empty());
    }

    #[test]
    fn snap_pulls_a_shifted_copy_back() {
        let s = store();
        let pts: Vec<[f32; 2]> = (0..190).map(|i| [5.0 + i as f32, 7.0]).collect();
        let r = snap(&s, &pts, SNAP_WIDE).unwrap();
        let p = r.apply([50.0, 7.0]);
        assert!(p[1].abs() < 0.5, "snapped y {}", p[1]);
    }
}
