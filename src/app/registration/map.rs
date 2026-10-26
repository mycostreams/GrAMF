//! Registration on the map: the placed videos overlay, and what a click on the map selects or changes.

use super::*;

/// Map text: position, text, colour.
pub type MapText = (Pos2, String, Color32);

/// A run's matched graph and its frame outline in µm, closed (5 corners): the same chain as
/// the video edges (place, nudge, snap).
fn frame_world(reg: &mut Registration, p: usize, run: usize, sim: &Similarity, store: &Store, added: &[AddedSegment]) -> Option<(Rc<RunMatch>, [[f32; 2]; 5])> {
    let m = reg.run_match(p, run, sim, store, added)?;
    let pos = &reg.videos.positions[p];
    let (stage, frame, dx) = (pos.stage_mm, pos.runs[run].frame_size, pos.runs[run].delta_x_um);
    let (camera, video_scale, rigid) = (reg.camera(), reg.video_scale(), m.snap.unwrap_or_default());
    let place = |q: [f32; 2]| {
        let w = placement::place(sim, camera, video_scale, stage, q);
        rigid.apply([w[0] + m.nudge[0], w[1] + m.nudge[1]])
    };
    let (hw, hh) = (frame[1] as f32 * dx / 2.0, frame[0] as f32 * dx / 2.0);
    let corners = [[-hw, -hh], [hw, -hh], [hw, hh], [-hw, hh], [-hw, -hh]].map(place);
    Some((m, corners))
}

/// [`frame_world`] in points.
fn run_frame(reg: &mut Registration, p: usize, run: usize, sim: &Similarity, store: &Store, added: &[AddedSegment], to_pt: &impl Fn([f32; 2]) -> Pos2) -> Option<(Rc<RunMatch>, Vec<Pos2>)> {
    let (m, corners) = frame_world(reg, p, run, sim, store, added)?;
    Some((m, corners.iter().map(|&q| to_pt(q)).collect()))
}

/// True when `q` lies inside the closed convex outline `c` (either winding).
fn inside(c: &[[f32; 2]; 5], q: [f32; 2]) -> bool {
    let side = |a: [f32; 2], b: [f32; 2]| (b[0] - a[0]) * (q[1] - a[1]) - (b[1] - a[1]) * (q[0] - a[0]);
    let s: Vec<f32> = c.windows(2).map(|w| side(w[0], w[1])).collect();
    s.iter().all(|&x| x >= 0.0) || s.iter().all(|&x| x <= 0.0)
}

impl Registration {
    /// The selected run's frame outline in µm, when its session is placed.
    pub fn selected_frame(&mut self, store: &Store, added: &[AddedSegment]) -> Option<[[f32; 2]; 5]> {
        let (p, run) = (self.position?, self.run);
        let sim = self.fits().get(self.session)?.as_ref()?.sim;
        frame_world(self, p, run, &sim, store, added).map(|(_, c)| c)
    }

    /// The video frame under world point `at` among those drawn (each position's shown run,
    /// and the selected position's other runs): the selected run when it is one of them, else
    /// the highest run number. `(position, run)`.
    pub fn frame_at(&mut self, at: [f32; 2], store: &Store, added: &[AddedSegment]) -> Option<(usize, usize)> {
        let sim = self.fits().get(self.session)?.as_ref()?.sim;
        let mut hits = vec![];
        for p in self.sessions[self.session].positions.clone() {
            let runs: Vec<usize> = if self.position == Some(p) {
                (0..self.videos.positions[p].runs.len()).collect()
            } else {
                vec![self.shown_run(p)]
            };
            for run in runs {
                if frame_world(self, p, run, &sim, store, added).is_some_and(|(_, c)| inside(&c, at)) {
                    hits.push((p, run));
                }
            }
        }
        if let Some(&hit) = hits.iter().find(|&&(p, run)| self.position == Some(p) && self.run == run) {
            return Some(hit);
        }
        hits.into_iter().max_by_key(|&(p, run)| self.videos.positions[p].runs[run].run_nr.unwrap_or(i64::MIN))
    }
}

