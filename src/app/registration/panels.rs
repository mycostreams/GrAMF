//! The registration side panels: sessions, the stage scatter, the run, its video edges and
//! corrections, and the toolbar.

use super::*;

fn utc(t: i64) -> String {
    chrono::DateTime::from_timestamp(t, 0).map_or(t.to_string(), |d| d.format("%d %b %H:%M").to_string())
}

/// Left panel: where the runs come from, the sessions, and the stage scatter of the selected
/// session. Without runs, a button to pick the folder.
pub fn left_panel(ui: &mut egui::Ui, reg: Option<&mut Registration>, store: &Store, added: &[AddedSegment]) -> Action {
    let mut action = Action::None;
    ui.heading("Video runs");
    let Some(reg) = reg else {
        ui.label(egui::RichText::new("Choose the folder with this plate's video runs.").weak());
        if ui.button("Choose video runs folder…").clicked() {
            action = Action::ChooseFolder;
        }
        flow_layer_note(ui, store);
        return action;
    };
    let n_runs: usize = reg.videos.positions.iter().map(|p| p.runs.len()).sum();
    ui.label(format!("{} positions · {n_runs} runs · {}", reg.videos.positions.len(), reg.videos.setup.name()));
    if !reg.videos.skipped.is_empty() {
        ui.colored_label(Color32::YELLOW, format!("{} run folders skipped", reg.videos.skipped.len()))
            .on_hover_text(reg.videos.skipped.join("\n"));
    }
    if ui.small_button("Other folder…").on_hover_text(reg.videos.root.display().to_string()).clicked() {
        action = Action::ChooseFolder;
    }
    flow_layer_note(ui, store);

    ui.separator();
    ui.strong("Sessions");
    let fits = reg.fits();
    for (i, s) in reg.sessions.iter().enumerate() {
        let status = session_status(reg, i, fits[i].as_ref());
        let label = format!("{} – {} · {} pos · {status}", utc(s.start), utc(s.end), s.positions.len());
        if ui.selectable_label(reg.session == i, label).clicked() && reg.session != i {
            reg.session = i;
            reg.position = None;
        }
    }
    if let Some(Some(fit)) = fits.get(reg.session) {
        session_details(ui, reg, fit, store, &mut action);
        runs_without_flow(ui, reg, store, added, &mut action);
    }

    ui.separator();
    ui.strong("Stage positions");
    scatter(ui, reg, &fits);
    ui.horizontal_wrapped(|ui| {
        // painted, not a glyph: the UI font has no "●"
        let dot = |ui: &mut egui::Ui, c: Color32, text: &str| {
            let (rect, _) = ui.allocate_exact_size(egui::vec2(10.0, 10.0), egui::Sense::hover());
            ui.painter().circle_filled(rect.center(), 4.0, c);
            ui.label(egui::RichText::new(text).small().weak());
        };
        dot(ui, ANCHORED, "anchored");
        dot(ui, PLACED, "placed");
        dot(ui, UNPLACED, "not placed");
        dot(ui, MAPPED, "mapped in store");
    });
    action
}

fn session_status(reg: &Registration, i: usize, fit: Option<&SessionFit>) -> String {
    let Some(fit) = fit else {
        return "not placed".into();
    };
    let verified = reg.session_key(i).is_some_and(|k| reg.layer.verified.contains(&k));
    let how = match (fit.inherited, fit.residuals.len()) {
        (true, _) => "from previous session".to_string(),
        (false, 0) => "setup default".to_string(),
        (false, 1) => "1 anchor".to_string(),
        (false, n) => format!("{n} anchors"),
    };
    format!("{how} · {}", if verified { "verified" } else { "unverified" })
}

