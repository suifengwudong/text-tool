use std::collections::VecDeque;
use std::path::{Path, PathBuf};

mod models;
mod file_manager;
mod panel;
mod ui_helpers;

pub use models::*;
pub use file_manager::*;

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

    // ── Characters & Chapters (Panel::Characters) ────────────────────────────
    pub(super) characters: Vec<Character>,
    pub(super) selected_char_idx: Option<usize>,
    pub(super) new_char_name: String,
    pub(super) new_rel_target: String,
    pub(super) new_rel_kind: RelationKind,

    pub(super) chapters: Vec<Chapter>,
    pub(super) selected_chap_idx: Option<usize>,
    pub(super) new_chap_title: String,

    // ── Outline & Foreshadowing (Panel::Outline) ─────────────────────────────
    pub(super) foreshadows: Vec<Foreshadow>,
    pub(super) selected_fs_idx: Option<usize>,
    pub(super) new_fs_name: String,

    // ── LLM Assistance (Panel::LLM) ──────────────────────────────────────────
    pub(super) llm_config: LlmConfig,
    pub(super) llm_prompt: String,
    pub(super) llm_output: String,
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

    pub(super) fn open_project(&mut self, path: PathBuf) {
        // Ensure required subdirectories exist
        for sub in &["Content", "Design", "废稿"] {
            let _ = std::fs::create_dir_all(path.join(sub));
        }
        self.project_root = Some(path.clone());
        self.refresh_tree();
        self.status = format!("已打开项目: {}", path.display());
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

    /// Sync: save characters to Design/人物配置.json in the project.
    pub(super) fn sync_characters_to_json(&mut self) {
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
    pub(super) fn sync_chapters_to_json(&mut self) {
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
    pub(super) fn sync_foreshadows_to_md(&mut self) {
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
