//! Annotation editing UI: tools, the Annotations panel, the inspector's edit section, the
//! map overlay, and the unsaved-changes prompt (DESIGN.md §3, §7).

use crate::annotations::{Annotations, Op, Target};
use crate::store::Store;
use egui::{Color32, Pos2, Shape, Stroke};
use std::path::PathBuf;

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum Tool {
    #[default]
    Select,
    BoxSelect,
    Exclude,
    Merge,
    Root,
    AddEdge,
    Trail,
}

impl Tool {
    pub const ALL: [Tool; 7] =
        [Tool::Select, Tool::BoxSelect, Tool::Exclude, Tool::Merge, Tool::Root, Tool::AddEdge, Tool::Trail];

    /// Tools that act on nodes: nodes are drawn and the one under the cursor highlighted.
    pub fn needs_nodes(self) -> bool {
        matches!(self, Tool::Root | Tool::AddEdge | Tool::Trail)
    }

    fn name(self) -> &'static str {
        match self {
            Tool::Select => "Select",
            Tool::BoxSelect => "Box select",
            Tool::Exclude => "Exclude",
            Tool::Merge => "Merge",
            Tool::Root => "Root",
            Tool::AddEdge => "Add edge",
            Tool::Trail => "Trail",
        }
    }

    pub fn key(self) -> egui::Key {
        match self {
            Tool::Select => egui::Key::V,
            Tool::BoxSelect => egui::Key::B,
            Tool::Exclude => egui::Key::X,
            Tool::Merge => egui::Key::M,
            Tool::Root => egui::Key::R,
            Tool::AddEdge => egui::Key::E,
            Tool::Trail => egui::Key::T,
        }
    }

    fn help(self) -> &'static str {
        match self {
            Tool::Select => "click an edge, node or added edge to inspect it",
            Tool::BoxSelect => "drag a box to select edges, e.g. for export (right-drag pans)",
            Tool::Exclude => "click an edge to toggle excluded",
            Tool::Merge => "click the duplicate edge, then the edge it duplicates",
            Tool::Root => "click a node to toggle root",
            Tool::AddEdge => "click two nodes to connect them",
            Tool::Trail => "click nodes to add waypoints to the active trail; it follows the shortest path",
        }
    }
}

/// Annotation state for the open store.
pub struct AnnotationState {
    pub author: String,
    /// The user's own file: the only one edited.
    pub own: Annotations,
    pub own_path: PathBuf,
    pub own_visible: bool,
    /// Other authors' files: read-only, each with a visibility toggle.
    pub others: Vec<(Annotations, bool)>,
    pub tool: Tool,
    /// First click of a two-click tool: the duplicate edge (Merge) or a node row (Add edge).
    pub first: Option<usize>,
    /// Text being edited in the inspector, for this target: (note, hyperedge override).
    pub fields: Option<(Target, String, String)>,
    /// The trail the Trail tool adds waypoints to, and the name field for a new one.
    pub active_trail: Option<String>,
    pub new_trail_name: String,
}

impl AnnotationState {
    pub fn new(author: &str, own: Annotations, own_path: PathBuf, others: Vec<Annotations>) -> Self {
        Self {
            author: author.into(),
            own,
            own_path,
            own_visible: true,
            others: others.into_iter().map(|a| (a, true)).collect(),
            tool: Tool::Select,
            first: None,
            fields: None,
            active_trail: None,
            new_trail_name: String::new(),
        }
    }
}

/// Tool buttons and the current tool's hint.
pub fn toolbar(ui: &mut egui::Ui, st: &mut AnnotationState) {
    ui.horizontal(|ui| {
        for tool in Tool::ALL {
            if ui.selectable_label(st.tool == tool, shortcut_label(ui, tool)).clicked() {
                st.tool = tool;
                st.first = None;
            }
        }
        ui.separator();
        let hint = match (st.tool, st.first) {
            (Tool::Merge, Some(e)) => format!("edge {e} duplicates which edge? (Esc cancels)"),
            (Tool::AddEdge, Some(_)) => "click the second node (Esc cancels)".into(),
            (Tool::Trail, _) => match &st.active_trail {
                Some(name) => format!("click nodes to extend \"{name}\""),
                None => "click a node to start a new trail".into(),
            },
            (tool, _) => tool.help().into(),
        };
        ui.label(egui::RichText::new(hint).weak());
    });
}

