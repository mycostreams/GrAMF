//! Shapes drawn over the map in points: trails, tool feedback, arc ends far from their node, and
//! id labels.

use super::*;

const TRAIL: egui::Color32 = egui::Color32::from_rgb(235, 90, 210);

/// Every visible author's trails, in points: the routed path (straight dashed legs when no
/// path exists) and waypoints as shapes, plus each trail's name at its first waypoint as a
/// map text. The active trail is thicker.
pub(super) fn trail_overlay(
    store: &Store,
    st: &AnnotationState,
    routes: &HashMap<(String, String), (Vec<i64>, usize, Option<Vec<i64>>)>,
    renderer: &AppRenderer,
    ppp: f32,
) -> (Vec<egui::Shape>, Vec<(egui::Pos2, String, egui::Color32)>) {
    use egui::{Color32, Shape, Stroke};
    let to_pt = |p: [f32; 2]| {
        let [x, y] = renderer.world_to_screen(p);
        egui::pos2(x / ppp, y / ppp)
    };
    let pos = |id: &i64| store.node_index.get(id).map(|&n| store.node_pos[n]);
    let (mut shapes, mut texts) = (vec![], vec![]);
    let layers = std::iter::once((&st.own, st.own_visible, true)).chain(st.others.iter().map(|(a, v)| (a, *v, false)));
    for (a, _, own) in layers.filter(|l| l.1) {
        let colour = if own { TRAIL } else { Color32::from_rgba_unmultiplied(235, 90, 210, 110) };
        for (name, waypoints) in &a.trails {
            let active = own && st.active_trail.as_deref() == Some(name.as_str());
            let stroke = Stroke::new(if active { 4.0 } else if own { 3.0 } else { 2.0 }, colour);
            match routes.get(&(a.author.clone(), name.clone())).and_then(|r| r.2.as_ref()) {
                Some(path) => shapes.push(Shape::line(store.path_polyline(path).into_iter().map(to_pt).collect(), stroke)),
                None => {
                    let points: Vec<egui::Pos2> = waypoints.iter().filter_map(pos).map(to_pt).collect();
                    shapes.extend(Shape::dashed_line(&points, stroke, 5.0, 5.0));
                }
            }
            for p in waypoints.iter().filter_map(pos) {
                shapes.push(Shape::circle_filled(to_pt(p), if active { 5.0 } else { 3.5 }, colour));
            }
            if let Some(first) = waypoints.first().and_then(pos) {
                texts.push((to_pt(first) + egui::vec2(8.0, -10.0), name.clone(), colour));
            }
        }
    }
    (shapes, texts)
}

/// A thin dashed line from each arc end that stops short of its node to that node, for edges
/// that have appeared, clipped to the view so a long gap seen up close stays a few dashes.
pub(super) fn detached_overlay(store: &Store, renderer: &AppRenderer, to_pt: impl Fn([f32; 2]) -> egui::Pos2) -> Vec<egui::Shape> {
    let (min, max) = renderer.visible_world();
    let stroke = egui::Stroke::new(1.0, egui::Color32::from_rgb(240, 170, 60));
    let mut shapes = vec![];
    for d in store.detached_ends() {
        if store.edge_t[d.edge] > renderer.time {
            continue;
        }
        if let Some((p, q)) = clip_segment(store.arcs.points[d.point], store.node_pos[d.node], min, max) {
            shapes.extend(egui::Shape::dashed_line(&[to_pt(p), to_pt(q)], stroke, 4.0, 3.0));
        }
    }
    shapes
}

/// The part of segment `p`-`q` inside the box `min`..`max` (Liang-Barsky), if any.
fn clip_segment(p: [f32; 2], q: [f32; 2], min: [f32; 2], max: [f32; 2]) -> Option<([f32; 2], [f32; 2])> {
    let (mut t0, mut t1) = (0.0f32, 1.0f32);
    for k in 0..2 {
        let d = q[k] - p[k];
        for (num, den) in [(p[k] - min[k], -d), (max[k] - p[k], d)] {
            if den == 0.0 {
                if num < 0.0 {
                    return None;
                }
            } else {
                let t = num / den;
                if den < 0.0 { t0 = t0.max(t) } else { t1 = t1.min(t) }
            }
        }
    }
    let at = |t: f32| [p[0] + (q[0] - p[0]) * t, p[1] + (q[1] - p[1]) * t];
    (t0 <= t1).then(|| (at(t0), at(t1)))
}

