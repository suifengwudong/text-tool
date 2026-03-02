use std::path::PathBuf;
use egui::{Context, RichText, Color32, Key};
use super::super::{TextToolApp, FileNode, StructNode, FileTreeMode, Panel, rfd_pick_folder};
use super::markdown::render_markdown;

impl TextToolApp {
    // ── Novel panel: file tree + dual editors ─────────────────────────────────

    pub(in crate::app) fn draw_file_tree(&mut self, ctx: &Context) {
        let mut open_left: Option<PathBuf> = None;
        let mut open_right: Option<PathBuf> = None;
        let mut new_in: Option<PathBuf> = None;
        let mut toggle_path: Option<PathBuf> = None;
        let mut select_path: Option<PathBuf> = None;
        let mut rename_path: Option<PathBuf> = None;

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
                    // F2 hint when a file is selected
                    if self.selected_file_path.is_some() && self.file_tree_mode == FileTreeMode::Files {
                        ui.label(
                            RichText::new("F2 重命名选中文件")
                                .small().color(Color32::from_gray(120)),
                        );
                    }
                    ui.separator();

                    egui::ScrollArea::vertical().id_salt("file_tree_scroll").show(ui, |ui| {
                        if self.file_tree_mode == FileTreeMode::Files {
                            let nodes = self.file_tree.clone();
                            let selected = &self.selected_file_path;
                            for node in &nodes {
                                Self::draw_tree_node(
                                    ui, node, 0,
                                    &mut open_left, &mut open_right, &mut new_in,
                                    &mut toggle_path, selected, &mut select_path,
                                    &mut rename_path,
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
        if let Some(p) = toggle_path {
            Self::toggle_expand_in_tree(&mut self.file_tree, &p);
        }
        if let Some(p) = select_path {
            self.selected_file_path = Some(p);
        }
        if let Some(p) = rename_path {
            let current_name = p.file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            self.selected_file_path = Some(p.clone());
            self.rename_dialog = Some(crate::app::RenameDialog {
                path: p,
                new_name: current_name,
            });
        }

        // Handle F2 key: open rename dialog for selected file when panel is focused
        if self.rename_dialog.is_none() {
            let f2 = ctx.input(|i| !i.modifiers.ctrl && !i.modifiers.shift && i.key_pressed(Key::F2));
            if f2 {
                if let Some(path) = self.selected_file_path.clone() {
                    let current_name = path.file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    self.rename_dialog = Some(crate::app::RenameDialog {
                        path,
                        new_name: current_name,
                    });
                }
            }
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
        for (idx, node) in nodes.iter().enumerate() {
            let done_mark = if node.done { "✅ " } else { "" };
            let label = format!("{} {}{}", node.kind.icon(), done_mark, node.title);

            if node.children.is_empty() {
                // Leaf node: clickable, tries to open the corresponding .md
                ui.horizontal(|ui| {
                    ui.add_space(depth as f32 * 14.0);
                    let resp = ui.selectable_label(false,
                        RichText::new(&label).color(Color32::from_gray(230))
                    );
                    if resp.clicked() || resp.double_clicked() {
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
                });
            } else {
                // Branch node: collapsible header (provides its own indentation)
                egui::CollapsingHeader::new(
                    RichText::new(&label).color(Color32::from_gray(200)).strong()
                )
                .id_salt(format!("ch_tree_{}_{}", depth, idx))
                .default_open(true)
                .show(ui, |ui| {
                    Self::draw_chapter_tree(ui, &node.children, depth + 1, open_left, project_root);
                });
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
        toggle_path: &mut Option<PathBuf>,
        selected_path: &Option<PathBuf>,
        select_path: &mut Option<PathBuf>,
        rename_path: &mut Option<PathBuf>,
    ) {
        let indent = depth as f32 * 12.0;
        ui.horizontal(|ui| {
            ui.add_space(indent);
            if node.is_dir {
                let icon = if node.expanded { "▼" } else { "▶" };
                let resp = ui.selectable_label(
                    false,
                    format!("{icon} 📁 {}", node.name),
                );
                if resp.clicked() {
                    *toggle_path = Some(node.path.clone());
                }
                resp.on_hover_text(if node.expanded { "点击折叠" } else { "点击展开" });
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
                let is_selected = selected_path.as_deref() == Some(node.path.as_path());
                let resp = ui.selectable_label(is_selected, format!("{icon} {}", node.name));
                resp.context_menu(|ui| {
                    if ui.button("打开 / 在左侧打开").clicked() {
                        *open_left = Some(node.path.clone());
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("重命名 (F2)").clicked() {
                        *rename_path = Some(node.path.clone());
                        ui.close_menu();
                    }
                });
                if resp.clicked() {
                    *select_path = Some(node.path.clone());
                }
                if resp.double_clicked() {
                    // All files open in the left (main) editor
                    *open_left = Some(node.path.clone());
                }
                resp.on_hover_text("单击选中  双击打开  右键菜单");
            }
        });

        if node.is_dir && node.expanded {
            for child in &node.children {
                Self::draw_tree_node(ui, child, depth + 1, open_left, open_right, new_in,
                    toggle_path, selected_path, select_path, rename_path);
            }
        }
    }

    /// Toggle the `expanded` flag of the tree node matching `path`.
    pub(in crate::app) fn toggle_expand_in_tree(nodes: &mut Vec<FileNode>, path: &std::path::Path) -> bool {
        for node in nodes.iter_mut() {
            if node.path == path {
                node.expanded = !node.expanded;
                return true;
            }
            if node.is_dir && Self::toggle_expand_in_tree(&mut node.children, path) {
                return true;
            }
        }
        false
    }

    pub(in crate::app) fn draw_editors(&mut self, ctx: &Context) {
        let mut do_extract_struct = false;
        let mut do_sync_folders   = false;
        let mut switch_to_obj_idx: Option<usize> = None;

        // ── Right sidebar: world-object reference cards ───────────────────────
        // Snapshot non-mutable data before any borrow of `self`.
        let objects_snapshot: Vec<_> = self.world_objects.iter().enumerate()
            .map(|(i, o)| (i, o.icon(), o.name.clone(), o.kind.label(), o.description.clone(), o.links.len()))
            .collect();
        let selected_obj = self.selected_obj_idx;

        egui::SidePanel::right("obj_ref_sidebar")
            .resizable(true)
            .default_width(190.0)
            .min_width(120.0)
            .max_width(300.0)
            .show(ctx, |ui| {
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.heading("对象参考");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("⊞").on_hover_text("在「世界对象」面板管理").clicked() {
                            switch_to_obj_idx = selected_obj; // keep selection, just switch panel
                        }
                    });
                });
                ui.separator();

                if objects_snapshot.is_empty() {
                    ui.label(
                        RichText::new("暂无对象\n请在「世界对象」面板添加")
                            .small().color(Color32::GRAY),
                    );
                } else {
                    egui::ScrollArea::vertical().id_salt("obj_ref_scroll").show(ui, |ui| {
                        for (i, icon, name, kind, desc, link_count) in &objects_snapshot {
                            let is_sel = selected_obj == Some(*i);
                            let bg = if is_sel {
                                Color32::from_rgb(0, 70, 130)
                            } else {
                                Color32::from_gray(36)
                            };
                            let card = egui::Frame::none()
                                .fill(bg)
                                .rounding(5.0)
                                .inner_margin(egui::Margin::symmetric(7.0, 5.0))
                                .show(ui, |ui| {
                                    ui.set_min_width(ui.available_width());
                                    ui.horizontal(|ui| {
                                        ui.label(RichText::new(*icon).size(16.0));
                                        ui.vertical(|ui| {
                                            ui.label(RichText::new(name).strong().size(12.0));
                                            ui.label(
                                                RichText::new(*kind)
                                                    .size(10.0)
                                                    .color(Color32::from_gray(160)),
                                            );
                                        });
                                    });
                                    if !desc.is_empty() {
                                        let mut chars = desc.chars();
                                        let preview: String = (&mut chars).take(24).collect();
                                        let suffix = if chars.next().is_some() { "…" } else { "" };
                                        ui.label(
                                            RichText::new(format!("{preview}{suffix}"))
                                                .size(10.0)
                                                .color(Color32::from_gray(140)),
                                        );
                                    }
                                    if *link_count > 0 {
                                        ui.label(
                                            RichText::new(format!("🔗{link_count}"))
                                                .size(10.0)
                                                .color(Color32::from_rgb(100, 170, 230)),
                                        );
                                    }
                                })
                                .response
                                .interact(egui::Sense::click());

                            if card.clicked() {
                                switch_to_obj_idx = Some(*i);
                            }
                            card.on_hover_text(if desc.is_empty() {
                                format!("{name} ({kind}) — 点击在对象面板中查看")
                            } else {
                                format!("{name}: {desc}")
                            });
                            ui.add_space(3.0);
                        }
                    });
                }
            });

        // ── Central panel: single full-width Markdown editor ──────────────────
        egui::CentralPanel::default().show(ctx, |ui| {
            // Toolbar row above editor
            ui.horizontal(|ui| {
                ui.label(RichText::new("编辑区").strong());
                ui.separator();
                if ui.button("提取结构")
                    .on_hover_text("从 Markdown 标题 (#/##/###) 提取章节结构到「章节结构」面板")
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
                        RichText::new("Ctrl+B 粗体  Ctrl+I 斜体  Ctrl+Z 撤销  Ctrl+S 保存  Ctrl+滚轮 缩放")
                            .small()
                            .color(Color32::from_gray(120)),
                    );
                });
            });
            ui.separator();

            let available = ui.available_size();

            // File header bar
            let file_title = self.left_file.as_ref()
                .map(|f| f.title())
                .unwrap_or_else(|| "文本编辑区".to_owned());

            ui.horizontal(|ui| {
                ui.label(RichText::new(&file_title).strong());
                // Word count
                if let Some(f) = &self.left_file {
                    if f.is_markdown() {
                        let char_count: usize = f.content.chars()
                            .filter(|c| !c.is_whitespace()).count();
                        ui.label(
                            RichText::new(format!("字数: {char_count}"))
                                .small().color(Color32::from_gray(150)),
                        );
                    }
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.small_button("💾").on_hover_text("保存 (Ctrl+S)").clicked() {
                        self.save_left();
                    }
                    let is_md = self.left_file.as_ref().map(|f| f.is_markdown()).unwrap_or(false);
                    if is_md {
                        let toggle_label = if self.left_preview_mode { "✏ 编辑" } else { "👁 预览" };
                        let hover = if self.left_preview_mode { "切换到编辑模式" } else { "切换到预览模式 (Ctrl+P)" };
                        if ui.small_button(toggle_label).on_hover_text(hover).clicked() {
                            self.left_preview_mode = !self.left_preview_mode;
                        }
                    }
                });
            });
            ui.separator();

            let height = available.y - 80.0;
            let is_preview = self.left_preview_mode
                && self.left_file.as_ref().map(|f| f.is_markdown()).unwrap_or(false);

            if is_preview {
                if let Some(f) = &self.left_file {
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
                let prev = f.content.clone();
                egui::ScrollArea::both()
                    .id_salt("left_editor")
                    .show(ui, |ui| {
                        let font_id = egui::FontId::monospace(self.md_settings.editor_font_size);
                        let editor = egui::TextEdit::multiline(&mut f.content)
                            .id(egui::Id::new("left_editor_main"))
                            .desired_width(f32::INFINITY)
                            .desired_rows(30)
                            .min_size(egui::vec2(0.0, height))
                            .font(font_id)
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
                    ui.label(
                        RichText::new("从左侧文件树双击打开文件，\n或通过菜单「文件 → 打开项目文件夹」")
                            .color(Color32::GRAY),
                    );
                });
            }
        });

        // Apply deferred actions
        if let Some(idx) = switch_to_obj_idx {
            self.selected_obj_idx = Some(idx);
            self.active_panel = Panel::Objects;
        }
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
