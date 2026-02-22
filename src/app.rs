use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use egui::{Context, RichText, Color32, Key};
use serde::{Deserialize, Serialize};

// ── Panel IDs ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Panel {
    Novel,
    Characters,
    Outline,
    LLM,
}

impl Panel {
    fn icon(self) -> &'static str {
        match self {
            Panel::Novel => "📝",
            Panel::Characters => "👤",
            Panel::Outline => "🧭",
            Panel::LLM => "🤖",
        }
    }
    fn label(self) -> &'static str {
        match self {
            Panel::Novel => "小说编辑",
            Panel::Characters => "人设&章节",
            Panel::Outline => "大纲&伏笔",
            Panel::LLM => "LLM辅助",
        }
    }
}

// ── File tree node ────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct FileNode {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    pub expanded: bool,
    pub children: Vec<FileNode>,
}

impl FileNode {
    fn from_path(path: &Path) -> Option<Self> {
        let name = path.file_name()?.to_string_lossy().into_owned();
        if path.is_dir() {
            let mut children: Vec<FileNode> = std::fs::read_dir(path)
                .ok()?
                .filter_map(|e| e.ok())
                .filter_map(|e| FileNode::from_path(&e.path()))
                .collect();
            children.sort_by(|a, b| {
                b.is_dir.cmp(&a.is_dir).then(a.name.cmp(&b.name))
            });
            Some(FileNode {
                name,
                path: path.to_owned(),
                is_dir: true,
                expanded: true,
                children,
            })
        } else {
            Some(FileNode {
                name,
                path: path.to_owned(),
                is_dir: false,
                expanded: false,
                children: vec![],
            })
        }
    }
}

// ── Outline entry (used for JSON sync) ───────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutlineEntry {
    pub level: u8,
    pub title: String,
    pub children: Vec<OutlineEntry>,
}

/// Parse Markdown headings into a flat list, then nest them.
pub fn parse_outline(markdown: &str) -> Vec<OutlineEntry> {
    let mut entries: Vec<(u8, String)> = vec![];
    for line in markdown.lines() {
        if let Some(rest) = line.strip_prefix("######") {
            entries.push((6, rest.trim().to_owned()));
        } else if let Some(rest) = line.strip_prefix("#####") {
            entries.push((5, rest.trim().to_owned()));
        } else if let Some(rest) = line.strip_prefix("####") {
            entries.push((4, rest.trim().to_owned()));
        } else if let Some(rest) = line.strip_prefix("###") {
            entries.push((3, rest.trim().to_owned()));
        } else if let Some(rest) = line.strip_prefix("##") {
            entries.push((2, rest.trim().to_owned()));
        } else if let Some(rest) = line.strip_prefix('#') {
            if rest.starts_with(' ') || rest.is_empty() {
                entries.push((1, rest.trim().to_owned()));
            }
        }
    }
    nest_entries(&entries, 1)
}

fn nest_entries(flat: &[(u8, String)], depth: u8) -> Vec<OutlineEntry> {
    let mut result = vec![];
    let mut i = 0;
    while i < flat.len() {
        let (lvl, title) = &flat[i];
        if *lvl == depth {
            // collect children (next level)
            let mut j = i + 1;
            while j < flat.len() && flat[j].0 > depth {
                j += 1;
            }
            let children = nest_entries(&flat[i + 1..j], depth + 1);
            result.push(OutlineEntry {
                level: depth,
                title: title.clone(),
                children,
            });
            i = j;
        } else if *lvl > depth {
            // skip - will be picked up by parent
            i += 1;
        } else {
            break;
        }
    }
    result
}

// ── Open file ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct OpenFile {
    pub path: PathBuf,
    pub content: String,
    pub modified: bool,
}

impl OpenFile {
    fn new(path: PathBuf, content: String) -> Self {
        OpenFile { path, content, modified: false }
    }

    fn save(&mut self) -> std::io::Result<()> {
        std::fs::write(&self.path, &self.content)?;
        self.modified = false;
        Ok(())
    }

