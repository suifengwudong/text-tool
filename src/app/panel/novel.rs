use std::path::PathBuf;
use egui::{Context, RichText, Color32};
use super::super::{TextToolApp, FileNode, rfd_pick_folder};
use super::markdown::render_markdown;

impl TextToolApp {
    // ── Novel panel: file tree + dual editors ─────────────────────────────────

    pub(in crate::app) fn draw_file_tree(&mut self, ctx: &Context) {
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
}
