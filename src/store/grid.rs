//! Nearest-arc queries: a grid over arc segments, and point / segment distances.

use super::{Arcs, Store};
use std::collections::HashMap;

/// Arc segments bucketed in square cells, for "nearest STG arc within r" queries.
pub struct SegmentGrid {
    cell: f32,
    /// Cell -> first vertex of each segment (`v`, `v + 1`) touching it.
    cells: HashMap<(i32, i32), Vec<u32>>,
}

impl SegmentGrid {
    const CELL_UM: f32 = 50.0;

    pub(super) fn build(arcs: &Arcs) -> Self {
        let cell = Self::CELL_UM;
        let mut cells: HashMap<(i32, i32), Vec<u32>> = HashMap::new();
        for e in arcs.offsets.windows(2) {
            for v in e[0]..e[1].saturating_sub(1) {
                let (a, b) = (arcs.points[v], arcs.points[v + 1]);
                let lo = ((a[0].min(b[0]) / cell).floor() as i32, (a[1].min(b[1]) / cell).floor() as i32);
                let hi = ((a[0].max(b[0]) / cell).floor() as i32, (a[1].max(b[1]) / cell).floor() as i32);
                for x in lo.0..=hi.0 {
                    for y in lo.1..=hi.1 {
                        cells.entry((x, y)).or_default().push(v as u32);
                    }
                }
            }
        }
        Self { cell, cells }
    }

    /// Distance from `p` to the nearest arc segment, if one lies within `max` µm, and that
    /// segment's first vertex.
    pub fn nearest(&self, arcs: &Arcs, p: [f32; 2], max: f32) -> Option<(f32, usize)> {
        let r = (max / self.cell).ceil() as i32;
        let (cx, cy) = ((p[0] / self.cell).floor() as i32, (p[1] / self.cell).floor() as i32);
        let mut best: Option<(f32, usize)> = None;
        for x in cx - r..=cx + r {
            for y in cy - r..=cy + r {
                for &v in self.cells.get(&(x, y)).into_iter().flatten() {
                    let v = v as usize;
                    let d = segment_dist2(p, arcs.points[v], arcs.points[v + 1]).sqrt();
                    if d <= max && best.is_none_or(|(bd, _)| d < bd) {
                        best = Some((d, v));
                    }
                }
            }
        }
        best
    }
}

pub(super) fn nearest(items: impl Iterator<Item = usize>, d2: impl Fn(usize) -> f32, radius: f32) -> Option<usize> {
    items
        .map(|i| (i, d2(i)))
        .filter(|&(_, d)| d <= radius * radius)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(i, _)| i)
}

pub(super) fn dist2(a: [f32; 2], b: [f32; 2]) -> f32 {
    (a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)
}

pub fn segment_dist2(p: [f32; 2], a: [f32; 2], b: [f32; 2]) -> f32 {
    let ab = [b[0] - a[0], b[1] - a[1]];
    let len2 = ab[0] * ab[0] + ab[1] * ab[1];
    let s = if len2 > 0.0 {
        (((p[0] - a[0]) * ab[0] + (p[1] - a[1]) * ab[1]) / len2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    dist2(p, [a[0] + s * ab[0], a[1] + s * ab[1]])
}

impl Store {
    pub fn grid(&self) -> &SegmentGrid {
        self.segment_grid.get_or_init(|| SegmentGrid::build(&self.arcs))
    }
}