    fn title(&self) -> String {
        let name = self.path.file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "untitled".to_owned());
        if self.modified {
            format!("● {name}")
        } else {
            name
        }
    }

    fn is_markdown(&self) -> bool {
        matches!(
            self.path.extension().and_then(|e| e.to_str()),
            Some("md") | Some("markdown")
        )
    }

    fn is_json(&self) -> bool {
        matches!(
            self.path.extension().and_then(|e| e.to_str()),
            Some("json")
        )
    }
}

// ── Application state ─────────────────────────────────────────────────────────

pub struct TextToolApp {
    // Panel
    active_panel: Panel,

    // Project
    project_root: Option<PathBuf>,
    file_tree: Vec<FileNode>,

    // Editors
    left_file: Option<OpenFile>,
    right_file: Option<OpenFile>,

    // Undo stacks (simple: store last content)
    left_undo_stack: VecDeque<String>,
    right_undo_stack: VecDeque<String>,

    // Track which editor pane was last focused for undo
    last_focused_left: bool,

    // Status bar message
    status: String,

    // New file dialog
    new_file_dialog: Option<NewFileDialog>,
}

#[derive(Debug)]
struct NewFileDialog {
    name: String,
    dir: PathBuf,
}

impl TextToolApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        TextToolApp {
            active_panel: Panel::Novel,
            project_root: None,
            file_tree: vec![],
            left_file: None,
            right_file: None,
            left_undo_stack: VecDeque::new(),
            right_undo_stack: VecDeque::new(),
            last_focused_left: true,
            status: "欢迎使用 Text Tool".to_owned(),
            new_file_dialog: None,
        }
    }

    // ── Project operations ────────────────────────────────────────────────────

    fn open_project(&mut self, path: PathBuf) {
        // Ensure required subdirectories exist
        for sub in &["Content", "Design", "废稿"] {
            let _ = std::fs::create_dir_all(path.join(sub));
        }
        self.project_root = Some(path.clone());
        self.refresh_tree();
        self.status = format!("已打开项目: {}", path.display());
    }

    fn refresh_tree(&mut self) {
        if let Some(root) = &self.project_root {
            self.file_tree = vec!["Content", "Design", "废稿"]
                .iter()
                .filter_map(|sub| {
                    let p = root.join(sub);
                    FileNode::from_path(&p)
                })
                .collect();
        }
    }

    // ── File operations ───────────────────────────────────────────────────────

    fn open_file_in_pane(&mut self, path: &Path, left: bool) {
        match std::fs::read_to_string(path) {
            Ok(content) => {
                let f = OpenFile::new(path.to_owned(), content);
                if left {
                self.left_file = Some(f);
                self.left_undo_stack.clear();
            } else {
                self.right_file = Some(f);
                self.right_undo_stack.clear();
            }
                self.status = format!("已打开: {}", path.display());
            }
            Err(e) => self.status = format!("打开失败: {e}"),
        }
    }

    fn save_left(&mut self) {
        if let Some(f) = &mut self.left_file {
            match f.save() {
                Ok(_) => self.status = format!("已保存: {}", f.path.display()),
                Err(e) => self.status = format!("保存失败: {e}"),
            }
        }
    }

    fn save_right(&mut self) {
        if let Some(f) = &mut self.right_file {
            match f.save() {
                Ok(_) => self.status = format!("已保存: {}", f.path.display()),
                Err(e) => self.status = format!("保存失败: {e}"),
            }
        }
    }

    fn new_file(&mut self, dir: PathBuf) {
        self.new_file_dialog = Some(NewFileDialog {
            name: String::new(),
            dir,
        });
    }

    fn create_file(&mut self, path: PathBuf) {
        if let Err(e) = std::fs::write(&path, "") {
            self.status = format!("创建失败: {e}");
        } else {
            self.refresh_tree();
            let open_in_left = !path.extension().and_then(|e| e.to_str()).eq(&Some("json"));
            self.open_file_in_pane(&path, open_in_left);
            self.status = format!("已创建: {}", path.display());
        }
    }

    /// Sync: generate outline JSON from the left markdown pane.
    fn sync_outline_to_right(&mut self) {
        let outline = if let Some(lf) = &self.left_file {
            if lf.is_markdown() {
                Some(parse_outline(&lf.content))
            } else {
                None
            }
        } else {
            None
        };

        if let Some(entries) = outline {
            let json = serde_json::to_string_pretty(&entries)
                .unwrap_or_else(|_| "[]".to_owned());
            if let Some(rf) = &mut self.right_file {
                if rf.is_json() {
                    rf.content = json;
                    rf.modified = true;
                    self.status = "已从 Markdown 同步大纲到 JSON".to_owned();
                    return;
                }
            }
            self.status = "请先在右侧打开一个 JSON 文件".to_owned();
        } else {
            self.status = "请先在左侧打开一个 Markdown 文件".to_owned();
        }
    }

    // ── UI helpers ────────────────────────────────────────────────────────────

    fn draw_menu_bar(&mut self, ctx: &Context) {
        egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("文件", |ui| {
                    if ui.button("打开项目文件夹…").clicked() {
                        if let Some(path) = rfd_pick_folder() {
                            self.open_project(path);
                        }
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("新建文件…").clicked() {
                        if let Some(root) = self.project_root.clone() {
                            self.new_file(root);
                        } else {
                            self.status = "请先打开一个项目".to_owned();
                        }
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("保存左侧  Ctrl+S").clicked() {
                        self.save_left();
                        ui.close_menu();
                    }
                    if ui.button("保存右侧  Ctrl+Shift+S").clicked() {
                        self.save_right();
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("导出左侧文件…").clicked() {
                        self.export_left();
                        ui.close_menu();
                    }
                    if ui.button("导出右侧文件…").clicked() {
                        self.export_right();
                        ui.close_menu();
                    }
                });

                ui.menu_button("视图", |ui| {
                    for panel in [Panel::Novel, Panel::Characters, Panel::Outline, Panel::LLM] {
                        let label = format!("{} {}", panel.icon(), panel.label());
                        let selected = self.active_panel == panel;
                        if ui.selectable_label(selected, label).clicked() {
                            self.active_panel = panel;
                            ui.close_menu();
                        }
                    }
                });

                ui.menu_button("工具", |ui| {
                    if ui.button("同步大纲 (MD → JSON)").clicked() {
                        self.sync_outline_to_right();
                        ui.close_menu();
                    }
                });
            });
        });
    }

    fn draw_toolbar(&mut self, ctx: &Context) {
        egui::SidePanel::left("toolbar")
            .resizable(false)
            .exact_width(48.0)
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(8.0);
                    for panel in [Panel::Novel, Panel::Characters, Panel::Outline, Panel::LLM] {
                        let selected = self.active_panel == panel;
                        let btn = egui::Button::new(
                            RichText::new(panel.icon()).size(22.0)
                        )
                        .fill(if selected {
                            Color32::from_rgb(0, 122, 204)
                        } else {
                            Color32::TRANSPARENT
                        })
                        .frame(true);

                        if ui.add_sized([40.0, 40.0], btn)
                            .on_hover_text(panel.label())
                            .clicked()
                        {
                            self.active_panel = panel;
                        }
                        ui.add_space(4.0);
                    }
                });
            });
    }

    fn draw_file_tree(&mut self, ctx: &Context) {
        let mut open_left: Option<PathBuf> = None;
        let mut open_right: Option<PathBuf> = None;
        let mut new_in: Option<PathBuf> = None;

        egui::SidePanel::left("file_tree")
            .resizable(true)
            .default_width(200.0)
            .min_width(120.0)
            .show(ctx, |ui| {
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.heading("项目");
                    if self.project_root.is_none() {
                        if ui.small_button("📂 打开").clicked() {
                            if let Some(path) = rfd_pick_folder() {
                                self.open_project(path);
                            }
                        }
                    } else if ui.small_button("🔄").on_hover_text("刷新").clicked() {
                        self.refresh_tree();
                    }
                });
                ui.separator();

                if self.project_root.is_none() {
                    ui.label(RichText::new("尚未打开项目").color(Color32::GRAY));
                    return;
                }

                egui::ScrollArea::vertical().show(ui, |ui| {
                    let nodes = self.file_tree.clone();
                    for node in &nodes {
                        Self::draw_tree_node(
                            ui, node, 0,
                            &mut open_left, &mut open_right, &mut new_in,
                        );
                    }
                });
            });

        // Apply deferred actions
        if let Some(p) = open_left {
            self.open_file_in_pane(&p, true);
        }
        if let Some(p) = open_right {
            self.open_file_in_pane(&p, false);
        }
        if let Some(p) = new_in {
            self.new_file(p);
        }
    }

    fn draw_tree_node(
        ui: &mut egui::Ui,
        node: &FileNode,
        depth: usize,
        open_left: &mut Option<PathBuf>,
        open_right: &mut Option<PathBuf>,
        new_in: &mut Option<PathBuf>,
    ) {
        let indent = depth as f32 * 12.0;
        ui.horizontal(|ui| {
            ui.add_space(indent);
            if node.is_dir {
                let icon = if node.expanded { "▼" } else { "▶" };
                ui.label(format!("{icon} 📁 {}", node.name));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.small_button("➕").on_hover_text("新建文件").clicked() {
                        *new_in = Some(node.path.clone());
                    }
                });
            } else {
                let icon = if node.name.ends_with(".md") || node.name.ends_with(".markdown") {
                    "📄"
                } else if node.name.ends_with(".json") {
                    "📋"
                } else {
                    "📃"
                };
                let resp = ui.selectable_label(false, format!("{icon} {}", node.name));
                resp.context_menu(|ui| {
                    if ui.button("在左侧打开").clicked() {
                        *open_left = Some(node.path.clone());
                        ui.close_menu();
                    }
                    if ui.button("在右侧打开").clicked() {
                        *open_right = Some(node.path.clone());
                        ui.close_menu();
                    }
                });
                if resp.double_clicked() {
                    // default: md → left, json → right
                    if node.name.ends_with(".json") {
                        *open_right = Some(node.path.clone());
                    } else {
                        *open_left = Some(node.path.clone());
                    }
                }
                resp.on_hover_text("双击打开 / 右键菜单");
            }
        });

        if node.is_dir && node.expanded {
            for child in &node.children {
                Self::draw_tree_node(ui, child, depth + 1, open_left, open_right, new_in);
            }
        }
    }

    fn draw_editors(&mut self, ctx: &Context) {
        // Sync flag
        let mut do_sync = false;

        egui::CentralPanel::default().show(ctx, |ui| {
            // Toolbar row above editors
            ui.horizontal(|ui| {
                ui.label(RichText::new("编辑区").strong());
                ui.separator();
                if ui.button("⟳ 同步大纲").on_hover_text("从左侧 Markdown 生成右侧 JSON 大纲").clicked() {
                    do_sync = true;
                }
            });
            ui.separator();

            let available = ui.available_size();

            ui.columns(2, |cols| {
                // Left pane - Markdown
                let left_title = self.left_file.as_ref()
                    .map(|f| f.title())
                    .unwrap_or_else(|| "左侧 (Markdown)".to_owned());

                cols[0].group(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(&left_title).strong());
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.small_button("💾").on_hover_text("保存 (Ctrl+S)").clicked() {
                                self.save_left();
                            }
                        });
                    });
                    ui.separator();

                    let height = available.y - 60.0;
                    if let Some(f) = &mut self.left_file {
                        let prev = f.content.clone();
                        egui::ScrollArea::both()
                            .id_salt("left_editor")
                            .show(ui, |ui| {
                                let editor = egui::TextEdit::multiline(&mut f.content)
                                    .desired_width(f32::INFINITY)
                                    .desired_rows(30)
                                    .min_size(egui::vec2(0.0, height))
                                    .font(egui::TextStyle::Monospace)
                                    .code_editor();
                                let resp = ui.add(editor);
                                if resp.has_focus() {
                                    self.last_focused_left = true;
                                }
                                if resp.changed() {
                                    if prev != f.content {
                                        self.left_undo_stack.push_back(prev);
                                        if self.left_undo_stack.len() > 200 {
                                            self.left_undo_stack.pop_front();
                                        }
                                    }
                                    f.modified = true;
                                }
                            });
                    } else {
                        ui.centered_and_justified(|ui| {
                            ui.label(RichText::new("双击文件树中的 .md 文件打开\n或从右键菜单选择\"在左侧打开\"")
                                .color(Color32::GRAY));
                        });
                    }
                });

                // Right pane - JSON
                let right_title = self.right_file.as_ref()
                    .map(|f| f.title())
                    .unwrap_or_else(|| "右侧 (JSON)".to_owned());

                cols[1].group(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(&right_title).strong());
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.small_button("💾").on_hover_text("保存 (Ctrl+Shift+S)").clicked() {
                                self.save_right();
                            }
                        });
                    });
                    ui.separator();

                    let height = available.y - 60.0;
                    if let Some(f) = &mut self.right_file {
                        let prev = f.content.clone();
                        egui::ScrollArea::both()
                            .id_salt("right_editor")
                            .show(ui, |ui| {
                                let editor = egui::TextEdit::multiline(&mut f.content)
                                    .desired_width(f32::INFINITY)
                                    .desired_rows(30)
                                    .min_size(egui::vec2(0.0, height))
                                    .font(egui::TextStyle::Monospace)
                                    .code_editor();
                                let resp = ui.add(editor);
                                if resp.has_focus() {
                                    self.last_focused_left = false;
                                }
                                if resp.changed() {
                                    if prev != f.content {
                                        self.right_undo_stack.push_back(prev);
                                        if self.right_undo_stack.len() > 200 {
                                            self.right_undo_stack.pop_front();
                                        }
                                    }
                                    f.modified = true;
                                }
                            });
                    } else {
                        ui.centered_and_justified(|ui| {
                            ui.label(RichText::new("双击文件树中的 .json 文件打开\n或从右键菜单选择\"在右侧打开\"")
                                .color(Color32::GRAY));
                        });
                    }
                });
            });
        });

        if do_sync {
            self.sync_outline_to_right();
        }
    }

    fn draw_status_bar(&self, ctx: &Context) {
        egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new(&self.status).color(Color32::from_gray(180)));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        RichText::new("Ctrl+S 保存  Ctrl+Z 撤销  Ctrl+Shift+S 保存右侧")
                            .color(Color32::from_gray(120))
                            .small(),
                    );
                });
            });
        });
    }

    fn draw_new_file_dialog(&mut self, ctx: &Context) {
        let mut create_path: Option<PathBuf> = None;
        let mut close = false;

        if let Some(dlg) = &mut self.new_file_dialog {
            egui::Window::new("新建文件")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label("文件名（含扩展名，如 chapter1.md）：");
                    let resp = ui.text_edit_singleline(&mut dlg.name);
                    if resp.lost_focus() && ctx.input(|i| i.key_pressed(Key::Escape)) {
                        close = true;
                    }
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.button("创建").clicked() || (resp.lost_focus() && ctx.input(|i| i.key_pressed(Key::Enter))) {
                            let name = dlg.name.trim().to_owned();
                            if !name.is_empty() {
                                create_path = Some(dlg.dir.join(&name));
                            }
                            close = true;
                        }
                        if ui.button("取消").clicked() {
                            close = true;
                        }
                    });
                });
        }

        if close {
            self.new_file_dialog = None;
        }
        if let Some(p) = create_path {
            self.create_file(p);
        }
    }

    fn handle_keyboard(&mut self, ctx: &Context) {
        let input = ctx.input(|i| {
            let ctrl = i.modifiers.ctrl || i.modifiers.command;
            let shift = i.modifiers.shift;
            (
                ctrl && !shift && i.key_pressed(Key::S),   // Ctrl+S
                ctrl && shift && i.key_pressed(Key::S),    // Ctrl+Shift+S
                ctrl && !shift && i.key_pressed(Key::Z),   // Ctrl+Z
            )
        });
        if input.0 {
            self.save_left();
        }
        if input.1 {
            self.save_right();
        }
        if input.2 {
            // Undo: apply to the last focused pane first
            if self.last_focused_left {
                if let Some(prev) = self.left_undo_stack.pop_back() {
                    if let Some(f) = &mut self.left_file {
                        f.content = prev;
                        f.modified = true;
                        self.status = "撤销 (左侧)".to_owned();
                    }
                }
            } else if let Some(prev) = self.right_undo_stack.pop_back() {
                if let Some(f) = &mut self.right_file {
                    f.content = prev;
                    f.modified = true;
                    self.status = "撤销 (右侧)".to_owned();
                }
            }
        }
    }

    fn export_left(&self) {
        if let Some(f) = &self.left_file {
            if let Some(dest) = rfd_save_file(&f.path) {
                if let Err(e) = std::fs::write(&dest, &f.content) {
                    eprintln!("导出失败: {e}");
                }
            }
        }
    }

    fn export_right(&self) {
        if let Some(f) = &self.right_file {
            if let Some(dest) = rfd_save_file(&f.path) {
                if let Err(e) = std::fs::write(&dest, &f.content) {
                    eprintln!("导出失败: {e}");
                }
            }
        }
    }
}