/// The tool name with its shortcut letter underlined, or `Name (K)` when the name lacks it.
fn shortcut_label(ui: &egui::Ui, tool: Tool) -> egui::text::LayoutJob {
    let name = tool.name();
    let key = format!("{:?}", tool.key()).to_lowercase();
    let style = ui.style();
    let plain = egui::TextFormat::simple(
        egui::TextStyle::Button.resolve(style),
        style.visuals.text_color(),
    );
    let mut job = egui::text::LayoutJob::default();
    match name.to_lowercase().find(&key) {
        Some(i) => {
            let end = i + key.len();
            job.append(&name[..i], 0.0, plain.clone());
            let underline = egui::TextFormat {
                underline: egui::Stroke::new(1.0, style.visuals.text_color()),
                ..plain.clone()
            };
            job.append(&name[i..end], 0.0, underline);
            job.append(&name[end..], 0.0, plain);
        }
        None => job.append(&format!("{name} ({})", key.to_uppercase()), 0.0, plain),
    }
    job
}

pub enum PanelAction {
    Ops(Vec<Op>),
    OpenSettings,
    None,
}

/// The Annotations section of the left panel.
/// `unreachable` names the user's trails whose waypoints have no connecting path.
pub fn panel(ui: &mut egui::Ui, st: &mut AnnotationState, store: &Store, unreachable: &[String]) -> PanelAction {
    let mut action = PanelAction::None;
    egui::CollapsingHeader::new("Annotations").default_open(true).show(ui, |ui| {
        let stale = |a: &Annotations| !a.fingerprint.is_empty() && a.fingerprint != store.fingerprint;
        let mark = if st.own.dirty { " •" } else { "" };
        ui.horizontal(|ui| {
            ui.checkbox(&mut st.own_visible, format!("{} (you){mark}", st.author));
            if stale(&st.own) {
                ui.colored_label(Color32::LIGHT_RED, "stale").on_hover_text(STALE_HELP);
            }
        });
        for (a, visible) in &mut st.others {
            ui.horizontal(|ui| {
                ui.checkbox(visible, &a.author);
                if stale(a) {
                    ui.colored_label(Color32::LIGHT_RED, "stale").on_hover_text(STALE_HELP);
                }
            });
        }
        let n = |x: usize| x.to_string();
        ui.label(
            egui::RichText::new(format!(
                "{} excluded · {} merged · {} roots · {} added · {} notes",
                n(st.own.excluded.len()),
                n(st.own.duplicate_of.len()),
                n(st.own.roots.len()),
                n(st.own.added.len()),
                n(st.own.notes.len())
            ))
            .weak()
            .small(),
        );

        trails_section(ui, st, store, unreachable, &mut action);

        if !store.root_nodes.is_empty() && st.own.roots.is_empty() {
            let label = format!("Import {} roots from store", store.root_nodes.len());
            if ui.button(label).on_hover_text("copies /root_nodes into your annotations").clicked() {
                action = PanelAction::Ops(store.root_nodes.iter().map(|&n| Op::SetRoot(n, true)).collect());
            }
        }

        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(format!("editing as {}", st.author)).weak().small());
            if ui.small_button("Settings…").on_hover_text("change the author name").clicked() {
                action = PanelAction::OpenSettings;
            }
        });
    });
    action
}

/// The user's trails: pick the active one, start, shorten or delete trails, import old ones.
fn trails_section(ui: &mut egui::Ui, st: &mut AnnotationState, store: &Store, unreachable: &[String], action: &mut PanelAction) {
    egui::CollapsingHeader::new(format!("Trails ({})", st.own.trails.len())).default_open(true).show(ui, |ui| {
        let mut ops = vec![];
        for (name, waypoints) in &st.own.trails {
            ui.horizontal(|ui| {
                let active = st.active_trail.as_deref() == Some(name.as_str());
                if ui.selectable_label(active, name).on_hover_text("make this the trail the Trail tool extends").clicked() {
                    st.active_trail = Some(name.clone());
                }
                ui.label(egui::RichText::new(format!("{} pts", waypoints.len())).weak().small());
                if unreachable.contains(name) {
                    ui.colored_label(Color32::LIGHT_RED, "no path").on_hover_text("some waypoints are not connected in the STG");
                }
            });
        }
        if let Some(name) = st.active_trail.clone().filter(|n| st.own.trails.contains_key(n)) {
            ui.horizontal(|ui| {
                let waypoints = &st.own.trails[&name];
                if ui.add_enabled(!waypoints.is_empty(), egui::Button::new("Remove last point").small()).clicked() {
                    ops.push(Op::SetTrail(name.clone(), Some(waypoints[..waypoints.len() - 1].to_vec())));
                }
                if ui.small_button("Delete").clicked() {
                    ops.push(Op::SetTrail(name.clone(), None));
                    st.active_trail = None;
                }
            });
        }
        ui.horizontal(|ui| {
            ui.add(egui::TextEdit::singleline(&mut st.new_trail_name).hint_text("new trail name").desired_width(110.0));
            let name = st.new_trail_name.trim().to_string();
            let ok = !name.is_empty() && !st.own.trails.contains_key(&name);
            if ui.add_enabled(ok, egui::Button::new("New").small()).clicked() {
                ops.push(Op::SetTrail(name.clone(), Some(vec![])));
                st.active_trail = Some(name);
                st.new_trail_name.clear();
                st.tool = Tool::Trail;
            }
        });
        if !store.trails_in_store.is_empty() && st.own.trails.is_empty() {
            let label = format!("Import {} trails from store", store.trails_in_store.len());
            if ui.small_button(label).on_hover_text("copies /flow_trails_* (their full node paths) into your annotations").clicked() {
                ops.extend(store.trails_in_store.iter().map(|(n, nodes)| Op::SetTrail(n.clone(), Some(nodes.clone()))));
            }
        }
        if !ops.is_empty() {
            *action = PanelAction::Ops(ops);
        }
    });
}