/// The selected session's fit: score, anchors with residuals, and its buttons.
/// The selected session's runs that give no flow in its placement, each a button that selects
/// the run and zooms to it: a run whose frame landed off its hyphae is otherwise easy to miss.
fn runs_without_flow(ui: &mut egui::Ui, reg: &mut Registration, store: &Store, added: &[AddedSegment], action: &mut Action) {
    let missing = reg.runs_without_flow(store, added);
    let n: usize = reg.sessions[reg.session].positions.iter().map(|&p| reg.videos.positions[p].runs.len()).sum();
    if missing.is_empty() {
        ui.label(egui::RichText::new(format!("all {n} runs give flow")).weak().small());
        return;
    }
    ui.colored_label(Color32::YELLOW, format!("{} of {n} runs give no flow", missing.len())).on_hover_text(
        "no video edge of the run lies on the STG in this placement (or its graph or flow file could not be read): Publish leaves it out",
    );
    for (p, run) in missing {
        if ui.small_button(reg.run_path(p, run)).on_hover_text("select this run and zoom to it").clicked() {
            reg.position = Some(p);
            reg.run = run;
            *action = Action::ZoomRun;
        }
    }
}

fn session_details(ui: &mut egui::Ui, reg: &mut Registration, fit: &SessionFit, store: &Store, action: &mut Action) {
    let i = reg.session;
    let score = reg.score(i, fit, store);
    egui::Frame::group(ui.style()).show(ui, |ui| {
        ui.label(format!(
            "scale {:.1} µm/mm · rotation {:.2}° · {}mirrored · camera {}",
            fit.sim.scale,
            fit.sim.theta.to_degrees(),
            if fit.sim.mirror { "" } else { "not " },
            reg.camera()
        ));
        match score {
            Some(s) => ui.label(format!("fit score {s:.1} µm"))
                .on_hover_text("mean distance from the placed video graphs to the nearest STG arc (capped at 50 µm); lower is better"),
            None => ui.label(egui::RichText::new("fit score: no video graphs").weak()),
        };
        for &(p, r) in &fit.residuals {
            ui.horizontal(|ui| {
                ui.label(format!("{} · {r:.1} µm", reg.videos.positions[p].name));
                if ui.small_button("remove").clicked() {
                    let name = reg.videos.positions[p].name.clone();
                    reg.pending.push(Op::SetAnchor(name, None));
                }
            });
        }
        let key = reg.session_key(i).unwrap_or_default();
        let mut verified = reg.layer.verified.contains(&key);
        if ui.checkbox(&mut verified, "Verified").on_hover_text("you checked that these videos sit on the right hyphae").changed() {
            reg.pending.push(Op::SetVerified(key, verified));
        }
        // Network-scale calibration: video graphs are true µm, the STG is in the µm its pixel
        // size made it (PRINCE at nominal 2x reads about 1.15x long).
        let explain = "STG µm per video µm. Scales every video graph and frame about its centre. \
            Corrects a network whose pixel size is mis-calibrated (the session's fit scale / 1000 when \
            the stage and the videos are right). Saved with the registration and with the setup default.";
        let mut k = reg.video_scale();
        ui.horizontal(|ui| {
            ui.label("video scale").on_hover_text(explain);
            if ui.add(egui::DragValue::new(&mut k).speed(0.001).range(0.5..=2.0).fixed_decimals(4)).on_hover_text(explain).changed() {
                reg.pending.push(Op::SetVideoScale(Some(k)));
            }
        });
        let from_fit = (fit.sim.scale / 1000.0) as f32;
        if ui
            .add_enabled(fit.residuals.len() >= 2, egui::Button::new(format!("Set from fit scale ({from_fit:.4})")).small())
            .on_hover_text("video scale = this session's fit scale / 1000 (needs 2 or more anchors)")
            .clicked()
        {
            reg.pending.push(Op::SetVideoScale(Some(from_fit)));
        }
        // stacked, not side by side: a wide row would widen the whole panel
        ui.vertical(|ui| {
            if ui.small_button("Detect camera orientation").on_hover_text("tries the 8 rotations / mirrors of the camera and keeps the best fit").clicked() {
                *action = Action::DetectCamera;
            }
            if fit.residuals.len() >= 2 && ui.small_button(format!("Save as {} default", reg.videos.setup.name())).clicked() {
                *action = Action::SaveDefault;
            }
        });
    });
}





fn flow_layer_note(ui: &mut egui::Ui, store: &Store) {
    if let Some(f) = &store.flow_in_store {
        ui.label(
            egui::RichText::new(format!(
                "store flow layer: {} runs, {} measurements ({}), read-only",
                f.run_paths.len(),
                f.n_measurements,
                if f.written_by.is_empty() { "unknown writer" } else { &f.written_by }
            ))
            .small()
            .weak(),
        );
    }
}

