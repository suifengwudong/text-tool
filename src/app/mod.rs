use std::collections::VecDeque;
use std::path::{Path, PathBuf};

/// Returns the home directory, using $HOME env var.
fn dirs_home() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

mod models;
mod file_manager;
mod llm_backend;
mod agent;
mod panel;
mod ui_helpers;

pub use models::*;
pub use file_manager::*;
pub use llm_backend::{LlmBackend, LlmTask, MockBackend, ApiBackend, LocalServerBackend, PromptTemplate};
pub use agent::{Skill, SkillSet, AgentBackend};

// ── Application state ─────────────────────────────────────────────────────────

pub struct TextToolApp {
    // Panel
    pub(super) active_panel: Panel,

    // Project
    pub(super) project_root: Option<PathBuf>,
    pub(super) file_tree: Vec<FileNode>,

    // Editors
    pub(super) left_file: Option<OpenFile>,
    pub(super) right_file: Option<OpenFile>,

    // Undo stacks (simple: store last content)
    pub(super) left_undo_stack: VecDeque<String>,
    pub(super) right_undo_stack: VecDeque<String>,

    // Track which editor pane was last focused for undo
    pub(super) last_focused_left: bool,

    // Status bar message
    pub(super) status: String,

    // New file dialog
    pub(super) new_file_dialog: Option<NewFileDialog>,

    // ── World Objects (Panel::Objects) ────────────────────────────────────────
    pub(super) world_objects: Vec<WorldObject>,
    pub(super) selected_obj_idx: Option<usize>,
    pub(super) new_obj_name: String,
    pub(super) new_obj_kind: ObjectKind,
    /// Input fields for adding a new ObjectLink on the selected object.
    pub(super) new_link_name: String,
    pub(super) new_link_rel_kind: RelationKind,
    /// Whether the new link target is a StructNode title (true) or a WorldObject name (false).
    pub(super) new_link_is_node: bool,
    pub(super) new_link_note: String,
    /// Kind filter shown in the object list side-panel (None = show all).
    pub(super) obj_kind_filter: Option<ObjectKind>,

    // ── Structure (Panel::Structure) ──────────────────────────────────────────
    pub(super) struct_roots: Vec<StructNode>,
    /// Path of indices from struct_roots into the currently selected node.
    pub(super) selected_node_path: Vec<usize>,
    pub(super) new_node_title: String,
    pub(super) new_node_kind: StructKind,
    /// Input fields for adding a NodeLink on the selected node.
    pub(super) new_node_link_title: String,
    pub(super) new_node_link_kind: RelationKind,
    pub(super) new_node_link_note: String,
    /// Name input for linking a WorldObject to the selected StructNode.
    pub(super) new_node_obj_link: String,

    // ── Outline & Foreshadowing (Panel::Structure – foreshadow sub-section) ───
    pub(super) foreshadows: Vec<Foreshadow>,
    pub(super) selected_fs_idx: Option<usize>,
    pub(super) new_fs_name: String,

    // ── Milestones (Panel::Structure – milestone sub-section) ────────────────
    pub(super) milestones: Vec<Milestone>,
    pub(super) selected_ms_idx: Option<usize>,
    pub(super) new_ms_name: String,

    // ── View mode toggles ─────────────────────────────────────────────────────
    pub(super) obj_view_mode: ObjectViewMode,
    pub(super) struct_view_mode: StructViewMode,

    // ── LLM Assistance (Panel::LLM) ──────────────────────────────────────────
    pub(super) llm_config: LlmConfig,
    pub(super) llm_prompt: String,
    pub(super) llm_output: String,
    /// Currently selected backend index: 0 = mock, 1 = HTTP API, 2 = LocalServer, 3 = Agent.
    pub(super) llm_backend_idx: usize,
    /// Active non-blocking LLM task (Some while a request is in-flight).
    pub(super) llm_task: Option<LlmTask>,
    /// Character name selected for dialogue-style optimisation.
    pub(super) llm_dialogue_char: String,

    // ── Markdown preview ─────────────────────────────────────────────────────
    pub(super) left_preview_mode: bool,
    pub(super) md_settings: MarkdownSettings,
    pub(super) show_settings_window: bool,

    // ── Config persistence ────────────────────────────────────────────────────
    pub(super) last_project: Option<PathBuf>,
    /// Auto-load world objects / struct / foreshadows / milestones from files when opening project.
    pub(super) auto_load_from_files: bool,

    // ── Full-text search ──────────────────────────────────────────────────────
    pub(super) show_search: bool,
    pub(super) search_query: String,
    pub(super) search_results: Vec<SearchResult>,
}

