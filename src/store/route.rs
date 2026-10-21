//! Routing over the STG: shortest node paths through waypoints, and their geometry.

use super::Store;
use std::collections::HashMap;

/// Dijkstra between two node ids; the path as node ids, `None` if unreachable.
pub(super) fn dijkstra(adjacency: &HashMap<i64, Vec<(i64, f32)>>, from: i64, to: i64) -> Option<Vec<i64>> {
    use std::cmp::Reverse;
    use std::collections::BinaryHeap;
    // Non-negative f32 compare like their bit patterns, so they can key the heap.
    let key = |d: f32| Reverse(d.to_bits());
    let mut dist: HashMap<i64, f32> = HashMap::from([(from, 0.0)]);
    let mut prev: HashMap<i64, i64> = HashMap::new();
    let mut heap = BinaryHeap::from([(key(0.0), from)]);
    while let Some((Reverse(bits), node)) = heap.pop() {
        let d = f32::from_bits(bits);
        if node == to {
            let mut path = vec![to];
            while let Some(&p) = prev.get(path.last()?) {
                path.push(p);
            }
            path.reverse();
            return Some(path);
        }
        if d > dist[&node] {
            continue;
        }
        for &(next, len) in adjacency.get(&node).into_iter().flatten() {
            let nd = d + len.max(0.0);
            if dist.get(&next).is_none_or(|&old| nd < old) {
                dist.insert(next, nd);
                prev.insert(next, node);
                heap.push((key(nd), next));
            }
        }
    }
    None
}

impl Store {
    /// Shortest path (by arc length) through every waypoint in turn, over the STG plus
    /// `extra` edges (`(u, v, length_um)`, e.g. added edges), as node ids. `None` when some
    /// leg has no path.
    pub fn route(&self, waypoints: &[i64], extra: &[(i64, i64, f32)]) -> Option<Vec<i64>> {
        let mut adjacency: HashMap<i64, Vec<(i64, f32)>> = HashMap::new();
        let edges = self.edge_nodes.iter().enumerate().map(|(e, &[u, v])| (u, v, self.edge_length_um[e]));
        for (u, v, len) in edges.chain(extra.iter().copied()) {
            adjacency.entry(u).or_default().push((v, len));
            adjacency.entry(v).or_default().push((u, len));
        }
        let mut path = vec![*waypoints.first()?];
        for leg in waypoints.windows(2) {
            let piece = dijkstra(&adjacency, leg[0], leg[1])?;
            path.extend(&piece[1..]);
        }
        Some(path)
    }

    /// Geometry of a node path: each step follows its edge's arc (straight when the step is
    /// an added edge or unknown).
    pub fn path_polyline(&self, nodes: &[i64]) -> Vec<[f32; 2]> {
        let pos = |id: &i64| self.node_index.get(id).map(|&n| self.node_pos[n]);
        let mut out: Vec<[f32; 2]> = nodes.first().and_then(pos).into_iter().collect();
        for step in nodes.windows(2) {
            let (a, b) = (step[0], step[1]);
            match self.edge_by_nodes.get(&(a.min(b), a.max(b))) {
                Some(&e) => {
                    let arc = &self.arcs.points[self.arcs.offsets[e]..self.arcs.offsets[e + 1]];
                    if self.edge_nodes[e][0] == a {
                        out.extend(arc.iter().skip(1));
                    } else {
                        out.extend(arc.iter().rev().skip(1));
                    }
                }
                None => out.extend(pos(&b)),
            }
        }
        out
    }
}