const STALE_HELP: &str = "made against a different backbone_fingerprint: the store was rebuilt, \
so edge ids may no longer mean the same edges";

/// Everything one author's file says about `target`, as short phrases.
fn facts(a: &Annotations, target: Target) -> Vec<String> {
    let mut out = vec![];
    match target {
        Target::Edge(e) => {
            if a.excluded.contains(&e) {
                out.push("excluded".into());
            }
            if let Some(k) = a.duplicate_of.get(&e) {
                out.push(format!("duplicate of {k}"));
            }
            if let Some(h) = a.hyperedge.get(&e) {
                out.push(format!("hyperedge {h}"));
            }
        }
        Target::Node(n) => {
            if a.roots.contains(&n) {
                out.push("root".into());
            }
        }
    }
    if let Some(note) = a.notes.get(&target) {
        out.push(format!("note “{note}”"));
    }
    out
}

/// The annotation part of the inspector for an edge or node: edit fields for the user's own
/// file, then what other authors recorded.
pub fn inspector_section(ui: &mut egui::Ui, st: &mut AnnotationState, store: &Store, target: Target) -> Vec<Op> {
    let mut ops = vec![];
    let own = &st.own;
    if st.fields.as_ref().map(|f| f.0) != Some(target) {
        let note = own.notes.get(&target).cloned().unwrap_or_default();
        let hyper = match target {
            Target::Edge(e) => own.hyperedge.get(&e).map_or(String::new(), i64::to_string),
            Target::Node(_) => String::new(),
        };
        st.fields = Some((target, note, hyper));
    }
    let (_, note, hyper) = st.fields.as_mut().expect("set above");

    ui.separator();
    ui.strong(format!("Your annotations ({})", st.author));
    match target {
        Target::Edge(e) => {
            let mut excluded = own.excluded.contains(&e);
            if ui.checkbox(&mut excluded, "excluded").changed() {
                ops.push(Op::SetExcluded(e, excluded));
            }
            if let Some(&k) = own.duplicate_of.get(&e) {
                ui.horizontal(|ui| {
                    ui.label(format!("duplicate of {k}"));
                    if ui.small_button("clear").clicked() {
                        ops.push(Op::SetDuplicateOf(e, None));
                    }
                });
            }
            ui.horizontal(|ui| {
                ui.label("hyperedge");
                let field = ui.add(
                    egui::TextEdit::singleline(hyper)
                        .desired_width(70.0)
                        .hint_text(store.hyperedge[e].to_string()),
                );
                if field.lost_focus() {
                    let value = hyper.trim().parse::<i64>().ok();
                    if value != own.hyperedge.get(&e).copied() {
                        ops.push(Op::SetHyperedge(e, value));
                    }
                }
            })
            .response
            .on_hover_text("override the pipeline's hyperedge id; empty = no override");
        }
        Target::Node(n) => {
            let mut root = own.roots.contains(&n);
            if ui.checkbox(&mut root, "root").changed() {
                ops.push(Op::SetRoot(n, root));
            }
        }
    }
    let field = ui.add(egui::TextEdit::multiline(note).desired_rows(2).hint_text("note"));
    if field.lost_focus() {
        let value = (!note.trim().is_empty()).then(|| note.trim().to_string());
        if value.as_ref() != own.notes.get(&target) {
            ops.push(Op::SetNote(target, value));
        }
    }

    let others: Vec<(String, Vec<String>)> = st
        .others
        .iter()
        .map(|(a, _)| (a.author.clone(), facts(a, target)))
        .filter(|(_, f)| !f.is_empty())
        .collect();
    if !others.is_empty() {
        ui.separator();
        ui.strong("Other authors");
        for (author, f) in others {
            ui.label(format!("{author}: {}", f.join(", ")));
        }
    }
    ops
}

