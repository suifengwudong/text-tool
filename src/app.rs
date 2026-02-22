use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use egui::{Context, RichText, Color32, Key};
use serde::{Deserialize, Serialize};

// ── Character / Chapter / Foreshadow data ─────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RelationKind {
    Friend,   // 友好
    Enemy,    // 敌对
    Family,   // 亲属
    Other,    // 其他
}

impl RelationKind {
    fn label(&self) -> &'static str {
        match self {
            RelationKind::Friend => "友好",
            RelationKind::Enemy => "敌对",
            RelationKind::Family => "亲属",
            RelationKind::Other => "其他",
        }
    }
    fn all() -> &'static [RelationKind] {
        &[RelationKind::Friend, RelationKind::Enemy, RelationKind::Family, RelationKind::Other]
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Relationship {
    pub target: String,
    pub kind: RelationKind,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Character {
    pub name: String,
    pub traits: String,
    pub background: String,
    pub relationships: Vec<Relationship>,
}

impl Character {
    fn new(name: &str) -> Self {
        Character {
            name: name.to_owned(),
            traits: String::new(),
            background: String::new(),
            relationships: vec![],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ChapterTag {
    Normal,     // 普通
    Climax,     // 高潮
    Foreshadow, // 伏笔
    Transition, // 过渡
}

impl ChapterTag {
    fn label(&self) -> &'static str {
        match self {
            ChapterTag::Normal => "普通",
            ChapterTag::Climax => "高潮",
            ChapterTag::Foreshadow => "伏笔",
            ChapterTag::Transition => "过渡",
        }
    }
    fn all() -> &'static [ChapterTag] {
        &[ChapterTag::Normal, ChapterTag::Climax, ChapterTag::Foreshadow, ChapterTag::Transition]
    }
    fn color(&self) -> Color32 {
        match self {
            ChapterTag::Normal => Color32::from_gray(160),
            ChapterTag::Climax => Color32::from_rgb(220, 80, 80),
            ChapterTag::Foreshadow => Color32::from_rgb(80, 160, 220),
            ChapterTag::Transition => Color32::from_rgb(120, 190, 120),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chapter {
    pub title: String,
    pub tag: ChapterTag,
    pub summary: String,
    pub word_count: u32,
    pub done: bool,
}

impl Chapter {
    fn new(title: &str) -> Self {
        Chapter {
            title: title.to_owned(),
            tag: ChapterTag::Normal,
            summary: String::new(),
            word_count: 0,
            done: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Foreshadow {
    pub name: String,
    pub description: String,
    pub related_chapters: Vec<String>,
    pub resolved: bool,
}

impl Foreshadow {
    fn new(name: &str) -> Self {
        Foreshadow {
            name: name.to_owned(),
            description: String::new(),
            related_chapters: vec![],
            resolved: false,
        }
    }
}

// ── LLM config ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct LlmConfig {
    pub model_path: String,
    pub api_url: String,
    pub temperature: f32,
    pub max_tokens: u32,
    pub use_local: bool,
}


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

    // ── Characters & Chapters (Panel::Characters) ────────────────────────────
    characters: Vec<Character>,
    selected_char_idx: Option<usize>,
    new_char_name: String,
    new_rel_target: String,
    new_rel_kind: RelationKind,

    chapters: Vec<Chapter>,
    selected_chap_idx: Option<usize>,
    new_chap_title: String,

    // ── Outline & Foreshadowing (Panel::Outline) ─────────────────────────────
    foreshadows: Vec<Foreshadow>,
    selected_fs_idx: Option<usize>,
    new_fs_name: String,

    // ── LLM Assistance (Panel::LLM) ──────────────────────────────────────────
    llm_config: LlmConfig,
    llm_prompt: String,
    llm_output: String,
}

#[derive(Debug)]
struct NewFileDialog {
    name: String,
    dir: PathBuf,
}

impl TextToolApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // Load Chinese font
        let mut fonts = egui::FontDefinitions::default();
        fonts.font_data.insert(
            "chinese".to_owned(),
            egui::FontData::from_static(include_bytes!("../assets/NotoSansCJKsc-Regular.otf")),
        );
        fonts.families.get_mut(&egui::FontFamily::Proportional).unwrap().insert(0, "chinese".to_owned());
        fonts.families.get_mut(&egui::FontFamily::Monospace).unwrap().insert(0, "chinese".to_owned());
        cc.egui_ctx.set_fonts(fonts);

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
            characters: vec![],
            selected_char_idx: None,
            new_char_name: String::new(),
            new_rel_target: String::new(),
            new_rel_kind: RelationKind::Friend,
            chapters: vec![],
            selected_chap_idx: None,
            new_chap_title: String::new(),
            foreshadows: vec![],
            selected_fs_idx: None,
            new_fs_name: String::new(),
            llm_config: LlmConfig {
                model_path: String::new(),
                api_url: "http://localhost:11434/api/generate".to_owned(),
                temperature: 0.7,
                max_tokens: 512,
                use_local: true,
            },
            llm_prompt: String::new(),
            llm_output: String::new(),
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

    /// Sync: save characters to Design/人物配置.json in the project.
    fn sync_characters_to_json(&mut self) {
        if let Some(root) = &self.project_root {
            let path = root.join("Design").join("人物配置.json");
            match serde_json::to_string_pretty(&self.characters) {
                Ok(json) => {
                    if let Err(e) = std::fs::write(&path, &json) {
                        self.status = format!("保存人物配置失败: {e}");
                    } else {
                        self.status = "人物配置已同步到 Design/人物配置.json".to_owned();
                    }
                }
                Err(e) => self.status = format!("序列化失败: {e}"),
            }
        } else {
            self.status = "请先打开一个项目".to_owned();
        }
    }

    /// Sync: save chapters to Design/章节结构.json in the project.
    fn sync_chapters_to_json(&mut self) {
        if let Some(root) = &self.project_root {
            let path = root.join("Design").join("章节结构.json");
            match serde_json::to_string_pretty(&self.chapters) {
                Ok(json) => {
                    if let Err(e) = std::fs::write(&path, &json) {
                        self.status = format!("保存章节结构失败: {e}");
                    } else {
                        self.status = "章节结构已同步到 Design/章节结构.json".to_owned();
                    }
                }
                Err(e) => self.status = format!("序列化失败: {e}"),
            }
        } else {
            self.status = "请先打开一个项目".to_owned();
        }
    }

    /// Sync: save foreshadows to Content/伏笔.md in the project.
    fn sync_foreshadows_to_md(&mut self) {
        if let Some(root) = &self.project_root {
            let path = root.join("Content").join("伏笔.md");
            let mut md = String::from("# 伏笔列表\n\n");
            for fs in &self.foreshadows {
                let status = if fs.resolved { "✅ 已解决" } else { "⏳ 未解决" };
                md.push_str(&format!("## {} {}\n\n", fs.name, status));
                if !fs.description.is_empty() {
                    md.push_str(&format!("{}\n\n", fs.description));
                }
                if !fs.related_chapters.is_empty() {
                    md.push_str(&format!("**关联章节**: {}\n\n", fs.related_chapters.join("、")));
                }
            }
            if let Err(e) = std::fs::write(&path, &md) {
                self.status = format!("保存伏笔失败: {e}");
            } else {
                self.status = "伏笔已同步到 Content/伏笔.md".to_owned();
            }
        } else {
            self.status = "请先打开一个项目".to_owned();
        }
    }

    // ── Panel: Characters & Chapters ─────────────────────────────────────────

    fn draw_characters_panel(&mut self, ctx: &Context) {
        // Left side: character list
        egui::SidePanel::left("char_list")
            .resizable(true)
            .default_width(180.0)
            .min_width(120.0)
            .show(ctx, |ui| {
                ui.add_space(4.0);
                ui.heading("人物列表");
                ui.separator();

                egui::ScrollArea::vertical().id_salt("char_scroll").show(ui, |ui| {
                    let mut to_remove: Option<usize> = None;
                    for (i, ch) in self.characters.iter().enumerate() {
                        let selected = self.selected_char_idx == Some(i);
                        let resp = ui.selectable_label(selected, &ch.name);
                        resp.context_menu(|ui| {
                            if ui.button("删除").clicked() {
                                to_remove = Some(i);
                                ui.close_menu();
                            }
                        });
                        if resp.clicked() {
                            self.selected_char_idx = Some(i);
                        }
                    }
                    if let Some(idx) = to_remove {
                        self.characters.remove(idx);
                        if self.selected_char_idx == Some(idx) {
                            self.selected_char_idx = None;
                        } else if let Some(sel) = self.selected_char_idx {
                            if sel > idx { self.selected_char_idx = Some(sel - 1); }
                        }
                    }
                });

                ui.separator();
                ui.horizontal(|ui| {
                    ui.text_edit_singleline(&mut self.new_char_name)
                        .on_hover_text("输入人物名称");
                    if ui.button("➕").on_hover_text("添加人物").clicked() {
                        let name = self.new_char_name.trim().to_owned();
                        if !name.is_empty() {
                            let idx = self.characters.len();
                            self.characters.push(Character::new(&name));
                            self.selected_char_idx = Some(idx);
                            self.new_char_name.clear();
                        }
                    }
                });
            });

        // Central: character editor
        egui::CentralPanel::default().show(ctx, |ui| {
            // Top: chapter timeline
            let ch_height = 200.0;
            ui.group(|ui| {
                ui.set_min_height(ch_height);
                ui.heading("章节时间轴");
                ui.separator();

                ui.horizontal(|ui| {
                    ui.text_edit_singleline(&mut self.new_chap_title)
                        .on_hover_text("输入章节名称");
                    if ui.button("➕ 添加章节").clicked() {
                        let title = self.new_chap_title.trim().to_owned();
                        if !title.is_empty() {
                            let idx = self.chapters.len();
                            self.chapters.push(Chapter::new(&title));
                            self.selected_chap_idx = Some(idx);
                            self.new_chap_title.clear();
                        }
                    }
                    ui.separator();
                    if ui.button("💾 同步章节到 JSON").clicked() {
                        self.sync_chapters_to_json();
                    }
                });
                ui.add_space(4.0);

                // Timeline: horizontal scroll
                egui::ScrollArea::horizontal().id_salt("timeline_scroll").show(ui, |ui| {
                    ui.horizontal(|ui| {
                        let mut swap: Option<(usize, usize)> = None;
                        let mut remove: Option<usize> = None;
                        let count = self.chapters.len();
                        for i in 0..count {
                            let ch = &self.chapters[i];
                            let selected = self.selected_chap_idx == Some(i);
                            let frame_color = if selected {
                                Color32::from_rgb(0, 122, 204)
                            } else {
                                Color32::from_gray(50)
                            };
                            egui::Frame::none()
                                .fill(frame_color)
                                .inner_margin(6.0)
                                .rounding(4.0)
                                .show(ui, |ui| {
                                    ui.set_min_width(100.0);
                                    ui.vertical(|ui| {
                                        let label = ui.selectable_label(false,
                                            RichText::new(&ch.title).strong()
                                        );
                                        if label.clicked() {
                                            self.selected_chap_idx = Some(i);
                                        }
                                        ui.label(
                                            RichText::new(ch.tag.label())
                                                .color(ch.tag.color())
                                                .small()
                                        );
                                        let done_text = if ch.done { "✅" } else { "⏳" };
                                        ui.label(RichText::new(done_text).small());
                                        label.context_menu(|ui| {
                                            if i > 0 && ui.button("← 左移").clicked() {
                                                swap = Some((i - 1, i));
                                                ui.close_menu();
                                            }
                                            if i + 1 < count && ui.button("右移 →").clicked() {
                                                swap = Some((i, i + 1));
                                                ui.close_menu();
                                            }
                                            ui.separator();
                                            if ui.button("删除").clicked() {
                                                remove = Some(i);
                                                ui.close_menu();
                                            }
                                        });
                                    });
                                });
                            if i + 1 < count {
                                ui.label("→");
                            }
                        }
                        if let Some((a, b)) = swap {
                            self.chapters.swap(a, b);
                        }
                        if let Some(idx) = remove {
                            self.chapters.remove(idx);
                            if self.selected_chap_idx == Some(idx) {
                                self.selected_chap_idx = None;
                            } else if let Some(sel) = self.selected_chap_idx {
                                if sel > idx { self.selected_chap_idx = Some(sel - 1); }
                            }
                        }
                    });
                });

                // Chapter detail editor
                if let Some(idx) = self.selected_chap_idx {
                    if let Some(ch) = self.chapters.get_mut(idx) {
                        ui.separator();
                        ui.horizontal(|ui| {
                            ui.label("标题:");
                            ui.text_edit_singleline(&mut ch.title);
                            ui.label("标签:");
                            for tag in ChapterTag::all() {
                                let sel = &ch.tag == tag;
                                if ui.selectable_label(sel, tag.label()).clicked() {
                                    ch.tag = tag.clone();
                                }
                            }
                            ui.checkbox(&mut ch.done, "已完成");
                        });
                        ui.label("简介:");
                        ui.text_edit_multiline(&mut ch.summary);
                    }
                }
            });

            ui.add_space(8.0);

            // Bottom: selected character editor
            let mut do_char_sync = false;
            if let Some(idx) = self.selected_char_idx {
                // Extract fields to avoid simultaneous mutable borrows in closures
                let char_name = self.characters.get(idx).map(|c| c.name.clone()).unwrap_or_default();
                let mut do_add_rel = false;
                let mut remove_rel_idx: Option<usize> = None;

                if let Some(ch) = self.characters.get_mut(idx) {
                    egui::Frame::none()
                        .stroke(egui::Stroke::new(1.0, Color32::from_gray(60)))
                        .inner_margin(8.0)
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.strong(format!("人物: {}", char_name));
                                if ui.button("💾 同步人物到 JSON").clicked() {
                                    do_char_sync = true;
                                }
                            });
                            ui.separator();
                            ui.horizontal(|ui| {
                                ui.label("姓名:");
                                ui.text_edit_singleline(&mut ch.name);
                            });
                            ui.label("核心特质:");
                            ui.text_edit_multiline(&mut ch.traits);
                            ui.label("背景故事:");
                            ui.text_edit_multiline(&mut ch.background);

                            ui.add_space(4.0);
                            ui.label("人物关系:");
                            for (ri, rel) in ch.relationships.iter().enumerate() {
                                ui.horizontal(|ui| {
                                    ui.label(format!("  {} ── {} ──▶ {}", char_name, rel.kind.label(), rel.target));
                                    if ui.small_button("🗑").clicked() {
                                        remove_rel_idx = Some(ri);
                                    }
                                });
                            }
                            if let Some(ri) = remove_rel_idx {
                                ch.relationships.remove(ri);
                            }
                        });
                }

                // Add-relationship row (needs both ch and self.new_rel_*)
                ui.horizontal(|ui| {
                    ui.label("添加关系:");
                    ui.text_edit_singleline(&mut self.new_rel_target)
                        .on_hover_text("目标人物名称");
                    for kind in RelationKind::all() {
                        let sel = &self.new_rel_kind == kind;
                        if ui.selectable_label(sel, kind.label()).clicked() {
                            self.new_rel_kind = kind.clone();
                        }
                    }
                    if ui.button("➕").clicked() {
                        let target = self.new_rel_target.trim().to_owned();
                        if !target.is_empty() {
                            do_add_rel = true;
                        }
                    }
                });

                if do_add_rel {
                    let target = self.new_rel_target.trim().to_owned();
                    let kind = self.new_rel_kind.clone();
                    if let Some(ch) = self.characters.get_mut(idx) {
                        ch.relationships.push(Relationship { target, kind });
                    }
                    self.new_rel_target.clear();
                }
            } else if self.characters.is_empty() {
                ui.centered_and_justified(|ui| {
                    ui.label(RichText::new("← 在左侧添加人物以开始编辑").color(Color32::GRAY));
                });
            } else {
                ui.centered_and_justified(|ui| {
                    ui.label(RichText::new("← 点击左侧人物名称以编辑").color(Color32::GRAY));
                });
            }

            if do_char_sync {
                self.sync_characters_to_json();
            }
        });
    }

    // ── Panel: Outline & Foreshadowing ────────────────────────────────────────

    fn draw_outline_panel(&mut self, ctx: &Context) {
        // Left: outline tree derived from left_file (markdown) if open
        egui::SidePanel::left("outline_tree")
            .resizable(true)
            .default_width(220.0)
            .min_width(140.0)
            .show(ctx, |ui| {
                ui.add_space(4.0);
                ui.heading("大纲树");
                ui.separator();

                if let Some(lf) = &self.left_file {
                    if lf.is_markdown() {
                        let outline = parse_outline(&lf.content);
                        if outline.is_empty() {
                            ui.label(RichText::new("Markdown 文件中暂无标题").color(Color32::GRAY));
                        } else {
                            egui::ScrollArea::vertical().id_salt("outline_tree_scroll").show(ui, |ui| {
                                Self::draw_outline_entries(ui, &outline, 0);
                            });
                        }
                    } else {
                        ui.label(RichText::new("请在小说编辑面板打开 .md 文件").color(Color32::GRAY));
                    }
                } else {
                    ui.label(RichText::new("请先在小说编辑面板\n打开 Markdown 文件").color(Color32::GRAY));
                }
            });

        // Central: foreshadowing + progress
        egui::CentralPanel::default().show(ctx, |ui| {
            // Progress summary
            ui.group(|ui| {
                ui.heading("进度追踪");
                ui.separator();
                let total = self.chapters.len();
                let done = self.chapters.iter().filter(|c| c.done).count();
                if total == 0 {
                    ui.label(RichText::new("暂无章节，请在人设&章节面板添加").color(Color32::GRAY));
                } else {
                    ui.horizontal(|ui| {
                        ui.label(format!("章节完成度: {done}/{total}"));
                        let progress = done as f32 / total as f32;
                        ui.add(egui::ProgressBar::new(progress).desired_width(200.0));
                    });
                    let pending: Vec<&str> = self.chapters.iter()
                        .filter(|c| !c.done)
                        .map(|c| c.title.as_str())
                        .collect();
                    if !pending.is_empty() {
                        ui.label(format!("待写: {}", pending.join("、")));
                    }
                }
            });

            ui.add_space(8.0);

            // Foreshadowing
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    ui.heading("伏笔管理");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("💾 同步到 MD").clicked() {
                            self.sync_foreshadows_to_md();
                        }
                    });
                });
                ui.separator();

                ui.horizontal(|ui| {
                    ui.text_edit_singleline(&mut self.new_fs_name)
                        .on_hover_text("输入伏笔名称");
                    if ui.button("➕ 添加伏笔").clicked() {
                        let name = self.new_fs_name.trim().to_owned();
                        if !name.is_empty() {
                            let idx = self.foreshadows.len();
                            self.foreshadows.push(Foreshadow::new(&name));
                            self.selected_fs_idx = Some(idx);
                            self.new_fs_name.clear();
                        }
                    }
                });

                ui.add_space(4.0);

                ui.columns(2, |cols| {
                    // Foreshadow list
                    cols[0].label("伏笔列表:");
                    egui::ScrollArea::vertical().id_salt("fs_list_scroll").show(&mut cols[0], |ui| {
                        let mut to_remove: Option<usize> = None;
                        for (i, fs) in self.foreshadows.iter().enumerate() {
                            let selected = self.selected_fs_idx == Some(i);
                            let label = if fs.resolved {
                                format!("✅ {}", fs.name)
                            } else {
                                format!("⏳ {}", fs.name)
                            };
                            let resp = ui.selectable_label(selected, &label);
                            resp.context_menu(|ui| {
                                if ui.button("删除").clicked() {
                                    to_remove = Some(i);
                                    ui.close_menu();
                                }
                            });
                            if resp.clicked() {
                                self.selected_fs_idx = Some(i);
                            }
                        }
                        if let Some(idx) = to_remove {
                            self.foreshadows.remove(idx);
                            if self.selected_fs_idx == Some(idx) {
                                self.selected_fs_idx = None;
                            } else if let Some(sel) = self.selected_fs_idx {
                                if sel > idx { self.selected_fs_idx = Some(sel - 1); }
                            }
                        }
                    });

                    // Foreshadow detail
                    if let Some(idx) = self.selected_fs_idx {
                        if let Some(fs) = self.foreshadows.get_mut(idx) {
                            cols[1].label("伏笔名称:");
                            cols[1].text_edit_singleline(&mut fs.name);
                            cols[1].add_space(4.0);
                            cols[1].label("描述:");
                            cols[1].text_edit_multiline(&mut fs.description);
                            cols[1].add_space(4.0);
                            cols[1].checkbox(&mut fs.resolved, "已解决/揭示");
                            cols[1].add_space(4.0);
                            cols[1].label("关联章节 (逗号分隔):");
                            let mut related = fs.related_chapters.join("、");
                            if cols[1].text_edit_singleline(&mut related).changed() {
                                fs.related_chapters = related
                                    .split(['，', '、', ','])
                                    .map(|s| s.trim().to_owned())
                                    .filter(|s| !s.is_empty())
                                    .collect();
                            }
                        }
                    } else {
                        cols[1].centered_and_justified(|ui| {
                            ui.label(RichText::new("选择左侧伏笔以编辑").color(Color32::GRAY));
                        });
                    }
                });
            });
        });
    }

    fn draw_outline_entries(ui: &mut egui::Ui, entries: &[OutlineEntry], depth: usize) {
        let indent = depth as f32 * 16.0;
        for entry in entries {
            ui.horizontal(|ui| {
                ui.add_space(indent);
                let prefix = match entry.level {
                    1 => "📖",
                    2 => "📑",
                    _ => "•",
                };
                ui.label(format!("{prefix} {}", entry.title));
            });
            if !entry.children.is_empty() {
                Self::draw_outline_entries(ui, &entry.children, depth + 1);
            }
        }
    }

    // ── Panel: LLM Assistance ─────────────────────────────────────────────────

    fn draw_llm_panel(&mut self, ctx: &Context) {
        egui::SidePanel::left("llm_config")
            .resizable(true)
            .default_width(240.0)
            .min_width(160.0)
            .show(ctx, |ui| {
                ui.add_space(4.0);
                ui.heading("LLM 配置");
                ui.separator();

                ui.checkbox(&mut self.llm_config.use_local, "使用本地模型");
                ui.add_space(4.0);

                if self.llm_config.use_local {
                    ui.label("模型路径:");
                    ui.text_edit_singleline(&mut self.llm_config.model_path)
                        .on_hover_text("本地模型文件路径 (.gguf 等)");
                } else {
                    ui.label("API 地址:");
                    ui.text_edit_singleline(&mut self.llm_config.api_url)
                        .on_hover_text("如 http://localhost:11434/api/generate");
                }

                ui.add_space(8.0);
                ui.label(format!("温度 (Temperature): {:.2}", self.llm_config.temperature));
                ui.add(egui::Slider::new(&mut self.llm_config.temperature, 0.0..=2.0)
                    .step_by(0.05));

                ui.add_space(4.0);
                ui.label(format!("最大 Token: {}", self.llm_config.max_tokens));
                ui.add(egui::Slider::new(&mut self.llm_config.max_tokens, 64..=2048)
                    .step_by(64.0));

                ui.add_space(8.0);
                ui.separator();
                ui.label(RichText::new("支持模型:\nLlama 2 7B、Phi-2\n等本地轻量模型\n或兼容 OpenAI API\n的云端服务")
                    .color(Color32::from_gray(140))
                    .small());
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("LLM 辅助写作");
            ui.separator();

            ui.label("提示词 / 上下文:");
            egui::ScrollArea::vertical()
                .id_salt("llm_prompt_scroll")
                .max_height(200.0)
                .show(ui, |ui| {
                    ui.add(
                        egui::TextEdit::multiline(&mut self.llm_prompt)
                            .desired_width(f32::INFINITY)
                            .desired_rows(8)
                            .hint_text("输入提示词，例如：\n续写以下场景：\n或 优化以下对话：")
                    );
                });

            ui.add_space(4.0);
            ui.horizontal(|ui| {
                if ui.button("▶ 调用 LLM 补全").clicked() {
                    self.llm_output = self.llm_simulate();
                    self.status = "LLM 补全完成（模拟）".to_owned();
                }
                if ui.button("插入到左侧编辑区").clicked() {
                    if !self.llm_output.is_empty() {
                        if let Some(lf) = &mut self.left_file {
                            lf.content.push_str("\n\n");
                            lf.content.push_str(&self.llm_output);
                            lf.modified = true;
                            self.status = "已将 LLM 输出插入左侧编辑区".to_owned();
                        } else {
                            self.status = "请先在小说编辑面板打开 Markdown 文件".to_owned();
                        }
                    }
                }
                if ui.button("🗑 清空").clicked() {
                    self.llm_prompt.clear();
                    self.llm_output.clear();
                }
            });

            ui.add_space(8.0);
            ui.label("输出结果:");
            egui::ScrollArea::vertical()
                .id_salt("llm_output_scroll")
                .show(ui, |ui| {
                    ui.add(
                        egui::TextEdit::multiline(&mut self.llm_output)
                            .desired_width(f32::INFINITY)
                            .desired_rows(12)
                            .hint_text("LLM 输出将显示在这里")
                    );
                });
        });
    }

    /// Placeholder LLM call – returns a simulated response.
    /// Replace with actual HTTP/FFI call when integrating a real model.
    fn llm_simulate(&self) -> String {
        if self.llm_prompt.trim().is_empty() {
            return "（提示词为空，请输入内容后再试）".to_owned();
        }
        format!(
            "【模拟输出 – 请配置真实模型】\n\n根据您的提示「{}…」，这里将显示模型生成的文本。\n\n当前配置:\n- {}: {}\n- 温度: {:.2}\n- 最大Token: {}",
            self.llm_prompt.chars().take(30).collect::<String>(),
            if self.llm_config.use_local { "本地模型" } else { "API" },
            if self.llm_config.use_local { &self.llm_config.model_path } else { &self.llm_config.api_url },
            self.llm_config.temperature,
            self.llm_config.max_tokens,
        )
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

        // UI layers always visible
        self.draw_menu_bar(ctx);
        self.draw_status_bar(ctx);
        self.draw_toolbar(ctx);

        // Content area switches based on active panel
        match self.active_panel {
            Panel::Novel => {
                self.draw_file_tree(ctx);
                self.draw_editors(ctx);
            }
            Panel::Characters => {
                self.draw_characters_panel(ctx);
            }
            Panel::Outline => {
                self.draw_outline_panel(ctx);
            }
            Panel::LLM => {
                self.draw_llm_panel(ctx);
            }
        }

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

    // ── New data-model tests ──────────────────────────────────────────────────

    #[test]
    fn test_character_new() {
        let ch = Character::new("张三");
        assert_eq!(ch.name, "张三");
        assert!(ch.traits.is_empty());
        assert!(ch.background.is_empty());
        assert!(ch.relationships.is_empty());
    }

    #[test]
    fn test_character_relationship() {
        let mut ch = Character::new("张三");
        ch.relationships.push(Relationship {
            target: "李四".to_owned(),
            kind: RelationKind::Friend,
        });
        assert_eq!(ch.relationships.len(), 1);
        assert_eq!(ch.relationships[0].target, "李四");
        assert_eq!(ch.relationships[0].kind, RelationKind::Friend);
    }

    #[test]
    fn test_chapter_new() {
        let chap = Chapter::new("第一章");
        assert_eq!(chap.title, "第一章");
        assert_eq!(chap.tag, ChapterTag::Normal);
        assert!(!chap.done);
    }

    #[test]
    fn test_chapter_tag_labels() {
        assert_eq!(ChapterTag::Climax.label(), "高潮");
        assert_eq!(ChapterTag::Foreshadow.label(), "伏笔");
        assert_eq!(ChapterTag::Transition.label(), "过渡");
        assert_eq!(ChapterTag::Normal.label(), "普通");
    }

    #[test]
    fn test_foreshadow_new() {
        let fs = Foreshadow::new("神秘礼物");
        assert_eq!(fs.name, "神秘礼物");
        assert!(!fs.resolved);
        assert!(fs.related_chapters.is_empty());
    }

    #[test]
    fn test_relation_kind_labels() {
        assert_eq!(RelationKind::Friend.label(), "友好");
        assert_eq!(RelationKind::Enemy.label(), "敌对");
        assert_eq!(RelationKind::Family.label(), "亲属");
        assert_eq!(RelationKind::Other.label(), "其他");
    }

    #[test]
    fn test_characters_json_serialization() {
        let mut ch = Character::new("主角");
        ch.traits = "勇敢、善良".to_owned();
        ch.relationships.push(Relationship {
            target: "反派".to_owned(),
            kind: RelationKind::Enemy,
        });
        let json = serde_json::to_string(&ch).unwrap();
        let deserialized: Character = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.name, "主角");
        assert_eq!(deserialized.relationships[0].kind, RelationKind::Enemy);
    }

    #[test]
    fn test_chapters_json_serialization() {
        let mut chap = Chapter::new("序章");
        chap.tag = ChapterTag::Foreshadow;
        chap.done = true;
        let json = serde_json::to_string(&chap).unwrap();
        let deserialized: Chapter = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.title, "序章");
        assert_eq!(deserialized.tag, ChapterTag::Foreshadow);
        assert!(deserialized.done);
    }
}