/// Positions of the selected session, oriented like the map; click selects, scroll zooms,
/// drag pans, double-click resets. Run numbers show where a dot has room for them.
fn scatter(ui: &mut egui::Ui, reg: &mut Registration, fits: &[Option<SessionFit>]) {
    let Some(session) = reg.sessions.get(reg.session) else {
        return;
    };
    let size = egui::vec2(ui.available_width(), 170.0);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click_and_drag());
    if reg.scatter_view.0 != reg.session || response.double_clicked() {
        reg.scatter_view = (reg.session, 1.0, egui::Vec2::ZERO);
    }
    let (_, zoom, pan) = &mut reg.scatter_view;
    *pan += response.drag_delta();
    if let Some(cursor) = response.hover_pos() {
        let scroll = ui.input(|i| i.smooth_scroll_delta.y);
        if scroll != 0.0 {
            let factor = (scroll * 0.005).exp().clamp(0.5, 2.0);
            let new_zoom = (*zoom * factor).clamp(1.0, 200.0);
            // keep the point under the cursor in place
            let from_centre = cursor - rect.center() - *pan;
            *pan += from_centre - from_centre * (new_zoom / *zoom);
            *zoom = new_zoom;
        }
    }
    let (zoom, pan) = (reg.scatter_view.1 as f64, reg.scatter_view.2);
    let painter = ui.painter_at(rect);
    painter.rect_stroke(rect, 4.0, Stroke::new(0.5, ui.visuals().weak_text_color()), egui::StrokeKind::Inside);

    // Drawn the way the map shows them: placed positions where the session's placement puts
    // them (STG axes, rows down); unplaced sessions in the orientation a fit would start from
    // (the setup default's mirror and rotation, else none).
    let fit = fits.get(reg.session).and_then(|f| f.as_ref()).map(|f| f.sim);
    let base = reg.default.unwrap_or_default();
    let stage = |p: usize| {
        let s = reg.videos.positions[p].stage_mm;
        fit.map_or(base.orient(s), |sim| sim.apply(s))
    };
    let (mut min, mut max) = ([f64::MAX; 2], [f64::MIN; 2]);
    for &p in &session.positions {
        for k in 0..2 {
            min[k] = min[k].min(stage(p)[k]);
            max[k] = max[k].max(stage(p)[k]);
        }
    }
    // equal scale on both axes, centred, with a margin
    let inner = rect.shrink(14.0);
    let span = (max[0] - min[0]).max(max[1] - min[1]).max(1e-3);
    let scale = (inner.width().min(inner.height()) as f64) / span * zoom;
    let mid = [(min[0] + max[0]) / 2.0, (min[1] + max[1]) / 2.0];
    let to_screen = |s: [f64; 2]| {
        Pos2::new(
            inner.center().x + pan.x + ((s[0] - mid[0]) * scale) as f32,
            inner.center().y + pan.y + ((s[1] - mid[1]) * scale) as f32,
        )
    };

    let screen: Vec<Pos2> = session.positions.iter().map(|&p| to_screen(stage(p))).collect();
    let mut hovered = None;
    for (i, &p) in session.positions.iter().enumerate() {
        let at = screen[i];
        if !rect.expand(8.0).contains(at) {
            continue;
        }
        let colour = if reg.anchors.contains_key(&p) {
            ANCHORED
        } else if fits.get(reg.session).is_some_and(|f| f.is_some()) {
            PLACED
        } else {
            UNPLACED
        };
        painter.circle_filled(at, 5.0, colour);
        if reg.position_mapped(p) {
            painter.circle_stroke(at, 6.5, Stroke::new(1.5, MAPPED));
        }
        if reg.position == Some(p) {
            painter.circle_stroke(at, 8.0, Stroke::new(2.0, Color32::from_rgb(55, 138, 221)));
        }
        if response.hover_pos().is_some_and(|h| h.distance(at) < 9.0) {
            hovered = Some(p);
        }
        // run numbers, once the nearest other dot is far enough away to fit them
        let room = screen.iter().enumerate().filter(|&(j, _)| j != i).map(|(_, q)| q.distance(at)).fold(f32::MAX, f32::min);
        if room > 34.0 {
            if let Some(label) = run_numbers(&reg.videos.positions[p].runs) {
                painter.text(at + egui::vec2(0.0, 8.0), egui::Align2::CENTER_TOP, label, egui::FontId::proportional(10.0), ui.visuals().text_color());
            }
        }
    }
    if let Some(p) = hovered {
        let pos = &reg.videos.positions[p];
        response.clone().on_hover_text(format!("{} · {} runs", pos.name, pos.runs.len()));
        if response.clicked() && reg.position != Some(p) {
            reg.position = Some(p);
            reg.run = 0;
        }
    }
}