/// "50 · 1/2": the run number and which of the position's runs this is.
fn run_label(reg: &Registration, p: usize, run: usize) -> String {
    let runs = &reg.videos.positions[p].runs;
    let nr = runs[run].run_nr.map_or("run".to_string(), |n| n.to_string());
    if runs.len() > 1 { format!("{nr} · {}/{}", run + 1, runs.len()) } else { nr }
}

/// Placed videos of the selected session, in points: each shown run's snapped graph and frame
/// outline, labelled with its run; for the selected position its other runs in grey, and every
/// video edge in its own colour, with its STG chain in the same colour and its gaps dashed;
/// anchors with a line to their video's centre.
pub fn overlay(reg: &mut Registration, store: &Store, added: &[AddedSegment], to_pt: impl Fn([f32; 2]) -> Pos2) -> (Vec<egui::Shape>, Vec<MapText>) {
    use egui::Shape;
    let fits = reg.fits();
    let Some(Some(fit)) = fits.get(reg.session) else {
        return (vec![], vec![]);
    };
    let sim = fit.sim;
    if let Some(p) = reg.position {
        reg.last_run.insert(p, reg.run);
    }
    let (mut shapes, mut texts) = (vec![], vec![]);
    for p in reg.sessions[reg.session].positions.clone() {
        let run = reg.shown_run(p);
        let stage = reg.videos.positions[p].stage_mm;
        let selected = reg.position == Some(p);
        let faint = Color32::from_rgba_unmultiplied(150, 190, 230, 150);
        if selected {
            // the position's other runs, in grey, so they do not vanish behind the selected one
            let grey = Color32::from_rgba_unmultiplied(170, 170, 170, 110);
            for other in (0..reg.videos.positions[p].runs.len()).filter(|&r| r != run) {
                let Some((m, corners)) = run_frame(reg, p, other, &sim, store, added, &to_pt) else { continue };
                shapes.extend(Shape::dashed_line(&corners, Stroke::new(1.0, grey), 2.0, 4.0));
                for pts in &m.placed {
                    shapes.push(Shape::line(pts.iter().map(|&q| to_pt(q)).collect(), Stroke::new(1.0, grey)));
                }
                texts.push((corners[0], run_label(reg, p, other), grey));
            }
        }
        let Some((m, corners)) = run_frame(reg, p, run, &sim, store, added, &to_pt) else { continue };
        let outline = if selected { Color32::from_rgb(80, 220, 240) } else { faint };
        shapes.extend(Shape::dashed_line(&corners, Stroke::new(1.0, outline), 4.0, 3.0));
        texts.push((corners[0], run_label(reg, p, run), outline));
        for (i, (pts, cover)) in m.placed.iter().zip(&m.covers).enumerate() {
            let line: Vec<Pos2> = pts.iter().map(|&q| to_pt(q)).collect();
            let chosen = reg.video_edge == Some(cover.key);
            if m.disabled[i] {
                // an artifact: dashed red with a cross at its middle, and no chain (no flow)
                let alpha = if selected { 255 } else { 120 };
                let red = Color32::from_rgba_unmultiplied(235, 70, 70, alpha);
                shapes.extend(Shape::dashed_line(&line, Stroke::new(if chosen { 3.5 } else { 2.5 }, red), 4.0, 4.0));
                if selected {
                    let mid = line[line.len() / 2];
                    let r = 6.0;
                    shapes.push(Shape::line_segment([mid + egui::vec2(-r, -r), mid + egui::vec2(r, r)], Stroke::new(2.5, red)));
                    shapes.push(Shape::line_segment([mid + egui::vec2(-r, r), mid + egui::vec2(r, -r)], Stroke::new(2.5, red)));
                }
                continue;
            }
            if !selected {
                shapes.push(Shape::line(line, Stroke::new(1.5, faint)));
                continue;
            }
            let colour = EDGE_COLOURS[i % EDGE_COLOURS.len()];
            for &(e, _) in &cover.chain {
                let stg: Vec<Pos2> = match e {
                    EdgeRef::Pipeline(e) => store.arcs.points[store.arcs.offsets[e]..store.arcs.offsets[e + 1]].iter().map(|&q| to_pt(q)).collect(),
                    EdgeRef::Added(id) => added.iter().filter(|a| a.id == id).flat_map(|a| [to_pt(a.a), to_pt(a.b)]).collect(),
                };
                let a = if chosen { 200 } else { 110 };
                shapes.push(Shape::line(stg, Stroke::new(if chosen { 6.0 } else { 4.0 }, Color32::from_rgba_unmultiplied(colour.r(), colour.g(), colour.b(), a))));
            }
            shapes.push(Shape::line(line, Stroke::new(if chosen { 3.5 } else { 2.0 }, colour)));
            for gap in &cover.gaps {
                let g: Vec<Pos2> = gap.iter().map(|&q| to_pt(q)).collect();
                shapes.extend(Shape::dashed_line(&g, Stroke::new(3.0, GAP), 5.0, 4.0));
            }
        }
        if let Some(&a) = reg.anchors.get(&p) {
            let centre = to_pt({
                let c = sim.apply(stage);
                [c[0] as f32, c[1] as f32]
            });
            let at = to_pt([a[0] as f32, a[1] as f32]);
            shapes.push(Shape::line_segment([centre, at], Stroke::new(1.5, ANCHORED)));
            shapes.push(Shape::circle_stroke(at, 6.0, Stroke::new(2.0, ANCHORED)));
        }
    }
    (shapes, texts)
}

