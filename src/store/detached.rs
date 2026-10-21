//! Arc ends that stop short of the node `/edges` gives them.

use super::Store;

/// An arc end this far from its node is drawn with a gap, the node floating beside it.
pub const DETACHED_END_UM: f32 = 15.0;

/// An arc end that stops short of (or overshoots) the node `/edges` gives it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DetachedEnd {
    pub edge: usize,
    /// Index into `arcs.points` of the arc end.
    pub point: usize,
    /// Node row.
    pub node: usize,
    pub gap_um: f32,
}

impl Store {
    /// Arc ends far from their node, largest gap first.
    pub fn detached_ends(&self) -> &[DetachedEnd] {
        self.detached.get_or_init(|| {
            let mut out = vec![];
            for e in 0..self.n_edges() {
                let (a, b) = (self.arcs.offsets[e], self.arcs.offsets[e + 1]);
                if a == b {
                    continue;
                }
                let [u, v] = self.edge_nodes[e];
                let (pu, pv) = if self.arc_reversed(e) { (b - 1, a) } else { (a, b - 1) };
                for (id, point) in [(u, pu), (v, pv)] {
                    let Some(&node) = self.node_index.get(&id) else {
                        continue; // reported by the "missing from /nodes" check
                    };
                    let (p, q) = (self.arcs.points[point], self.node_pos[node]);
                    let gap_um = (p[0] - q[0]).hypot(p[1] - q[1]);
                    if gap_um > DETACHED_END_UM {
                        out.push(DetachedEnd { edge: e, point, node, gap_um });
                    }
                }
            }
            out.sort_by(|a, b| b.gap_um.total_cmp(&a.gap_um));
            out
        })
    }
}