/// Annotations drawn over the map, in points. `to_screen` maps µm to points; only geometry
/// that has appeared by timepoint `t` is drawn. Other authors' layers are drawn fainter.
pub fn overlay(
    store: &Store,
    st: &AnnotationState,
    t: f32,
    selected_added: Option<i64>,
    to_screen: impl Fn([f32; 2]) -> Pos2,
) -> Vec<Shape> {
    let mut shapes = vec![];
    let layers = std::iter::once((&st.own, st.own_visible, 255u8))
        .chain(st.others.iter().map(|(a, v)| (a, *v, 110)));
    let node_pos = |id: &i64| store.node_index.get(id).map(|&n| (store.node_pos[n], store.node_t[n]));

    for (a, _, alpha) in layers.filter(|l| l.1) {
        let width = if alpha == 255 { 2.5 } else { 1.5 };
        let stroke = |c: Color32| Stroke::new(width, Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), alpha));
        let edge_line = |e: usize, c: Color32| -> Option<Shape> {
            (e < store.n_edges() && store.edge_t[e] <= t).then(|| {
                let r = store.arcs.offsets[e]..store.arcs.offsets[e + 1];
                Shape::line(store.arcs.points[r].iter().map(|&p| to_screen(p)).collect(), stroke(c))
            })
        };
        shapes.extend(a.excluded.iter().filter_map(|&e| edge_line(e, Color32::from_rgb(230, 60, 60))));
        shapes.extend(a.duplicate_of.keys().filter_map(|&e| edge_line(e, Color32::from_rgb(240, 150, 40))));
        for added in &a.added {
            if let (Some((pu, tu)), Some((pv, tv))) = (node_pos(&added.u), node_pos(&added.v)) {
                if tu <= t && tv <= t {
                    let c = if selected_added == Some(added.id) { Color32::WHITE } else { Color32::from_rgb(60, 210, 230) };
                    shapes.extend(Shape::dashed_line(&[to_screen(pu), to_screen(pv)], stroke(c), 6.0, 4.0));
                }
            }
        }
        for root in &a.roots {
            if let Some((p, _)) = node_pos(root).filter(|(_, tn)| *tn <= t) {
                shapes.push(Shape::circle_stroke(to_screen(p), 6.0, stroke(Color32::from_rgb(250, 220, 60))));
            }
        }
        for target in a.notes.keys() {
            let p = match *target {
                Target::Edge(e) if e < store.n_edges() && store.edge_t[e] <= t => {
                    Some(store.arcs.points[(store.arcs.offsets[e] + store.arcs.offsets[e + 1]) / 2])
                }
                Target::Node(n) => node_pos(&n).filter(|(_, tn)| *tn <= t).map(|(p, _)| p),
                _ => None,
            };
            if let Some(p) = p {
                let c = Color32::from_rgba_unmultiplied(90, 150, 255, alpha);
                shapes.push(Shape::circle_filled(to_screen(p), 3.5, c));
            }
        }
    }
    shapes
}

#[derive(Clone)]
pub enum Pending {
    Quit,
    Open(PathBuf),
}

pub enum Choice {
    Save,
    Discard,
    Cancel,
}

/// Asks what to do with unsaved annotations before `pending` happens.
pub fn confirm(ui: &mut egui::Ui, author: &str) -> Option<Choice> {
    let mut choice = None;
    egui::Modal::new(egui::Id::new("unsaved")).show(ui.ctx(), |ui| {
        ui.heading("Unsaved annotations");
        ui.label(format!("Save your changes to annotations.{author}.h5?"));
        ui.horizontal(|ui| {
            if ui.button("Save").clicked() {
                choice = Some(Choice::Save);
            }
            if ui.button("Discard").clicked() {
                choice = Some(Choice::Discard);
            }
            if ui.button("Cancel").clicked() {
                choice = Some(Choice::Cancel);
            }
        });
    });
    choice
}
