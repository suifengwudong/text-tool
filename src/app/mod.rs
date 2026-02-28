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

    // ── LLM Assistance (Panel::LLM) ──────────────────────────────────────────
    pub(super) llm_config: LlmConfig,
    pub(super) llm_prompt: String,
    pub(super) llm_output: String,

    // ── Markdown preview ─────────────────────────────────────────────────────
    pub(super) left_preview_mode: bool,
    pub(super) md_settings: MarkdownSettings,
    pub(super) show_settings_window: bool,
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
            llm_config: LlmConfig {
                model_path: String::new(),
                api_url: "http://localhost:11434/api/generate".to_owned(),
                temperature: 0.7,
                max_tokens: 512,
                use_local: true,
            },
            llm_prompt: String::new(),
            llm_output: String::new(),
            left_preview_mode: false,
            md_settings: MarkdownSettings::default(),
            show_settings_window: false,
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

    /// Sync: save world objects to Design/世界对象.json.
    pub(super) fn sync_world_objects_to_json(&mut self) {
        if let Some(root) = &self.project_root {
            let path = root.join("Design").join("世界对象.json");
            match serde_json::to_string_pretty(&self.world_objects) {
                Ok(json) => {
                    if let Err(e) = std::fs::write(&path, &json) {
                        self.status = format!("保存世界对象失败: {e}");
                    } else {
                        self.status = "世界对象已同步到 Design/世界对象.json".to_owned();
                    }
                }
                Err(e) => self.status = format!("序列化失败: {e}"),
            }
        } else {
            self.status = "请先打开一个项目".to_owned();
        }
    }

    /// Sync: save struct tree to Design/章节结构.json.
    pub(super) fn sync_struct_to_json(&mut self) {
        if let Some(root) = &self.project_root {
            let path = root.join("Design").join("章节结构.json");
            match serde_json::to_string_pretty(&self.struct_roots) {
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

    // ── Tree helpers ──────────────────────────────────────────────────────────

    /// Collect the names of all world objects for auto-complete / validation.
    pub(super) fn all_object_names(&self) -> Vec<String> {
        self.world_objects.iter().map(|o| o.name.clone()).collect()
    }

    /// Collect all structure node titles (depth-first).
    pub(super) fn all_struct_node_titles(&self) -> Vec<String> {
        all_node_titles(&self.struct_roots)
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
}
