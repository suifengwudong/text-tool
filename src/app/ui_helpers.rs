use egui::{Context, RichText, Color32, Key};
use super::{TextToolApp, Panel, rfd_pick_folder, rfd_save_file};

impl TextToolApp {
    // ── UI helpers ────────────────────────────────────────────────────────────

    pub(super) fn draw_menu_bar(&mut self, ctx: &Context) {
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
                    if ui.button("导出章节合集…").clicked() {
                        self.export_chapters_merged();
                        ui.close_menu();
                    }
                    if ui.button("备份项目到文件夹…").clicked() {
                        self.backup_project();
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
                    for panel in [Panel::Novel, Panel::Objects, Panel::Structure, Panel::LLM] {
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
                    ui.separator();
                    if ui.button("同步世界对象到 JSON").clicked() {
                        self.sync_world_objects_to_json();
                        ui.close_menu();
                    }
                    if ui.button("同步章节结构到 JSON").clicked() {
                        self.sync_struct_to_json();
                        ui.close_menu();
                    }
                    if ui.button("同步伏笔到 MD").clicked() {
                        self.sync_foreshadows_to_md();
                        ui.close_menu();
                    }
                    if ui.button("同步里程碑到 JSON").clicked() {
                        self.sync_milestones_to_json();
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("从 JSON 加载世界对象").clicked() {
                        self.load_world_objects_from_json();
                        ui.close_menu();
                    }
                    if ui.button("从 JSON 加载章节结构").clicked() {
                        self.load_struct_from_json();
                        ui.close_menu();
                    }
                    if ui.button("从 MD 加载伏笔").clicked() {
                        self.load_foreshadows_from_md();
                        ui.close_menu();
                    }
                    if ui.button("从 JSON 加载里程碑").clicked() {
                        self.load_milestones_from_json();
                        ui.close_menu();
                    }
                });

                ui.menu_button("设置", |ui| {
                    if ui.button("⚙ Markdown 预览设置…").clicked() {
                        self.show_settings_window = true;
                        ui.close_menu();
                    }
                });
            });
        });
    }

    pub(super) fn draw_toolbar(&mut self, ctx: &Context) {
        egui::SidePanel::left("toolbar")
            .resizable(false)
            .exact_width(48.0)
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(8.0);
                    for panel in [Panel::Novel, Panel::Objects, Panel::Structure, Panel::LLM] {
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

    pub(super) fn draw_status_bar(&self, ctx: &Context) {
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

    pub(super) fn draw_new_file_dialog(&mut self, ctx: &Context) {
        let mut create_path: Option<std::path::PathBuf> = None;
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

    pub(super) fn handle_keyboard(&mut self, ctx: &Context) {
        let input = ctx.input(|i| {
            let ctrl = i.modifiers.ctrl || i.modifiers.command;
            let shift = i.modifiers.shift;
            (
                ctrl && !shift && i.key_pressed(Key::S),           // Ctrl+S
                ctrl && shift && i.key_pressed(Key::S),            // Ctrl+Shift+S
                ctrl && !shift && i.key_pressed(Key::Z),           // Ctrl+Z
                ctrl && shift && i.key_pressed(Key::F),            // Ctrl+Shift+F
                ctrl && !shift && i.key_pressed(Key::B),           // Ctrl+B
                ctrl && !shift && i.key_pressed(Key::I),           // Ctrl+I
            )
        });
        if input.0 { self.save_left(); }
        if input.1 { self.save_right(); }
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
        if input.3 {
            self.show_search = !self.show_search;
        }
        // Ctrl+B / Ctrl+I: bold / italic insertion in the left editor
        if (input.4 || input.5) && self.last_focused_left {
            let marker = if input.4 { "**" } else { "*" };
            let te_id = egui::Id::new("left_editor_main");
            if let Some(mut state) = egui::text_edit::TextEditState::load(ctx, te_id) {
                if let Some(range) = state.cursor.char_range() {
                    let from = range.primary.index.min(range.secondary.index);
                    let to   = range.primary.index.max(range.secondary.index);
                    if let Some(f) = &mut self.left_file {
                        let chars: Vec<char> = f.content.chars().collect();
                        let selected: String = chars[from..to].iter().collect();
                        let replacement = if from == to {
                            let tmpl = if input.4 { "**粗体**" } else { "*斜体*" };
                            tmpl.to_owned()
                        } else {
                            format!("{}{}{}", marker, selected, marker)
                        };
                        let new_end = from + replacement.chars().count();
                        let mut new_content = String::new();
                        new_content.extend(chars[..from].iter());
                        new_content.push_str(&replacement);
                        new_content.extend(chars[to..].iter());
                        f.content = new_content;
                        f.modified = true;
                        // Move cursor to end of inserted text
                        let new_cursor = egui::text::CCursorRange::one(
                            egui::text::CCursor::new(new_end));
                        state.cursor.set_char_range(Some(new_cursor));
                        egui::text_edit::TextEditState::store(state, ctx, te_id);
                    }
                }
            }
        }
    }

    pub(super) fn export_left(&self) {
        if let Some(f) = &self.left_file {
            if let Some(dest) = rfd_save_file(&f.path) {
                if let Err(e) = std::fs::write(&dest, &f.content) {
                    eprintln!("导出失败: {e}");
                }
            }
        }
    }

    pub(super) fn export_right(&self) {
        if let Some(f) = &self.right_file {
            if let Some(dest) = rfd_save_file(&f.path) {
                if let Err(e) = std::fs::write(&dest, &f.content) {
                    eprintln!("导出失败: {e}");
                }
            }
        }
    }

    /// Draw the floating Markdown preview settings window.
    pub(super) fn draw_settings_window(&mut self, ctx: &Context) {
        if !self.show_settings_window {
            return;
        }

        let mut open = self.show_settings_window;
        egui::Window::new("⚙ Markdown 预览设置")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .min_width(280.0)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.add_space(4.0);

                ui.horizontal(|ui| {
                    ui.label("预览字体大小:");
                    ui.add(
                        egui::Slider::new(&mut self.md_settings.preview_font_size, 10.0..=26.0)
                            .step_by(1.0)
                            .suffix(" px"),
                    );
                });

                ui.add_space(4.0);
                ui.checkbox(
                    &mut self.md_settings.default_to_preview,
                    "打开 Markdown 文件时默认切换到预览模式",
                );

                ui.add_space(4.0);
                ui.separator();
                ui.add_space(4.0);

                ui.checkbox(
                    &mut self.auto_load_from_files,
                    "打开项目时自动从文件反向同步数据",
                );

                ui.add_space(8.0);
                ui.separator();
                ui.add_space(4.0);

                ui.horizontal(|ui| {
                    if ui.button("重置默认值").clicked() {
                        self.md_settings = crate::app::MarkdownSettings::default();
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("关闭").clicked() {
                            self.show_settings_window = false;
                            self.save_config();
                        }
                    });
                });
            });

        self.show_settings_window = open;
    }

    /// Draw the floating full-text search window (Ctrl+Shift+F).
    pub(super) fn draw_search_window(&mut self, ctx: &Context) {
        if !self.show_search { return; }

        let mut open = self.show_search;
        let mut run_search = false;
        let mut open_file: Option<std::path::PathBuf> = None;

        egui::Window::new("🔍 全文搜索")
            .open(&mut open)
            .resizable(true)
            .default_size([500.0, 360.0])
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("搜索:");
                    let resp = ui.add(
                        egui::TextEdit::singleline(&mut self.search_query)
                            .desired_width(300.0)
                            .hint_text("输入关键词…"),
                    );
                    if resp.lost_focus() && ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
                        run_search = true;
                    }
                    if ui.button("搜索").clicked() {
                        run_search = true;
                    }
                });
                ui.separator();

                let results_snapshot = self.search_results.clone();
                if results_snapshot.is_empty() {
                    ui.label(RichText::new("暂无结果").color(Color32::GRAY));
                } else {
                    ui.label(RichText::new(format!("共 {} 处匹配", results_snapshot.len())).small());
                    egui::ScrollArea::vertical().id_salt("search_results_scroll").show(ui, |ui| {
                        for result in &results_snapshot {
                            let fname = result.file_path.file_name()
                                .unwrap_or_default().to_string_lossy();
                            let label = format!("{}:{} — {}",
                                fname, result.line_no, result.line.trim());
                            let resp = ui.selectable_label(false,
                                RichText::new(&label).monospace().small())
                                .on_hover_text(result.file_path.display().to_string());
                            if resp.double_clicked() {
                                open_file = Some(result.file_path.clone());
                            }
                        }
                    });
                }
            });

        self.show_search = open;
        if run_search { self.run_search(); }
        if let Some(path) = open_file {
            self.open_file_in_pane(&path, true);
        }
    }
}