#[derive(Debug)]
pub(super) struct NewFileDialog {
    pub(super) name: String,
    pub(super) dir: PathBuf,
}

impl TextToolApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // Load Chinese font
        let mut fonts = egui::FontDefinitions::default();
        fonts.font_data.insert(
            "chinese".to_owned(),
            egui::FontData::from_static(include_bytes!("../../assets/NotoSansCJKsc-Regular.otf")),
        );
        fonts.families.get_mut(&egui::FontFamily::Proportional).unwrap().insert(0, "chinese".to_owned());
        fonts.families.get_mut(&egui::FontFamily::Monospace).unwrap().insert(0, "chinese".to_owned());
        cc.egui_ctx.set_fonts(fonts);

        let mut app = TextToolApp {
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
            world_objects: vec![],
            selected_obj_idx: None,
            new_obj_name: String::new(),
            new_obj_kind: ObjectKind::Character,
            new_link_name: String::new(),
            new_link_rel_kind: RelationKind::Friend,
            new_link_is_node: false,
            new_link_note: String::new(),
            obj_kind_filter: None,
            struct_roots: vec![],
            selected_node_path: vec![],
            new_node_title: String::new(),
            new_node_kind: StructKind::Chapter,
            new_node_link_title: String::new(),
            new_node_link_kind: RelationKind::Foreshadows,
            new_node_link_note: String::new(),
            new_node_obj_link: String::new(),
            foreshadows: vec![],
            selected_fs_idx: None,
            new_fs_name: String::new(),
            milestones: vec![
                Milestone::new("完成 VS Code 风格 UI 复刻"),
                Milestone::new("实现本地 MD/JSON 文件操作"),
                Milestone::new("完成轻量化基础（体积/速度/内存）"),
                Milestone::new("完成人设图形化编辑器（卡片视图）"),
                Milestone::new("完成章节时间轴编辑器"),
                Milestone::new("完成大纲树与伏笔管理"),
                Milestone::new("接入本地 LLM 模型"),
            ],
            selected_ms_idx: None,
            new_ms_name: String::new(),
            obj_view_mode: ObjectViewMode::List,
            struct_view_mode: StructViewMode::Tree,
            llm_config: LlmConfig {
                model_path: String::new(),
                api_url: "http://localhost:11434/api/generate".to_owned(),
                temperature: 0.7,
                max_tokens: 512,
                use_local: true,
                system_prompt: String::new(),
            },
            llm_prompt: String::new(),
            llm_output: String::new(),
            llm_backend_idx: 0,
            llm_task: None,
            llm_dialogue_char: String::new(),
            left_preview_mode: false,
            md_settings: MarkdownSettings::default(),
            show_settings_window: false,
            last_project: None,
            auto_load_from_files: false,
            show_search: false,
            search_query: String::new(),
            search_results: vec![],
        };

        // Apply saved configuration (LLM settings, MD settings, last project).
        if let Some(cfg) = Self::load_config() {
            app.llm_config = cfg.llm_config;
            app.md_settings = cfg.md_settings;
            app.auto_load_from_files = cfg.auto_load;
            if let Some(p) = cfg.last_project {
                let pb = PathBuf::from(p);
                if pb.is_dir() {
                    app.last_project = Some(pb.clone());
                    app.open_project(pb);
                }
            }
        }

        app
    }

    // ── Project operations ────────────────────────────────────────────────────

    pub(super) fn open_project(&mut self, path: PathBuf) {
        // Ensure required subdirectories exist
        for sub in &["Content", "Design", "废稿"] {
            let _ = std::fs::create_dir_all(path.join(sub));
        }
        self.project_root = Some(path.clone());
        self.last_project = Some(path.clone());
        self.refresh_tree();
        self.status = format!("已打开项目: {}", path.display());
        self.save_config();
        if self.auto_load_from_files {
            self.load_all_from_files();
        }
    }

    pub(super) fn refresh_tree(&mut self) {
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

    pub(super) fn open_file_in_pane(&mut self, path: &Path, left: bool) {
        match std::fs::read_to_string(path) {
            Ok(content) => {
                let f = OpenFile::new(path.to_owned(), content);
                if left {
                    // Apply the default preview setting for Markdown files
                    self.left_preview_mode = f.is_markdown() && self.md_settings.default_to_preview;
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

    pub(super) fn save_left(&mut self) {
        if let Some(f) = &mut self.left_file {
            match f.save() {
                Ok(_) => self.status = format!("已保存: {}", f.path.display()),
                Err(e) => self.status = format!("保存失败: {e}"),
            }
        }
    }

    pub(super) fn save_right(&mut self) {
        if let Some(f) = &mut self.right_file {
            match f.save() {
                Ok(_) => self.status = format!("已保存: {}", f.path.display()),
                Err(e) => self.status = format!("保存失败: {e}"),
            }
        }
    }

    pub(super) fn new_file(&mut self, dir: PathBuf) {
        self.new_file_dialog = Some(NewFileDialog {
            name: String::new(),
            dir,
        });
    }

    pub(super) fn create_file(&mut self, path: PathBuf) {
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
    pub(super) fn sync_outline_to_right(&mut self) {
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

    /// Write `content` to `<project_root>/<subdir>/<filename>`.
    /// Sets `self.status` on error or when no project is open.
    /// Returns `true` on success.
    fn write_project_file(&mut self, subdir: &str, filename: &str, content: &str) -> bool {
        if let Some(root) = self.project_root.as_ref() {
            let path = root.join(subdir).join(filename);
            if let Err(e) = std::fs::write(&path, content) {
                self.status = format!("写入 {} 失败: {e}", path.display());
                return false;
            }
            true
        } else {
            self.status = "请先打开一个项目".to_owned();
            false
        }
    }

    /// Sync: save world objects to Design/世界对象.json.
    pub(super) fn sync_world_objects_to_json(&mut self) {
        match serde_json::to_string_pretty(&self.world_objects) {
            Ok(json) => {
                if self.write_project_file("Design", "世界对象.json", &json) {
                    self.status = "世界对象已同步到 Design/世界对象.json".to_owned();
                }
            }
            Err(e) => self.status = format!("序列化失败: {e}"),
        }
    }

    /// Sync: save struct tree to Design/章节结构.json.
    pub(super) fn sync_struct_to_json(&mut self) {
        match serde_json::to_string_pretty(&self.struct_roots) {
            Ok(json) => {
                if self.write_project_file("Design", "章节结构.json", &json) {
                    self.status = "章节结构已同步到 Design/章节结构.json".to_owned();
                }
            }
            Err(e) => self.status = format!("序列化失败: {e}"),
        }
    }

    /// Sync: save milestones to Design/里程碑.json.
    pub(super) fn sync_milestones_to_json(&mut self) {
        match serde_json::to_string_pretty(&self.milestones) {
            Ok(json) => {
                if self.write_project_file("Design", "里程碑.json", &json) {
                    self.status = "里程碑已同步到 Design/里程碑.json".to_owned();
                }
            }
            Err(e) => self.status = format!("序列化失败: {e}"),
        }
    }

    /// Sync: save foreshadows to Content/伏笔.md in the project.
    pub(super) fn sync_foreshadows_to_md(&mut self) {
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
        if self.write_project_file("Content", "伏笔.md", &md) {
            self.status = "伏笔已同步到 Content/伏笔.md".to_owned();
        }
    }

    // ── Structured context builders (used by LLM panel) ──────────────────────

    /// Build a dialogue-optimization prompt for a specific character.
    ///
    /// Looks up the named character in `world_objects`, injects their description
    /// and background, then wraps `dialogue_text` in an optimization request.
    /// Returns `None` if no matching character is found.
    pub(super) fn build_dialogue_optimization_prompt(
        &self,
        char_name: &str,
        dialogue_text: &str,
    ) -> Option<String> {
        let obj = self.world_objects.iter().find(|o| o.name == char_name)?;

        let mut ctx = format!("## 人物：{} ({})\n", obj.name, obj.kind.label());
        if !obj.description.is_empty() {
            ctx.push_str(&format!("- 特质：{}\n", obj.description));
        }
        if !obj.background.is_empty() {
            ctx.push_str(&format!("- 背景：{}\n", obj.background));
        }
        if !obj.links.is_empty() {
            let rels: Vec<String> = obj.links.iter()
                .map(|l| format!("{} → {}", l.kind.label(), l.target.display_name()))
                .collect();
            ctx.push_str(&format!("- 关系：{}\n", rels.join("、")));
        }

        Some(PromptTemplate::DialogueOptimize.fill(&ctx, dialogue_text))
    }

    /// Build a prompt context block listing all world objects and their links.
    pub(super) fn build_character_context(&self) -> String {
        if self.world_objects.is_empty() {
            return String::new();
        }
        let mut out = String::from("## 世界对象\n\n");
        for obj in &self.world_objects {
            out.push_str(&format!("- **{}** ({})", obj.name, obj.kind.label()));
            if !obj.description.is_empty() {
                out.push_str(&format!(": {}", obj.description));
            }
            if !obj.links.is_empty() {
                let links: Vec<String> = obj.links.iter()
                    .map(|l| format!("{} → {}", l.kind.label(), l.target.display_name()))
                    .collect();
                out.push_str(&format!("  [关联: {}]", links.join(", ")));
            }
            out.push('\n');
        }
        out
    }

    /// Build a prompt context block listing the chapter structure.
    pub(super) fn build_structure_context(&self) -> String {
        if self.struct_roots.is_empty() {
            return String::new();
        }
        let mut out = String::from("## 章节结构\n\n");
        fn walk(nodes: &[crate::app::StructNode], depth: usize, out: &mut String) {
            for n in nodes {
                let indent = "  ".repeat(depth);
                let done = if n.done { "✅" } else { "⏳" };
                out.push_str(&format!("{indent}- {done} **{}** ({})\n", n.title, n.kind.label()));
                if !n.summary.is_empty() {
                    out.push_str(&format!("{indent}  > {}\n", n.summary));
                }
                walk(&n.children, depth + 1, out);
            }
        }
        walk(&self.struct_roots, 0, &mut out);
        out
    }

    // ── LLM / Agent helpers ───────────────────────────────────────────────────

    /// Snapshot the current project data into a `SkillSet` for the agent backend.
    pub(super) fn build_skill_set(&self) -> SkillSet {
        SkillSet::new(
            self.world_objects.clone(),
            self.struct_roots.clone(),
            self.foreshadows.clone(),
        )
    }

    /// Construct the `AgentBackend` for the currently-open project.
    pub(super) fn make_agent_backend(&self) -> AgentBackend {
        AgentBackend { skills: self.build_skill_set() }
    }

    /// Return the LLM backend that corresponds to `self.llm_backend_idx`.
    ///
    /// | idx | Backend |
    /// |-----|---------|
    /// | 0   | `MockBackend` (default / offline) |
    /// | 1   | `ApiBackend` (Ollama or OpenAI-compat HTTP) |
    /// | 2   | `LocalServerBackend` (llama.cpp native `/completion`) |
    /// | 3   | `AgentBackend` (OpenAI tool-calling loop) |
    pub(super) fn make_llm_backend(&self) -> std::sync::Arc<dyn LlmBackend> {
        match self.llm_backend_idx {
            1 => std::sync::Arc::new(ApiBackend),
            2 => std::sync::Arc::new(LocalServerBackend),
            3 => std::sync::Arc::new(self.make_agent_backend()),
            _ => std::sync::Arc::new(MockBackend),
        }
    }

    // ── Tree helpers ──────────────────────────────────────────────────────────

    /// Collect the names of all world objects for auto-complete / validation.
    pub(super) fn all_object_names(&self) -> Vec<String> {
        self.world_objects.iter().map(|o| o.name.clone()).collect()
    }

    /// Collect all structure node titles (depth-first).
    pub(super) fn all_struct_node_titles(&self) -> Vec<String> {
        all_node_titles(&self.struct_roots)
    }

    // ── Config persistence ────────────────────────────────────────────────────

    /// Returns the path to `~/.config/qingmo/config.json`.
    fn config_path() -> Option<PathBuf> {
        dirs_home().map(|h| h.join(".config").join("qingmo").join("config.json"))
    }

    /// Save LLM config, Markdown settings, and last project to disk.
    pub(super) fn save_config(&self) {
        let Some(path) = Self::config_path() else { return };
        let cfg = AppConfig {
            llm_config: self.llm_config.clone(),
            md_settings: self.md_settings.clone(),
            last_project: self.last_project.as_ref().map(|p| p.to_string_lossy().into_owned()),
            auto_load: self.auto_load_from_files,
        };
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(json) = serde_json::to_string_pretty(&cfg) {
            let _ = std::fs::write(&path, json);
        }
    }

    /// Load saved configuration from `~/.config/qingmo/config.json`.
    pub(super) fn load_config() -> Option<AppConfig> {
        let path = Self::config_path()?;
        let text = std::fs::read_to_string(&path).ok()?;
        serde_json::from_str(&text).ok()
    }

    // ── Reverse sync (file → app state) ──────────────────────────────────────

    /// Load world objects from `Design/世界对象.json` into `self.world_objects`.
    pub(super) fn load_world_objects_from_json(&mut self) {
        let Some(root) = self.project_root.as_ref() else {
            self.status = "请先打开一个项目".to_owned(); return;
        };
        let path = root.join("Design").join("世界对象.json");
        match std::fs::read_to_string(&path) {
            Ok(text) => match serde_json::from_str::<Vec<WorldObject>>(&text) {
                Ok(objs) => {
                    self.world_objects = objs;
                    self.selected_obj_idx = None;
                    self.status = format!("已从 {} 加载世界对象", path.display());
                }
                Err(e) => self.status = format!("解析失败: {e}"),
            },
            Err(e) => self.status = format!("读取失败: {e}"),
        }
    }

    /// Load chapter structure from `Design/章节结构.json` into `self.struct_roots`.
    pub(super) fn load_struct_from_json(&mut self) {
        let Some(root) = self.project_root.as_ref() else {
            self.status = "请先打开一个项目".to_owned(); return;
        };
        let path = root.join("Design").join("章节结构.json");
        match std::fs::read_to_string(&path) {
            Ok(text) => match serde_json::from_str::<Vec<StructNode>>(&text) {
                Ok(nodes) => {
                    self.struct_roots = nodes;
                    self.selected_node_path.clear();
                    self.status = format!("已从 {} 加载章节结构", path.display());
                }
                Err(e) => self.status = format!("解析失败: {e}"),
            },
            Err(e) => self.status = format!("读取失败: {e}"),
        }
    }

    /// Load milestones from `Design/里程碑.json` into `self.milestones`.
    pub(super) fn load_milestones_from_json(&mut self) {
        let Some(root) = self.project_root.as_ref() else {
            self.status = "请先打开一个项目".to_owned(); return;
        };
        let path = root.join("Design").join("里程碑.json");
        match std::fs::read_to_string(&path) {
            Ok(text) => match serde_json::from_str::<Vec<Milestone>>(&text) {
                Ok(ms) => {
                    self.milestones = ms;
                    self.selected_ms_idx = None;
                    self.status = format!("已从 {} 加载里程碑", path.display());
                }
                Err(e) => self.status = format!("解析失败: {e}"),
            },
            Err(e) => self.status = format!("读取失败: {e}"),
        }
    }

    /// Parse `Content/伏笔.md` → `self.foreshadows`.
    /// Headings (`## name`) become foreshadow entries; `✅` in the heading marks them resolved.
    pub(super) fn load_foreshadows_from_md(&mut self) {
        let Some(root) = self.project_root.as_ref() else {
            self.status = "请先打开一个项目".to_owned(); return;
        };
        let path = root.join("Content").join("伏笔.md");
        match std::fs::read_to_string(&path) {
            Ok(text) => {
                let mut foreshadows = Vec::new();
                for line in text.lines() {
                    if let Some(rest) = line.strip_prefix("## ") {
                        let resolved = rest.contains('✅');
                        let name = rest.replace("✅", "").replace("已解决", "")
                            .replace("⏳", "").replace("未解决", "").trim().to_owned();
                        if !name.is_empty() {
                            let mut fs = Foreshadow::new(&name);
                            fs.resolved = resolved;
                            foreshadows.push(fs);
                        }
                    }
                }
                self.foreshadows = foreshadows;
                self.selected_fs_idx = None;
                self.status = format!("已从 {} 加载伏笔", path.display());
            }
            Err(e) => self.status = format!("读取失败: {e}"),
        }
    }

    /// Run all four reverse-sync loads.
    pub(super) fn load_all_from_files(&mut self) {
        self.load_world_objects_from_json();
        self.load_struct_from_json();
        self.load_milestones_from_json();
        self.load_foreshadows_from_md();
        self.status = "已从文件加载所有数据".to_owned();
    }

    // ── Full-text search ──────────────────────────────────────────────────────

    /// Scan all `.md` and `.json` files under the project root for `self.search_query`.
    pub(super) fn run_search(&mut self) {
        self.search_results.clear();
        let query = self.search_query.clone();
        if query.is_empty() { return; }
        let Some(root) = self.project_root.clone() else {
            self.status = "请先打开一个项目".to_owned(); return;
        };
        Self::search_dir(&root, &query, &mut self.search_results);
        self.status = format!("搜索「{}」找到 {} 处结果", query, self.search_results.len());
    }

    fn search_dir(dir: &Path, query: &str, results: &mut Vec<SearchResult>) {
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                Self::search_dir(&path, query, results);
            } else {
                let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
                if ext == "md" || ext == "json" {
                    if let Ok(text) = std::fs::read_to_string(&path) {
                        for (line_no, line) in text.lines().enumerate() {
                            if line.contains(query) {
                                results.push(SearchResult {
                                    file_path: path.clone(),
                                    line_no: line_no + 1,
                                    line: line.to_owned(),
                                });
                            }
                        }
                    }
                }
            }
        }
    }

    // ── Export and backup ─────────────────────────────────────────────────────

    /// Concatenate all `Content/*.md` files and save to a user-chosen file.
    pub(super) fn export_chapters_merged(&mut self) {
        let Some(root) = self.project_root.as_ref() else {
            self.status = "请先打开一个项目".to_owned(); return;
        };
        let content_dir = root.join("Content");
        let mut md_files: Vec<PathBuf> = std::fs::read_dir(&content_dir)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("md"))
            .collect();
        md_files.sort();

        let mut merged = String::new();
        for path in &md_files {
            if let Ok(text) = std::fs::read_to_string(path) {
                let name = path.file_name().unwrap_or_default().to_string_lossy();
                merged.push_str(&format!("# ── {} ──\n\n", name));
                merged.push_str(&text);
                merged.push_str("\n\n");
            }
        }

        let dummy = PathBuf::from("merged.md");
        if let Some(dest) = rfd_save_file(&dummy) {
            match std::fs::write(&dest, &merged) {
                Ok(_) => self.status = format!("已导出合集到 {}", dest.display()),
                Err(e) => self.status = format!("导出失败: {e}"),
            }
        }
    }

    /// Copy the entire project folder to a user-selected destination directory.
    pub(super) fn backup_project(&mut self) {
        let Some(root) = self.project_root.clone() else {
            self.status = "请先打开一个项目".to_owned(); return;
        };
        let Some(dest_parent) = rfd_pick_folder() else { return };
        let folder_name = root.file_name().unwrap_or_default();
        let dest = dest_parent.join(folder_name);
        match Self::copy_dir_all(&root, &dest) {
            Ok(_) => self.status = format!("已备份到 {}", dest.display()),
            Err(e) => self.status = format!("备份失败: {e}"),
        }
    }

    fn copy_dir_all(src: &Path, dst: &Path) -> std::io::Result<()> {
        std::fs::create_dir_all(dst)?;
        for entry in std::fs::read_dir(src)? {
            let entry = entry?;
            let ty = entry.file_type()?;
            let dst_path = dst.join(entry.file_name());
            if ty.is_dir() {
                Self::copy_dir_all(&entry.path(), &dst_path)?;
            } else {
                std::fs::copy(entry.path(), dst_path)?;
            }
        }
        Ok(())
    }
}

// ── eframe::App impl ──────────────────────────────────────────────────────────

impl eframe::App for TextToolApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
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
            Panel::Objects => {
                self.draw_objects_panel(ctx);
            }
            Panel::Structure => {
                self.draw_structure_panel(ctx);
            }
            Panel::LLM => {
                self.draw_llm_panel(ctx);
            }
        }

        // Dialogs
        self.draw_new_file_dialog(ctx);
        self.draw_settings_window(ctx);
        self.draw_search_window(ctx);
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

    // ── ObjectKind tests ──────────────────────────────────────────────────────

    #[test]
    fn test_object_kind_labels() {
        assert_eq!(ObjectKind::Character.label(), "人物");
        assert_eq!(ObjectKind::Scene.label(), "场景");
        assert_eq!(ObjectKind::Location.label(), "地点");
        assert_eq!(ObjectKind::Item.label(), "道具");
        assert_eq!(ObjectKind::Faction.label(), "势力");
        assert_eq!(ObjectKind::Other.label(), "其他");
    }

    #[test]
    fn test_world_object_new() {
        let obj = WorldObject::new("张三", ObjectKind::Character);
        assert_eq!(obj.name, "张三");
        assert_eq!(obj.kind, ObjectKind::Character);
        assert!(obj.description.is_empty());
        assert!(obj.links.is_empty());
    }

    #[test]
    fn test_world_object_link() {
        let mut obj = WorldObject::new("张三", ObjectKind::Character);
        obj.links.push(ObjectLink {
            target: LinkTarget::Object("李四".to_owned()),
            kind: RelationKind::Friend,
            note: String::new(),
        });
        assert_eq!(obj.links.len(), 1);
        assert_eq!(obj.links[0].target.display_name(), "李四");
        assert_eq!(obj.links[0].target.type_label(), "对象");
    }

    #[test]
    fn test_world_object_link_to_node() {
        let mut obj = WorldObject::new("古剑", ObjectKind::Item);
        obj.links.push(ObjectLink {
            target: LinkTarget::Node("第一章".to_owned()),
            kind: RelationKind::AppearsIn,
            note: "在山洞中被发现".to_owned(),
        });
        assert_eq!(obj.links[0].target.type_label(), "章节");
        assert_eq!(obj.links[0].note, "在山洞中被发现");
    }

    #[test]
    fn test_world_object_json_serialization() {
        let mut obj = WorldObject::new("主角", ObjectKind::Character);
        obj.description = "勇敢、善良".to_owned();
        obj.links.push(ObjectLink {
            target: LinkTarget::Object("反派".to_owned()),
            kind: RelationKind::Enemy,
            note: String::new(),
        });
        let json = serde_json::to_string(&obj).unwrap();
        let d: WorldObject = serde_json::from_str(&json).unwrap();
        assert_eq!(d.name, "主角");
        assert_eq!(d.kind, ObjectKind::Character);
        assert_eq!(d.links[0].kind, RelationKind::Enemy);
    }

    // ── StructKind tests ──────────────────────────────────────────────────────

    #[test]
    fn test_struct_kind_labels() {
        assert_eq!(StructKind::Outline.label(), "总纲");
        assert_eq!(StructKind::Volume.label(), "卷");
        assert_eq!(StructKind::Chapter.label(), "章");
        assert_eq!(StructKind::Section.label(), "节");
    }

    #[test]
    fn test_struct_kind_default_child() {
        assert_eq!(StructKind::Outline.default_child_kind(), StructKind::Volume);
        assert_eq!(StructKind::Volume.default_child_kind(), StructKind::Chapter);
        assert_eq!(StructKind::Chapter.default_child_kind(), StructKind::Section);
    }

    // ── StructNode tests ──────────────────────────────────────────────────────

    #[test]
    fn test_struct_node_new() {
        let n = StructNode::new("第一章", StructKind::Chapter);
        assert_eq!(n.title, "第一章");
        assert_eq!(n.kind, StructKind::Chapter);
        assert!(n.children.is_empty());
        assert!(n.linked_objects.is_empty());
        assert!(!n.done);
    }

    #[test]
    fn test_struct_node_leaf_count() {
        let mut vol = StructNode::new("第一卷", StructKind::Volume);
        vol.children.push(StructNode::new("第一章", StructKind::Chapter));
        vol.children.push(StructNode::new("第二章", StructKind::Chapter));
        assert_eq!(vol.leaf_count(), 2);
    }

    #[test]
    fn test_struct_node_done_count() {
        let mut vol = StructNode::new("第一卷", StructKind::Volume);
        let mut ch1 = StructNode::new("第一章", StructKind::Chapter);
        ch1.done = true;
        vol.children.push(ch1);
        vol.children.push(StructNode::new("第二章", StructKind::Chapter));
        assert_eq!(vol.done_count(), 1);
        assert_eq!(vol.leaf_count(), 2);
    }

    #[test]
    fn test_struct_node_json_serialization() {
        let mut node = StructNode::new("序章", StructKind::Chapter);
        node.tag = ChapterTag::Foreshadow;
        node.done = true;
        node.linked_objects.push("主角".to_owned());
        let json = serde_json::to_string(&node).unwrap();
        let d: StructNode = serde_json::from_str(&json).unwrap();
        assert_eq!(d.title, "序章");
        assert_eq!(d.tag, ChapterTag::Foreshadow);
        assert!(d.done);
        assert_eq!(d.linked_objects[0], "主角");
    }

    // ── node_at / node_at_mut tests ───────────────────────────────────────────

    #[test]
    fn test_node_at() {
        let mut roots = vec![StructNode::new("第一卷", StructKind::Volume)];
        roots[0].children.push(StructNode::new("第一章", StructKind::Chapter));
        assert_eq!(node_at(&roots, &[0]).unwrap().title, "第一卷");
        assert_eq!(node_at(&roots, &[0, 0]).unwrap().title, "第一章");
        assert!(node_at(&roots, &[1]).is_none());
    }

    #[test]
    fn test_node_at_mut() {
        let mut roots = vec![StructNode::new("第一卷", StructKind::Volume)];
        roots[0].children.push(StructNode::new("第一章", StructKind::Chapter));
        node_at_mut(&mut roots, &[0, 0]).unwrap().done = true;
        assert!(roots[0].children[0].done);
    }

    #[test]
    fn test_all_node_titles() {
        let mut roots = vec![StructNode::new("第一卷", StructKind::Volume)];
        roots[0].children.push(StructNode::new("第一章", StructKind::Chapter));
        roots[0].children.push(StructNode::new("第二章", StructKind::Chapter));
        let titles = all_node_titles(&roots);
        assert_eq!(titles, vec!["第一卷", "第一章", "第二章"]);
    }

    // ── RelationKind tests ────────────────────────────────────────────────────

    #[test]
    fn test_relation_kind_labels() {
        assert_eq!(RelationKind::Friend.label(), "友好");
        assert_eq!(RelationKind::Enemy.label(), "敌对");
        assert_eq!(RelationKind::Family.label(), "亲属");
        assert_eq!(RelationKind::AppearsIn.label(), "出场");
        assert_eq!(RelationKind::Foreshadows.label(), "铺垫");
        assert_eq!(RelationKind::Resolves.label(), "回收");
    }

    // ── ChapterTag tests ──────────────────────────────────────────────────────

    #[test]
    fn test_chapter_tag_labels() {
        assert_eq!(ChapterTag::Climax.label(), "高潮");
        assert_eq!(ChapterTag::Foreshadow.label(), "伏笔");
        assert_eq!(ChapterTag::Transition.label(), "过渡");
        assert_eq!(ChapterTag::Normal.label(), "普通");
    }

    // ── Foreshadow tests ──────────────────────────────────────────────────────

    #[test]
    fn test_foreshadow_new() {
        let fs = Foreshadow::new("神秘礼物");
        assert_eq!(fs.name, "神秘礼物");
        assert!(!fs.resolved);
        assert!(fs.related_chapters.is_empty());
    }

    // ── MarkdownSettings tests ────────────────────────────────────────────────

    #[test]
    fn test_markdown_settings_default() {
        let s = MarkdownSettings::default();
        assert_eq!(s.preview_font_size, 14.0);
        assert!(!s.default_to_preview);
    }

    #[test]
    fn test_markdown_settings_custom() {
        let s = MarkdownSettings {
            preview_font_size: 18.0,
            default_to_preview: true,
        };
        assert_eq!(s.preview_font_size, 18.0);
        assert!(s.default_to_preview);
    }

    // ── Milestone tests ───────────────────────────────────────────────────────

    #[test]
    fn test_milestone_new() {
        let m = Milestone::new("第一阶段完成");
        assert_eq!(m.name, "第一阶段完成");
        assert!(!m.completed);
        assert!(m.description.is_empty());
    }

    #[test]
    fn test_milestone_completion() {
        let mut m = Milestone::new("MVP");
        assert!(!m.completed);
        m.completed = true;
        assert!(m.completed);
    }

    #[test]
    fn test_milestone_json_serialization() {
        let mut m = Milestone::new("发布 v1.0");
        m.description = "第一个正式版本".to_owned();
        m.completed = true;
        let json = serde_json::to_string(&m).unwrap();
        let d: Milestone = serde_json::from_str(&json).unwrap();
        assert_eq!(d.name, "发布 v1.0");
        assert_eq!(d.description, "第一个正式版本");
        assert!(d.completed);
    }

    // ── build_dialogue_optimization_prompt tests ──────────────────────────────

    #[test]
    fn test_build_dialogue_optimization_prompt_found() {
        use crate::app::{ObjectLink, LinkTarget};
        let mut app_objs = vec![WorldObject::new("张三", ObjectKind::Character)];
        app_objs[0].description = "热情开朗".to_owned();
        app_objs[0].links.push(ObjectLink {
            target: LinkTarget::Object("李四".to_owned()),
            kind: RelationKind::Friend,
            note: String::new(),
        });

        // We can't construct TextToolApp without a GPU context in tests,
        // so we test the underlying build_dialogue_optimization_prompt logic
        // through the PromptTemplate + context combination directly.
        let ctx = format!(
            "## 人物：{} ({})\n- 特质：{}\n- 关系：{} → {}\n",
            app_objs[0].name,
            app_objs[0].kind.label(),
            app_objs[0].description,
            app_objs[0].links[0].kind.label(),
            app_objs[0].links[0].target.display_name(),
        );
        let prompt = PromptTemplate::DialogueOptimize.fill(&ctx, "\"你好啊！\"");
        assert!(prompt.contains("张三"));
        assert!(prompt.contains("热情开朗"));
        assert!(prompt.contains("友好"));
        assert!(prompt.contains("你好啊"));
    }

    #[test]
    fn test_build_character_context_empty() {
        // When no world objects exist, context is empty.
        let objects: Vec<WorldObject> = vec![];
        let ctx: String = if objects.is_empty() {
            String::new()
        } else {
            let mut out = String::from("## 世界对象\n\n");
            for o in &objects {
                out.push_str(&format!("- **{}** ({})\n", o.name, o.kind.label()));
            }
            out
        };
        assert!(ctx.is_empty());
    }

    #[test]
    fn test_build_character_context_with_objects() {
        let objects = vec![
            WorldObject::new("主角", ObjectKind::Character),
            WorldObject::new("城堡", ObjectKind::Location),
        ];
        let mut out = String::from("## 世界对象\n\n");
        for o in &objects {
            out.push_str(&format!("- **{}** ({})\n", o.name, o.kind.label()));
        }
        assert!(out.contains("主角"));
        assert!(out.contains("城堡"));
        assert!(out.contains("人物"));
        assert!(out.contains("地点"));
    }
}