/// "24-25" for consecutive run numbers, else "3, 7"; `None` when no run has one.
pub(super) fn run_numbers(runs: &[crate::video::runs::Run]) -> Option<String> {
    let nrs: Vec<i64> = runs.iter().filter_map(|r| r.run_nr).collect();
    let (&first, &last) = (nrs.first()?, nrs.last()?);
    Some(if nrs.len() > 1 && nrs.windows(2).all(|w| w[1] == w[0] + 1) {
        format!("{first}-{last}")
    } else {
        nrs.iter().map(|n| n.to_string()).collect::<Vec<_>>().join(", ")
    })
}

/// Right panel: the selected position and its runs.
pub fn right_panel(
    ui: &mut egui::Ui,
    reg: Option<&mut Registration>,
    axis: Option<&TimeAxis>,
    unit: TimeUnit,
    store: &Store,
    added: &[AddedSegment],
) -> bool {
    ui.heading("Position");
    let Some(reg) = reg else {
        return false;
    };
    let Some(p) = reg.position else {
        ui.label(egui::RichText::new("Click a position in the stage scatter").weak());
        return false;
    };
    let zoom = ui.small_button("Zoom to run").on_hover_text("fit the view to the selected run's frame (⌘2)").clicked();
    let pos = &reg.videos.positions[p];
    ui.strong(&pos.name);
    ui.label(format!("stage x {:.3}  y {:.3} mm · {} runs", pos.stage_mm[0], pos.stage_mm[1], pos.runs.len()));
    match reg.anchors.get(&p) {
        Some(w) => {
            let w = *w;
            ui.horizontal(|ui| {
                ui.label(format!("anchored at {:.0}, {:.0} µm", w[0], w[1]));
                if ui.small_button("remove").clicked() {
                    let name = reg.videos.positions[p].name.clone();
                    reg.pending.push(Op::SetAnchor(name, None));
                }
            });
        }
        None => {
            ui.label(egui::RichText::new("no anchor: Anchor tool (A), then click where this video's centre belongs").weak().small());
        }
    }
    let pos = &reg.videos.positions[p];
    if let Some(run) = pos.runs.get(reg.run) {
        ui.separator();
        ui.strong(format!("Run {} / {}", reg.run + 1, pos.runs.len()));
        egui::Grid::new("run").num_columns(2).show(ui, |ui| {
            let row = |ui: &mut egui::Ui, k: &str, v: String| {
                ui.label(egui::RichText::new(k).weak());
                ui.label(v);
                ui.end_row();
            };
            row(ui, "time", utc(run.utc));
            row(ui, "mode", run.mode.clone());
            row(ui, "run nr", run.run_nr.map_or("—".into(), |n| n.to_string()));
            row(ui, "scale", format!("{:.3} µm/px", run.delta_x_um));
            if let Some(axis) = axis {
                if let Some(f) = crate::video::runs::stg_frame(run.utc, &axis.timepoints) {
                    let offset_h = (axis.timepoints[f] - run.utc) as f64 / 3600.0;
                    row(ui, "STG frame", format!("{f} ({offset_h:+.1} h)"));
                    if unit != TimeUnit::Frame {
                        row(ui, "", axis.label(f, unit));
                    }
                }
            }
            row(ui, "in store", if reg.mapped_in_store.contains(&run.rel_path) { "mapped".into() } else { "no".into() });
        });
    }
    let n_runs = pos.runs.len();
    // live, from the current placement: the store mark only says what was last published
    let chained: Vec<Option<usize>> = (0..n_runs).map(|i| reg.chained_edges(p, i, store, added)).collect();
    let pos = &reg.videos.positions[p];
    let mut chosen = None;
    egui::CollapsingHeader::new(format!("All runs ({n_runs})")).default_open(true).show(ui, |ui| {
        egui::ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
            for (i, run) in pos.runs.iter().enumerate() {
                let stored = if reg.mapped_in_store.contains(&run.rel_path) { " · in store" } else { "" };
                let (live, colour) = match chained[i] {
                    Some(0) => ("0 edges chained".to_string(), Some(Color32::YELLOW)),
                    Some(n) => (format!("{n} edges chained"), None),
                    None if reg.fits().get(reg.session).is_some_and(|f| f.is_some()) => ("no video graph".to_string(), Some(Color32::LIGHT_RED)),
                    None => ("not placed".to_string(), None),
                };
                let mut text = egui::RichText::new(format!("{} · {} · {live}{stored}", utc(run.utc), run.mode));
                if let Some(c) = colour {
                    text = text.color(c);
                }
                if ui.selectable_label(reg.run == i, text).clicked() {
                    chosen = Some(i);
                }
            }
        });
    });
    if let Some(i) = chosen {
        reg.run = i;
    }
    edges_section(ui, reg, store, added);
    zoom
}

