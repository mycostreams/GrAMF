//! Clicks on the map: picking edges, nodes and added edges, box selection, and registration clicks.

use super::*;

/// Nearest added edge (own or a visible other author's) within `radius` µm.
fn pick_added(store: &Store, st: &AnnotationState, at: [f32; 2], radius: f32) -> Option<i64> {
    let pos = |id: &i64| store.node_index.get(id).map(|&n| store.node_pos[n]);
    std::iter::once((&st.own, st.own_visible))
        .chain(st.others.iter().map(|(a, v)| (a, *v)))
        .filter(|(_, visible)| *visible)
        .flat_map(|(a, _)| &a.added)
        .filter_map(|e| Some((e.id, crate::store::segment_dist2(at, pos(&e.u)?, pos(&e.v)?))))
        .filter(|&(_, d)| d <= radius * radius)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(id, _)| id)
}

impl App {
    /// World bounds of what is selected: the selected video frame in Video registration, else
    /// the selected edge, node, hyperedge, added edge or box selection.
    pub(super) fn selection_bounds(&mut self) -> Option<([f32; 2], [f32; 2])> {
        let store = self.store.as_ref()?;
        let added = added_segments(store, self.annot.as_ref());
        let mut pts: Vec<[f32; 2]> = vec![];
        if self.workflow == registration::Workflow::Registration {
            pts.extend(self.reg.as_mut()?.selected_frame(store, &added)?);
        } else {
            let arc = |e: usize| &store.arcs.points[store.arcs.offsets[e]..store.arcs.offsets[e + 1]];
            match self.selection.as_ref()? {
                ui::Selection::Edge(e) => pts.extend(arc(*e)),
                ui::Selection::Node(n) => pts.push(store.node_pos[*n]),
                ui::Selection::Hyperedge(h) => store.hyperedge_edges(*h).into_iter().for_each(|e| pts.extend(arc(e))),
                ui::Selection::Added(id) => added.iter().filter(|a| a.id == *id).for_each(|a| pts.extend([a.a, a.b])),
                ui::Selection::Box => self.boxed.iter().for_each(|&e| pts.extend(arc(e))),
            }
        }
        let first = *pts.first()?;
        Some(pts.iter().fold((first, first), |(lo, hi), p| ([lo[0].min(p[0]), lo[1].min(p[1])], [hi[0].max(p[0]), hi[1].max(p[1])])))
    }

    pub(super) fn select(&mut self, selection: Option<ui::Selection>) {
        let (Some(renderer), Some(store)) = (self.renderer.as_mut(), self.store.as_ref()) else {
            return;
        };
        self.selection = selection;
        let edges = match selection {
            Some(ui::Selection::Edge(e)) => vec![e],
            Some(ui::Selection::Node(n)) => store.incident_edges(n),
            Some(ui::Selection::Hyperedge(h)) => store.hyperedge_edges(h),
            Some(ui::Selection::Box) => self.boxed.clone(),
            Some(ui::Selection::Added(_)) | None => vec![], // the overlay highlights added edges
        };
        renderer.set_selection(store, &edges);
    }

    /// Acts on a click at window pixel `px` with the current tool.
    pub(super) fn pick(&mut self, px: [f32; 2]) {
        let started = Instant::now();
        if self.workflow == registration::Workflow::Registration {
            self.registration_click(px);
        } else {
            self.pick_inner(px);
        }
        self.dev.timings.pick_ms = ms(started);
    }

    /// A map click in the registration workspace: select a video, or anchor the selected one.
    fn registration_click(&mut self, px: [f32; 2]) {
        let (Some(renderer), Some(store), Some(reg)) = (self.renderer.as_ref(), self.store.as_ref(), self.reg.as_mut()) else {
            return;
        };
        let at = renderer.screen_to_world(px);
        let added = added_segments(store, self.annot.as_ref());
        let radius = PICK_RADIUS_PX * renderer.um_per_px();
        match (reg.tool, reg.position) {
            (registration::RegTool::Assign, Some(_)) => {
                let edge = store
                    .pick_edge(at, radius, f32::MAX)
                    .map(crate::video::matching::EdgeRef::Pipeline)
                    .or_else(|| {
                        added
                            .iter()
                            .map(|a| (crate::store::segment_dist2(at, a.a, a.b).sqrt(), a.id))
                            .filter(|x| x.0 <= radius)
                            .min_by(|a, b| a.0.total_cmp(&b.0))
                            .map(|x| crate::video::matching::EdgeRef::Added(x.1))
                    });
                if let Some(op) = edge.and_then(|e| registration::toggle_in_chain(reg, store, &added, e)) {
                    reg.pending.push(op);
                }
            }
            (registration::RegTool::Disable, Some(p)) => {
                if let Some(key) = registration::video_edge_at(reg, store, &added, at, radius) {
                    let run_edge = (reg.run_path(p, reg.shown_run(p)), key);
                    let now = reg.layer.disabled.contains(&run_edge);
                    reg.pending.push(Op::SetDisabled(run_edge, !now));
                    reg.video_edge = Some(key);
                }
            }
            (registration::RegTool::Nudge, _) => (), // drags, not clicks
            (registration::RegTool::Anchor, Some(p)) => {
                let snap = store.pick_node(at, NODE_PICK_RADIUS_PX * renderer.um_per_px(), renderer.time);
                let w = snap.map_or(at, |n| store.node_pos[n]);
                let name = reg.videos.positions[p].name.clone();
                reg.pending.push(Op::SetAnchor(name, Some([w[0] as f64, w[1] as f64])));
            }
            _ => registration::pick(reg, store, &added, at, radius),
        }
    }