/// A click with the Select tool: the selected position's video edge under `at` (µm) when
/// there is one, else the position whose placed video passes nearest.
pub fn pick(reg: &mut Registration, store: &Store, added: &[AddedSegment], at: [f32; 2], radius: f32) {
    let fits = reg.fits();
    let Some(Some(fit)) = fits.get(reg.session) else { return };
    let sim = fit.sim;
    let d = |pts: &[[f32; 2]]| pts.windows(2).map(|w| crate::store::segment_dist2(at, w[0], w[1]).sqrt()).fold(f32::INFINITY, f32::min);
    if let Some(p) = reg.position {
        let run = reg.shown_run(p);
        if let Some(m) = reg.run_match(p, run, &sim, store, added) {
            let hit = m.placed.iter().zip(&m.covers).map(|(pts, c)| (d(pts), c.key)).filter(|x| x.0 <= radius).min_by(|a, b| a.0.total_cmp(&b.0));
            if let Some((_, key)) = hit {
                reg.video_edge = Some(key);
                return;
            }
        }
    }
    let mut best: Option<(f32, usize)> = None;
    for p in reg.sessions[reg.session].positions.clone() {
        let run = reg.shown_run(p);
        let Some(m) = reg.run_match(p, run, &sim, store, added) else { continue };
        let dist = m.placed.iter().map(|pts| d(pts)).fold(f32::INFINITY, f32::min);
        if dist <= radius && best.is_none_or(|(bd, _)| dist < bd) {
            best = Some((dist, p));
        }
    }
    if let Some((_, p)) = best {
        if reg.position != Some(p) {
            reg.position = Some(p);
            reg.run = reg.shown_run(p);
            reg.video_edge = None;
        }
    }
}

/// A video edge of the selected run near `at` (µm), within `radius`.
pub fn video_edge_at(reg: &mut Registration, store: &Store, added: &[AddedSegment], at: [f32; 2], radius: f32) -> Option<[i64; 3]> {
    let p = reg.position?;
    let fits = reg.fits();
    let sim = fits.get(reg.session)?.as_ref()?.sim;
    let m = reg.run_match(p, reg.shown_run(p), &sim, store, added)?;
    let d = |pts: &[[f32; 2]]| pts.windows(2).map(|w| crate::store::segment_dist2(at, w[0], w[1]).sqrt()).fold(f32::INFINITY, f32::min);
    m.placed
        .iter()
        .zip(&m.covers)
        .map(|(pts, c)| (d(pts), c.key))
        .filter(|x| x.0 <= radius)
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|x| x.1)
}