/// Bottom bar: step through the selected position's runs; the STG time follows the run.
pub fn run_bar(ui: &mut egui::Ui, reg: &mut Registration) {
    let Some(p) = reg.position else {
        ui.label(egui::RichText::new("Select a position to step through its runs").weak());
        return;
    };
    let n = reg.videos.positions[p].runs.len();
    ui.horizontal(|ui| {
        if ui.button("◀").on_hover_text("Previous run").clicked() {
            reg.run = reg.run.saturating_sub(1);
        }
        if ui.button("▶").on_hover_text("Next run").clicked() {
            reg.run = (reg.run + 1).min(n - 1);
        }
        ui.spacing_mut().slider_width = (ui.available_width() - 220.0).max(80.0);
        ui.add(egui::Slider::new(&mut reg.run, 0..=n - 1).show_value(false));
        let run = &reg.videos.positions[p].runs[reg.run];
        ui.label(format!("run {} / {n} · {}", reg.run + 1, utc(run.utc)));
    });
}

/// Tool buttons for the registration workspace.
/// Returns true when Publish to store… was clicked.
pub fn toolbar(ui: &mut egui::Ui, reg: &mut Registration) -> bool {
    for (tool, _, label) in RegTool::ALL {
        if ui.selectable_label(reg.tool == tool, label).clicked() {
            reg.tool = tool;
        }
    }
    ui.separator();
    let (mut threshold, mut snap) = (reg.threshold(), reg.snap());
    let t = ui
        .add(egui::DragValue::new(&mut threshold).range(2.0..=40.0).speed(0.2).suffix(" µm").prefix("threshold "))
        .on_hover_text("how close a video edge must lie to an STG edge to count as on it");
    let s = ui.checkbox(&mut snap, "snap").on_hover_text("fit each run onto the STG with a small rigid move");
    if t.changed() || s.changed() {
        reg.set_params(reg.camera(), threshold, snap);
    }
    ui.separator();
    let publish = ui
        .button("Publish to store…")
        .on_hover_text("write your verified flow layer, trails and roots into stg_store.h5, for tools that do not read annotation files yet")
        .clicked();
    ui.separator();
    let hint = match (reg.tool, reg.position) {
        (RegTool::Select, None) => "click a video on the map, or a dot in the stage scatter",
        (RegTool::Select, Some(_)) => "click a video edge to see its STG chain and flow",
        (RegTool::Anchor, None) => "select a position first (stage scatter or map)",
        (RegTool::Anchor, Some(_)) => "click the STG where this video's centre belongs (snaps to a node within 10 px)",
        (RegTool::Assign, _) if reg.video_edge.is_none() => "select a video edge first (right panel or Select tool)",
        (RegTool::Assign, _) => "click STG edges to add them to the selected video edge's chain, or remove them",
        (RegTool::Disable, _) => "click a video edge to mark it as an artifact (no flow), or unmark it",
        (RegTool::Nudge, _) => "click a video frame to select its run, drag it to move it (the snap refines from where you leave it); the left / right arrow keys switch runs",
    };
    ui.label(egui::RichText::new(hint).weak());
    publish
}