    fn pick_inner(&mut self, px: [f32; 2]) {
        let (Some(renderer), Some(store), Some(st)) = (self.renderer.as_ref(), self.store.as_ref(), self.annot.as_mut())
        else {
            return;
        };
        let at = renderer.screen_to_world(px);
        let radius = PICK_RADIUS_PX * renderer.um_per_px();
        let node_radius = NODE_PICK_RADIUS_PX * renderer.um_per_px();
        let t = renderer.time;
        let node = || store.pick_node(at, if st.tool.needs_nodes() { node_radius } else { radius }, t);
        let edge = || store.pick_edge(at, radius, t);

        let mut op = None;
        let selection = match st.tool {
            Tool::BoxSelect => return, // boxes are drawn by dragging; a plain click does nothing
            Tool::Select => node()
                .map(ui::Selection::Node)
                .or_else(|| pick_added(store, st, at, radius).map(ui::Selection::Added))
                .or_else(|| edge().map(ui::Selection::Edge)),
            Tool::Exclude => edge().map(|e| {
                op = Some(Op::SetExcluded(e, !st.own.excluded.contains(&e)));
                ui::Selection::Edge(e)
            }),
            Tool::Root => node().map(|n| {
                let id = store.node_ids[n];
                op = Some(Op::SetRoot(id, !st.own.roots.contains(&id)));
                ui::Selection::Node(n)
            }),
            Tool::Merge => edge().map(|e| match st.first.take() {
                Some(dup) if dup != e => {
                    op = Some(Op::SetDuplicateOf(dup, Some(e)));
                    ui::Selection::Edge(dup)
                }
                _ => {
                    st.first = Some(e);
                    ui::Selection::Edge(e)
                }
            }),
            Tool::Trail => node().map(|n| {
                let id = store.node_ids[n];
                let name = st.active_trail.clone().filter(|t| st.own.trails.contains_key(t)).unwrap_or_else(|| {
                    (1..).map(|k| format!("trail {k}")).find(|t| !st.own.trails.contains_key(t)).expect("unbounded")
                });
                let mut waypoints = st.own.trails.get(&name).cloned().unwrap_or_default();
                if waypoints.last() != Some(&id) {
                    waypoints.push(id);
                    op = Some(Op::SetTrail(name.clone(), Some(waypoints)));
                }
                st.active_trail = Some(name);
                ui::Selection::Node(n)
            }),
            Tool::AddEdge => node().map(|n| match st.first.take() {
                Some(first) if first != n => {
                    let added = AddedEdge {
                        id: st.own.next_added_id(),
                        u: store.node_ids[first],
                        v: store.node_ids[n],
                    };
                    op = Some(Op::AddEdge(added));
                    ui::Selection::Added(added.id)
                }
                _ => {
                    st.first = Some(n);
                    ui::Selection::Node(n)
                }
            }),
        };
        if let Some(op) = op {
            st.own.edit(op);
        }
        if selection.is_some() || st.tool == Tool::Select {
            self.select(selection);
        }
    }

    /// Selects the edges with a vertex inside the box `a`–`b` (µm) that have appeared by now.
    pub(super) fn box_select(&mut self, a: [f32; 2], b: [f32; 2]) {
        let (Some(store), Some(renderer)) = (self.store.as_ref(), self.renderer.as_ref()) else {
            return;
        };
        let (min, max) = ([a[0].min(b[0]), a[1].min(b[1])], [a[0].max(b[0]), a[1].max(b[1])]);
        let t = renderer.time;
        let (p, vt) = (&store.arcs.points, &store.vertex_t);
        self.boxed = (0..store.n_edges())
            .filter(|&e| {
                (store.arcs.offsets[e]..store.arcs.offsets[e + 1]).any(|v| {
                    vt[v] <= t && p[v][0] >= min[0] && p[v][0] <= max[0] && p[v][1] >= min[1] && p[v][1] <= max[1]
                })
            })
            .collect();
        let selection = (!self.boxed.is_empty()).then_some(ui::Selection::Box);
        self.select(selection);
    }
}