/// The selected video edge's chain with `edge` added (in order along the video edge, with
/// its direction from the shared tangent) or removed if it was there.
pub fn toggle_in_chain(reg: &mut Registration, store: &Store, added: &[AddedSegment], edge: EdgeRef) -> Option<Op> {
    let (p, key) = (reg.position?, reg.video_edge?);
    let fits = reg.fits();
    let sim = fits.get(reg.session)?.as_ref()?.sim;
    let run = reg.shown_run(p);
    let m = reg.run_match(p, run, &sim, store, added)?;
    let i = m.covers.iter().position(|c| c.key == key)?;
    let mut chain = m.covers[i].chain.clone();
    let video = &m.placed[i];
    let rel = reg.run_path(p, run);
    if let Some(at) = chain.iter().position(|(e, _)| *e == edge) {
        chain.remove(at);
        return Some(Op::SetAssignment((rel, key), Some(chain)));
    }
    // geometry of the STG edge: its polyline, node1 -> node2
    let stg: Vec<[f32; 2]> = match edge {
        EdgeRef::Pipeline(e) => {
            let arc = store.arcs.points[store.arcs.offsets[e]..store.arcs.offsets[e + 1]].to_vec();
            if store.arc_reversed(e) { arc.into_iter().rev().collect() } else { arc }
        }
        EdgeRef::Added(id) => added.iter().find(|a| a.id == id).map(|a| vec![a.a, a.b])?,
    };
    let mid = stg[stg.len() / 2];
    // where along the video edge it sits, and which way the video runs there
    let along = |q: [f32; 2]| {
        (0..video.len().saturating_sub(1))
            .min_by(|&a, &b| {
                crate::store::segment_dist2(q, video[a], video[a + 1]).total_cmp(&crate::store::segment_dist2(q, video[b], video[b + 1]))
            })
            .unwrap_or(0)
    };
    let k = along(mid);
    let t = [video[k + 1][0] - video[k][0], video[k + 1][1] - video[k][1]];
    let j = (0..stg.len() - 1)
        .min_by(|&a, &b| crate::store::segment_dist2(mid, stg[a], stg[a + 1]).total_cmp(&crate::store::segment_dist2(mid, stg[b], stg[b + 1])))
        .unwrap_or(0);
    let s = [stg[j + 1][0] - stg[j][0], stg[j + 1][1] - stg[j][1]];
    let dir: i8 = if t[0] * s[0] + t[1] * s[1] >= 0.0 { 1 } else { -1 };
    let mid_of = |e: &EdgeRef| -> [f32; 2] {
        match *e {
            EdgeRef::Pipeline(e) => store.arcs.points[(store.arcs.offsets[e] + store.arcs.offsets[e + 1]) / 2],
            EdgeRef::Added(id) => added.iter().find(|a| a.id == id).map_or([0.0; 2], |a| [(a.a[0] + a.b[0]) / 2.0, (a.a[1] + a.b[1]) / 2.0]),
        }
    };
    let at = chain.iter().position(|(e, _)| along(mid_of(e)) > k).unwrap_or(chain.len());
    chain.insert(at, (edge, dir));
    Some(Op::SetAssignment((rel, key), Some(chain)))
}

#[cfg(test)]
mod tests {
    #[test]
    fn inside_a_rotated_frame() {
        let c = [[0.0, -10.0], [10.0, 0.0], [0.0, 10.0], [-10.0, 0.0], [0.0, -10.0]];
        assert!(super::inside(&c, [0.0, 0.0]) && super::inside(&c, [4.0, 4.0]));
        assert!(!super::inside(&c, [8.0, 8.0]));
        let reversed = [c[4], c[3], c[2], c[1], c[0]];
        assert!(super::inside(&reversed, [1.0, -2.0]));
    }
}