/// Feedback for the current tool, in points: node dots and the node under the cursor for node
/// tools, a rubber band from the first node while adding an edge, and the box being dragged.
pub(super) fn tool_overlay(
    store: &Store,
    st: &AnnotationState,
    renderer: &AppRenderer,
    ppp: f32,
    cursor: [f32; 2],
    box_start: Option<[f32; 2]>,
) -> Vec<egui::Shape> {
    use egui::{Color32, Shape, Stroke};
    let to_pt = |p: [f32; 2]| {
        let [x, y] = renderer.world_to_screen(p);
        egui::pos2(x / ppp, y / ppp)
    };
    let cursor_pt = egui::pos2(cursor[0] / ppp, cursor[1] / ppp);
    let mut shapes = vec![];
    let t = renderer.time;

    if st.tool.needs_nodes() {
        let (min, max) = renderer.visible_world();
        let visible: Vec<usize> = (0..store.node_pos.len())
            .filter(|&n| store.node_t[n] <= t)
            .filter(|&n| {
                let p = store.node_pos[n];
                p[0] >= min[0] && p[0] <= max[0] && p[1] >= min[1] && p[1] <= max[1]
            })
            .take(MAX_NODE_DOTS + 1)
            .collect();
        if visible.len() <= MAX_NODE_DOTS {
            let dot = Color32::from_rgba_unmultiplied(220, 220, 220, 160);
            shapes.extend(visible.iter().map(|&n| Shape::circle_filled(to_pt(store.node_pos[n]), 2.0, dot)));
        }
        let at = renderer.screen_to_world(cursor);
        let hover = store.pick_node(at, NODE_PICK_RADIUS_PX * renderer.um_per_px(), t);
        let accent = Color32::from_rgb(60, 210, 230);
        if let Some(n) = hover {
            shapes.push(Shape::circle_stroke(to_pt(store.node_pos[n]), 7.0, Stroke::new(2.0, accent)));
        }
        let trail_end = (st.tool == Tool::Trail)
            .then(|| st.active_trail.as_ref().and_then(|t| st.own.trails.get(t)?.last()))
            .flatten()
            .and_then(|id| store.node_index.get(id).copied());
        if let Some(last) = trail_end {
            let from = to_pt(store.node_pos[last]);
            let to = hover.map_or(cursor_pt, |n| to_pt(store.node_pos[n]));
            shapes.extend(Shape::dashed_line(&[from, to], Stroke::new(2.0, TRAIL), 6.0, 4.0));
        }
        if let (Tool::AddEdge, Some(first)) = (st.tool, st.first) {
            let from = to_pt(store.node_pos[first]);
            let to = hover.map_or(cursor_pt, |n| to_pt(store.node_pos[n]));
            shapes.push(Shape::circle_filled(from, 4.0, accent));
            shapes.extend(Shape::dashed_line(&[from, to], Stroke::new(2.0, accent), 6.0, 4.0));
        }
    }
    if let (Tool::BoxSelect, Some(start)) = (st.tool, box_start) {
        let rect = egui::Rect::from_two_pos(to_pt(start), cursor_pt);
        shapes.push(Shape::rect_filled(rect, 0.0, Color32::from_rgba_unmultiplied(60, 210, 230, 30)));
        shapes.push(Shape::rect_stroke(rect, 0.0, Stroke::new(1.5, Color32::from_rgb(60, 210, 230)), egui::StrokeKind::Inside));
    }
    shapes
}

/// Id labels inside the view for items that have appeared by the current time, in points;
/// `None` when more than [`MAX_LABELS`] would be drawn.
pub(super) fn id_labels(store: &Store, layers: &ui::Layers, renderer: &AppRenderer, ppp: f32) -> Option<Vec<(egui::Pos2, String)>> {
    let (min, max) = renderer.visible_world();
    let inside = |p: [f32; 2]| p[0] >= min[0] && p[0] <= max[0] && p[1] >= min[1] && p[1] <= max[1];
    let t = renderer.time;
    let mut items: Vec<([f32; 2], String)> = Vec::new();
    if layers.node_ids {
        for n in (0..store.node_pos.len()).filter(|&n| store.node_t[n] <= t && inside(store.node_pos[n])) {
            items.push((store.node_pos[n], store.node_ids[n].to_string()));
            if items.len() > MAX_LABELS {
                return None;
            }
        }
    }
    if layers.hyperedge_ids {
        for (h, p, ht) in &store.hyperedge_anchors {
            if *ht <= t && inside(*p) {
                items.push((*p, format!("H{h}")));
                if items.len() > MAX_LABELS {
                    return None;
                }
            }
        }
    }
    Some(
        items
            .into_iter()
            .map(|(p, text)| {
                let [x, y] = renderer.world_to_screen(p);
                (egui::pos2(x / ppp, y / ppp), text)
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    #[test]
    fn clip_segment_to_box() {
        let (min, max) = ([0.0, 0.0], [10.0, 10.0]);
        assert_eq!(super::clip_segment([-10.0, 5.0], [20.0, 5.0], min, max), Some(([0.0, 5.0], [10.0, 5.0])));
        assert_eq!(super::clip_segment([2.0, 2.0], [3.0, 3.0], min, max), Some(([2.0, 2.0], [3.0, 3.0])));
        assert_eq!(super::clip_segment([-5.0, 20.0], [20.0, 20.0], min, max), None);
        assert_eq!(super::clip_segment([-5.0, -5.0], [-1.0, 15.0], min, max), None);
    }
}