/// The selected run's video edges: coverage and chain per edge, the selected edge's chain and
/// flow values, and the snap.
pub fn edges_section(ui: &mut egui::Ui, reg: &mut Registration, store: &Store, added: &[AddedSegment]) {
    let Some(p) = reg.position else { return };
    let fits = reg.fits();
    let Some(Some(fit)) = fits.get(reg.session) else {
        ui.label(egui::RichText::new("place the session to match its videos").weak());
        return;
    };
    let run = reg.shown_run(p);
    let Some(m) = reg.run_match(p, run, &fit.sim, store, added) else {
        ui.colored_label(Color32::LIGHT_RED, "could not read this run's video graph");
        return;
    };
    ui.separator();
    match m.snap {
        Some(r) => ui.label(format!("snap: {:.1}, {:.1} µm · {:.2}°", r.shift[0], r.shift[1], r.theta.to_degrees())),
        None if reg.snap() => ui.colored_label(Color32::YELLOW, "snap refused (too little STG nearby, or a large move)"),
        None => ui.label(egui::RichText::new("snap off").weak()),
    };
    let rel = reg.run_path(p, run);
    if m.nudge != [0.0; 2] {
        ui.horizontal(|ui| {
            ui.label(format!("nudge: {:.1}, {:.1} µm", m.nudge[0], m.nudge[1]));
            if ui.small_button("reset").clicked() {
                reg.pending.push(Op::SetNudge(rel.clone(), None));
            }
        });
    }
    run_corrections(ui, reg, p, run);
    let flow = m.flow.as_ref().ok();
    if let Err(e) = &m.flow {
        ui.colored_label(Color32::LIGHT_RED, format!("flow: {e}"));
    }
    ui.strong("Video edges");
    egui::Grid::new("video_edges").num_columns(4).striped(true).show(ui, |ui| {
        for (i, c) in m.covers.iter().enumerate() {
            let (rect, _) = ui.allocate_exact_size(egui::vec2(10.0, 10.0), egui::Sense::hover());
            ui.painter().rect_filled(rect, 2.0, EDGE_COLOURS[i % EDGE_COLOURS.len()]);
            let has_flow = flow.is_some_and(|f| f.contains_key(&c.key));
            let mut label = egui::RichText::new(format!("({}, {}, {})", c.key[0], c.key[1], c.key[2]));
            if m.disabled[i] {
                label = label.strikethrough().weak();
            }
            if ui.selectable_label(reg.video_edge == Some(c.key), label).clicked() {
                reg.video_edge = Some(c.key);
            }
            let cov = format!("{:.0}%", c.coverage * 100.0);
            ui.label(if c.coverage < 0.9 { egui::RichText::new(cov).color(GAP) } else { egui::RichText::new(cov) });
            let mut chain = match (c.chain.len(), has_flow) {
                (0, _) => "no STG".to_string(),
                (n, true) => format!("{n} STG"),
                (n, false) => format!("{n} STG · no flow"),
            };
            if m.assigned[i] {
                chain += " · edited";
            }
            if m.disabled[i] {
                chain += " · artifact";
            }
            ui.label(egui::RichText::new(chain).small());
            ui.end_row();
        }
    });
    let Some(ci) = reg.video_edge.and_then(|k| m.covers.iter().position(|c| c.key == k)) else { return };
    let c = &m.covers[ci];
    ui.separator();
    ui.strong(format!("Video edge ({}, {}, {})", c.key[0], c.key[1], c.key[2]));
    let run_edge = (rel.clone(), c.key);
    let mut artifact = m.disabled[ci];
    if ui.checkbox(&mut artifact, "artifact (no flow)").changed() {
        reg.pending.push(Op::SetDisabled(run_edge.clone(), artifact));
    }
    for (j, (e, dir)) in c.chain.iter().enumerate() {
        ui.horizontal(|ui| {
            let id = match e {
                EdgeRef::Pipeline(e) => format!("edge {e}"),
                EdgeRef::Added(id) => format!("added {id}"),
            };
            ui.label(format!("{id} {}", if *dir > 0 { "(node1 to node2)" } else { "(node2 to node1)" }));
            if ui.small_button("flip").clicked() {
                let mut chain = c.chain.clone();
                chain[j].1 = -chain[j].1;
                reg.pending.push(Op::SetAssignment(run_edge.clone(), Some(chain)));
            }
            if ui.small_button("remove").clicked() {
                let mut chain = c.chain.clone();
                chain.remove(j);
                reg.pending.push(Op::SetAssignment(run_edge.clone(), Some(chain)));
            }
        });
    }
    if m.assigned[ci] && ui.small_button("Back to automatic").clicked() {
        reg.pending.push(Op::SetAssignment(run_edge.clone(), None));
    }
    let chain: Vec<String> = c
        .chain
        .iter()
        .map(|(e, dir)| {
            let id = match e {
                EdgeRef::Pipeline(e) => e.to_string(),
                EdgeRef::Added(id) => format!("{id} (added)"),
            };
            format!("{id}{}", if *dir > 0 { "" } else { " (reversed)" })
        })
        .collect();
    ui.label(egui::RichText::new(format!("STG chain: {}", if chain.is_empty() { "none".into() } else { chain.join(", ") })).small())
        .on_hover_text("in order from the video edge's u to v; reversed = the video runs node2 -> node1 of that STG edge");
    match flow.and_then(|f| f.get(&c.key)) {
        None => {
            ui.label(egui::RichText::new("no flow analysed for this edge").weak());
        }
        Some(dirs) => {
            egui::Grid::new("flow_values").num_columns(3).show(ui, |ui| {
                ui.label("");
                for d in dirs {
                    ui.label(egui::RichText::new(if d.dir == c.key { "u to v" } else { "v to u" }).strong());
                }
                ui.end_row();
                for (k, name) in crate::video::flow::VALUE_NAMES.iter().enumerate() {
                    ui.label(egui::RichText::new(*name).weak());
                    for d in dirs {
                        ui.monospace(format!("{:.3}", d.values[k]));
                    }
                    ui.end_row();
                }
            });
        }
    }
}

