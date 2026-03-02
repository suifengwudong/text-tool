use std::path::PathBuf;
use egui::{Context, RichText, Color32};
use super::super::{TextToolApp, FileNode, StructNode, FileTreeMode, rfd_pick_folder};
use super::markdown::render_markdown;

impl TextToolApp {
    // ── Novel panel: file tree + dual editors ─────────────────────────────────

    pub(in crate::app) fn draw_file_tree(&mut self, ctx: &Context) {
        let mut open_left: Option<PathBuf> = None;
        let mut open_right: Option<PathBuf> = None;
        let mut new_in: Option<PathBuf> = None;

        egui::SidePanel::left("file_tree")
            .resizable(true)
            .default_width(210.0)
            .min_width(130.0)
            .show(ctx, |ui| {
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.heading("导航");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        // Mode toggle: Files ↔ Chapter tree
                        let is_chapters = self.file_tree_mode == FileTreeMode::Chapters;
                        if ui.selectable_label(is_chapters, "📖 章节")
                            .on_hover_text("章节树视图（按结构导航）").clicked()
                        {
                            self.file_tree_mode = FileTreeMode::Chapters;
                        }
                        if ui.selectable_label(!is_chapters, "📁 文件")
                            .on_hover_text("文件系统视图").clicked()
                        {
                            self.file_tree_mode = FileTreeMode::Files;
                        }
                    });
                });
                ui.separator();

                if self.project_root.is_none() {
                    ui.vertical_centered(|ui| {
                        ui.add_space(8.0);
                        ui.label(RichText::new("尚未打开项目").color(Color32::GRAY));
                        ui.add_space(4.0);
                        if ui.button("📂 打开项目").clicked() {
                            if let Some(path) = rfd_pick_folder() {
                                // will be applied after the panel closes
                                open_left = Some(path.clone()); // reuse as signal
                                let _ = path; // handled below via special case
                            }
                        }
                    });
                } else {
                    // Refresh / new-file row
                    ui.horizontal(|ui| {
                        if ui.small_button("🔄").on_hover_text("刷新文件树").clicked() {
                            self.refresh_tree();
                        }
                        if self.file_tree_mode == FileTreeMode::Files {
                            if let Some(root) = self.project_root.clone() {
                                if ui.small_button("➕").on_hover_text("新建文件").clicked() {
                                    new_in = Some(root.join("Content"));
                                }
                            }
                        }
                    });
                    ui.separator();

                    egui::ScrollArea::vertical().id_salt("file_tree_scroll").show(ui, |ui| {
                        if self.file_tree_mode == FileTreeMode::Files {
                            let nodes = self.file_tree.clone();
                            for node in &nodes {
                                Self::draw_tree_node(
                                    ui, node, 0,
                                    &mut open_left, &mut open_right, &mut new_in,
                                );
                            }
                        } else {
                            // ── Chapter tree view ─────────────────────────────
                            if self.struct_roots.is_empty() {
                                ui.label(
                                    RichText::new("章节结构为空\n请先在「章节结构」面板\n添加章节，或点击\n「文件夹同步结构」")
                                        .small()
                                        .color(Color32::GRAY),
                                );
                            } else {
                                let roots = self.struct_roots.clone();
                                Self::draw_chapter_tree(
                                    ui, &roots, 0,
                                    &mut open_left,
                                    &self.project_root,
                                );
                            }
                        }
                    });
                }
            });

        // Apply deferred actions
        if let Some(p) = open_left {
            // Special case: if it's a directory, open as project
            if p.is_dir() && self.project_root.is_none() {
                self.open_project(p);
            } else {
                self.open_file_in_pane(&p, true);
            }
        }
        if let Some(p) = open_right {
            self.open_file_in_pane(&p, false);
        }
        if let Some(p) = new_in {
            self.new_file(p);
        }
    }

    /// Render the chapter structure tree. Clicking a leaf chapter opens its `.md` file.
    pub(in crate::app) fn draw_chapter_tree(
        ui: &mut egui::Ui,
        nodes: &[StructNode],
        depth: usize,
        open_left: &mut Option<PathBuf>,
        project_root: &Option<std::path::PathBuf>,
    ) {
        for node in nodes {
            let indent = depth as f32 * 14.0;
            let has_children = !node.children.is_empty();
            let done_mark = if node.done { "✅ " } else { "" };
            let label = format!("{} {}{}", node.kind.icon(), done_mark, node.title);

            ui.horizontal(|ui| {
                ui.add_space(indent);
                if has_children {
                    // Branch node: show as non-clickable label in muted color
                    ui.label(
                        RichText::new(&label)
                            .color(Color32::from_gray(200))
                            .strong(),
                    );
                } else {
                    // Leaf node: clickable, tries to open the corresponding .md
                    let resp = ui.selectable_label(false,
                        RichText::new(&label).color(Color32::from_gray(230))
                    );
                    if resp.clicked() || resp.double_clicked() {
                        // Look for matching .md file in Content/
                        if let Some(root) = project_root {
                            let needle = node.title.to_lowercase();
                            if let Some(path) = find_md_for_title(&root.join("Content"), &needle) {
                                *open_left = Some(path);
                            }
                        }
                    }
                    resp.on_hover_text(if node.summary.is_empty() {
                        "单击打开对应 Markdown 文件".to_owned()
                    } else {
                        node.summary.clone()
                    });
                }
            });

            if has_children {
                Self::draw_chapter_tree(ui, &node.children, depth + 1, open_left, project_root);
            }
        }
    }

    pub(in crate::app) fn draw_tree_node(
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

    pub(in crate::app) fn draw_editors(&mut self, ctx: &Context) {
        let mut do_extract_struct = false;
        let mut do_sync_folders   = false;

        egui::CentralPanel::default().show(ctx, |ui| {
            // Toolbar row above editors
            ui.horizontal(|ui| {
                ui.label(RichText::new("编辑区").strong());
                ui.separator();
                if ui.button("提取结构")
                    .on_hover_text("从左侧 Markdown 标题 (#/##/###) 提取章节结构到「章节结构」面板")
                    .clicked()
                {
                    do_extract_struct = true;
                }
                if ui.button("文件夹同步结构")
                    .on_hover_text("根据 Content/ 文件夹层级自动生成章节结构")
                    .clicked()
                {
                    do_sync_folders = true;
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        RichText::new("Tab=缩进  Ctrl+B=粗体  Ctrl+I=斜体  Ctrl+Z=撤销  Ctrl+S=保存")
                            .small()
                            .color(Color32::from_gray(120)),
                    );
                });
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
                        // Word count for markdown files
                        if let Some(f) = &self.left_file {
                            if f.is_markdown() {
                                let char_count: usize = f.content.chars()
                                    .filter(|c| !c.is_whitespace()).count();
                                ui.label(
                                    RichText::new(format!("文字数: {}", char_count))
                                        .small().color(Color32::from_gray(150))
                                );
                            }
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.small_button("💾").on_hover_text("保存 (Ctrl+S)").clicked() {
                                self.save_left();
                            }
                            // Preview toggle – only meaningful for Markdown files
                            let is_md = self.left_file.as_ref().map(|f| f.is_markdown()).unwrap_or(false);
                            if is_md {
                                let toggle_label = if self.left_preview_mode { "✏ 编辑" } else { "👁 预览" };
                                let hover = if self.left_preview_mode { "切换到编辑模式" } else { "切换到预览模式" };
                                if ui.small_button(toggle_label).on_hover_text(hover).clicked() {
                                    self.left_preview_mode = !self.left_preview_mode;
                                }
                            }
                        });
                    });
                    ui.separator();

                    let height = available.y - 60.0;
                    let is_preview = self.left_preview_mode
                        && self.left_file.as_ref().map(|f| f.is_markdown()).unwrap_or(false);

                    if is_preview {
                        // ── Markdown preview ──────────────────────────────────
                        if let Some(f) = &self.left_file {
                            // Use references to avoid cloning – both are immutable
                            // borrows of different fields, which Rust allows.
                            let content: &str = &f.content;
                            let settings = &self.md_settings;
                            egui::ScrollArea::vertical()
                                .id_salt("left_preview")
                                .show(ui, |ui| {
                                    ui.set_min_height(height);
                                    render_markdown(ui, content, settings);
                                });
                        }
                    } else if let Some(f) = &mut self.left_file {
                        // ── Plain text editor ─────────────────────────────────
                        let prev = f.content.clone();
                        egui::ScrollArea::both()
                            .id_salt("left_editor")
                            .show(ui, |ui| {
                                let editor = egui::TextEdit::multiline(&mut f.content)
                                    .id(egui::Id::new("left_editor_main"))
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
                            ui.label(RichText::new("双击文件树中的文件打开\n或从右键菜单选择\"在右侧打开\"")
                                .color(Color32::GRAY));
                        });
                    }
                });
            });
        });

        if do_extract_struct { self.extract_structure_from_left(); }
        if do_sync_folders   { self.sync_struct_from_folders(); }
    }
}

// ── Chapter tree file-finding helper ─────────────────────────────────────────

/// Recursively search `dir` for a `.md` file whose stem (lowercased) matches `needle`.
fn find_md_for_title(dir: &std::path::Path, needle: &str) -> Option<PathBuf> {
    let entries = std::fs::read_dir(dir).ok()?;
    let mut sorted: Vec<_> = entries.filter_map(|e| e.ok()).collect();
    sorted.sort_by_key(|e| e.file_name());
    for entry in sorted {
        let path = entry.path();
        if path.is_dir() {
            if let Some(found) = find_md_for_title(&path, needle) {
                return Some(found);
            }
        } else if path.extension().and_then(|e| e.to_str()) == Some("md") {
            let stem = path.file_stem()
                .map(|n| n.to_string_lossy().to_lowercase())
                .unwrap_or_default();
            if stem == needle {
                return Some(path);
            }
        }
    }
    None
}