// ── eframe::App impl ──────────────────────────────────────────────────────────

impl eframe::App for TextToolApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        // Keyboard shortcuts (checked before UI to avoid conflicts)
        self.handle_keyboard(ctx);

        // UI layers
        self.draw_menu_bar(ctx);
        self.draw_status_bar(ctx);
        self.draw_toolbar(ctx);
        self.draw_file_tree(ctx);
        self.draw_editors(ctx);

        // Dialogs
        self.draw_new_file_dialog(ctx);
    }
}

// ── Thin wrappers around rfd (no-op stubs when rfd unavailable) ───────────────

fn rfd_pick_folder() -> Option<PathBuf> {
    // Use rfd if available; fall back to a simple path dialog via env.
    // We use a simple stdin fallback for headless environments.
    #[cfg(not(target_arch = "wasm32"))]
    {
        // Try rfd dialog first
        rfd::FileDialog::new().pick_folder()
    }
    #[cfg(target_arch = "wasm32")]
    {
        None
    }
}

fn rfd_save_file(hint: &Path) -> Option<PathBuf> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let ext = hint.extension().and_then(|e| e.to_str()).unwrap_or("txt");
        let name = hint.file_name().and_then(|n| n.to_str()).unwrap_or("file");
        rfd::FileDialog::new()
            .set_file_name(name)
            .add_filter("文件", &[ext])
            .save_file()
    }
    #[cfg(target_arch = "wasm32")]
    {
        None
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_outline_empty() {
        let result = parse_outline("");
        assert!(result.is_empty());
    }

    #[test]
    fn test_parse_outline_headings() {
        let md = "# Chapter 1\n## Scene 1\n## Scene 2\n# Chapter 2\n";
        let result = parse_outline(md);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].title, "Chapter 1");
        assert_eq!(result[0].children.len(), 2);
        assert_eq!(result[0].children[0].title, "Scene 1");
        assert_eq!(result[1].title, "Chapter 2");
        assert!(result[1].children.is_empty());
    }

    #[test]
    fn test_parse_outline_no_headings() {
        let md = "Just some text\nNo headings here.";
        let result = parse_outline(md);
        assert!(result.is_empty());
    }

    #[test]
    fn test_open_file_is_markdown() {
        let f = OpenFile::new(PathBuf::from("test.md"), String::new());
        assert!(f.is_markdown());
        let f2 = OpenFile::new(PathBuf::from("test.json"), String::new());
        assert!(!f2.is_markdown());
    }

    #[test]
    fn test_open_file_title_modified() {
        let mut f = OpenFile::new(PathBuf::from("test.md"), String::new());
        assert_eq!(f.title(), "test.md");
        f.modified = true;
        assert_eq!(f.title(), "● test.md");
    }
}