/// Copy the previous run's corrections onto this run, or clear this run's corrections.
fn run_corrections(ui: &mut egui::Ui, reg: &mut Registration, p: usize, run: usize) {
    let rel = reg.run_path(p, run);
    let mine = reg.layer.nudges.contains_key(&rel)
        || reg.layer.assignments.keys().any(|k| k.0 == rel)
        || reg.layer.disabled.iter().any(|k| k.0 == rel);
    ui.horizontal(|ui| {
        if run > 0 && ui.small_button("Copy corrections from previous run").clicked() {
            let prev = reg.run_path(p, run - 1);
            let keys: HashSet<[i64; 3]> = reg.graph(p, run).map(|g| g.edges.iter().map(|e| e.key).collect()).unwrap_or_default();
            let mut ops = vec![];
            if let Some(n) = reg.layer.nudges.get(&prev) {
                ops.push(Op::SetNudge(rel.clone(), Some(*n)));
            }
            for ((r, k), chain) in &reg.layer.assignments {
                if *r == prev && keys.contains(k) {
                    ops.push(Op::SetAssignment((rel.clone(), *k), Some(chain.clone())));
                }
            }
            for (r, k) in &reg.layer.disabled {
                if *r == prev && keys.contains(k) {
                    ops.push(Op::SetDisabled((rel.clone(), *k), true));
                }
            }
            reg.copied_note = Some((rel.clone(), format!("copied {} corrections from run {}", ops.len(), run)));
            reg.pending.extend(ops);
        }
        if mine && ui.small_button("Clear this run's corrections").clicked() {
            let mut ops = vec![Op::SetNudge(rel.clone(), None)];
            ops.extend(reg.layer.assignments.keys().filter(|k| k.0 == rel).map(|k| Op::SetAssignment(k.clone(), None)));
            ops.extend(reg.layer.disabled.iter().filter(|k| k.0 == rel).map(|k| Op::SetDisabled(k.clone(), false)));
            reg.pending.extend(ops);
        }
    });
    if let Some((_, note)) = reg.copied_note.as_ref().filter(|(r, _)| *r == rel) {
        ui.label(egui::RichText::new(note).weak().small());
    }
}
