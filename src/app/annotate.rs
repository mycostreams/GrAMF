//! Annotation editing UI: tools, the Annotations panel, the inspector's edit section, the
//! map overlay, and the unsaved-changes prompt (DESIGN.md §3, §7).

use crate::annotations::{self, Annotations, Op, Target};
use crate::store::Store;
use egui::{Color32, Pos2, Shape, Stroke};
use std::path::PathBuf;

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum Tool {
    #[default]
    Select,
    Exclude,
    Merge,
    Root,
    AddEdge,
}

impl Tool {
    pub const ALL: [Tool; 5] = [Tool::Select, Tool::Exclude, Tool::Merge, Tool::Root, Tool::AddEdge];

    fn name(self) -> &'static str {
        match self {
            Tool::Select => "Select",
            Tool::Exclude => "Exclude",
            Tool::Merge => "Merge",
            Tool::Root => "Root",
            Tool::AddEdge => "Add edge",
        }
    }

    pub fn key(self) -> egui::Key {
        match self {
            Tool::Select => egui::Key::V,
            Tool::Exclude => egui::Key::X,
            Tool::Merge => egui::Key::M,
            Tool::Root => egui::Key::R,
            Tool::AddEdge => egui::Key::E,
        }
    }

    fn help(self) -> &'static str {
        match self {
            Tool::Select => "click an edge, node or added edge to inspect it",
            Tool::Exclude => "click an edge to toggle excluded",
            Tool::Merge => "click the duplicate edge, then the edge it duplicates",
            Tool::Root => "click a node to toggle root",
            Tool::AddEdge => "click two nodes to connect them",
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
    /// Text in the author field of the panel.
    pub author_field: String,
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
            author_field: author.into(),
        }
    }
}

/// Where the user's author name is kept between sessions.
fn author_file() -> Option<PathBuf> {
    std::env::home_dir().map(|h| h.join(".gramf").join("author"))
}

/// The saved author name, else the OS user name.
pub fn load_author() -> String {
    let saved = author_file().and_then(|p| std::fs::read_to_string(p).ok());
    let name = saved.or_else(|| std::env::var("USER").or_else(|_| std::env::var("USERNAME")).ok());
    let name = annotations::sanitize_author(name.as_deref().unwrap_or(""));
    if name.is_empty() { "anonymous".into() } else { name }
}

pub fn save_author(name: &str) {
    if let Some(path) = author_file() {
        let _ = std::fs::create_dir_all(path.parent().unwrap_or(&path));
        let _ = std::fs::write(path, name);
    }
}

/// Tool buttons and the current tool's hint.
pub fn toolbar(ui: &mut egui::Ui, st: &mut AnnotationState) {
    ui.horizontal(|ui| {
        for tool in Tool::ALL {
            let label = format!("{} ({:?})", tool.name(), tool.key());
            if ui.selectable_label(st.tool == tool, label).clicked() {
                st.tool = tool;
                st.first = None;
            }
        }
        ui.separator();
        let hint = match (st.tool, st.first) {
            (Tool::Merge, Some(e)) => format!("edge {e} duplicates which edge? (Esc cancels)"),
            (Tool::AddEdge, Some(_)) => "click the second node (Esc cancels)".into(),
            (tool, _) => tool.help().into(),
        };
        ui.label(egui::RichText::new(hint).weak());
    });
}

pub enum PanelAction {
    Ops(Vec<Op>),
    SwitchAuthor(String),
    None,
}

/// The Annotations section of the left panel.
pub fn panel(ui: &mut egui::Ui, st: &mut AnnotationState, store: &Store) -> PanelAction {
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

        if !store.root_nodes.is_empty() && st.own.roots.is_empty() {
            let label = format!("Import {} roots from store", store.root_nodes.len());
            if ui.button(label).on_hover_text("copies /root_nodes into your annotations").clicked() {
                action = PanelAction::Ops(store.root_nodes.iter().map(|&n| Op::SetRoot(n, true)).collect());
            }
        }

        ui.horizontal(|ui| {
            ui.label("author");
            ui.add(egui::TextEdit::singleline(&mut st.author_field).desired_width(90.0));
            let name = annotations::sanitize_author(&st.author_field);
            let can = !name.is_empty() && name != st.author && !st.own.dirty;
            let switch = ui.add_enabled(can, egui::Button::new("switch"));
            if st.own.dirty {
                switch.on_disabled_hover_text("save your annotations first");
            } else if switch.clicked() {
                action = PanelAction::SwitchAuthor(name);
            }
        });
    });
    action
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
