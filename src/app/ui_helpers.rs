use egui::{Context, RichText, Color32, Key};
use super::{TextToolApp, Panel, rfd_pick_folder, rfd_save_file};

/// Minimum Ctrl+scroll delta (in points) required to adjust the font size by one step.
const CTRL_SCROLL_THRESHOLD: f32 = 1.0;

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
                    if ui.button("保存  Ctrl+S").clicked() {
                        self.save_left();
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
                    if ui.button("导出当前文件…").clicked() {
                        self.export_left();
                        ui.close_menu();
                    }
                });

                ui.menu_button("视图", |ui| {
                    for panel in [Panel::Novel, Panel::Objects, Panel::Structure, Panel::Llm] {
                        let label = format!("{} {}", panel.icon(), panel.label());
                        let selected = self.active_panel == panel;
                        if ui.selectable_label(selected, label).clicked() {
                            self.active_panel = panel;
                            ui.close_menu();
                        }
                    }
                });

                ui.menu_button("工具", |ui| {
                    if ui.button("从 Markdown 标题提取章节结构").clicked() {
                        self.extract_structure_from_left();
                        ui.close_menu();
                    }
                    if ui.button("从文件夹结构生成章节").clicked() {
                        self.sync_struct_from_folders();
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("保存世界对象到 JSON").clicked() {
                        self.sync_world_objects_to_json();
                        ui.close_menu();
                    }
                    if ui.button("保存章节结构到 JSON").clicked() {
                        self.sync_struct_to_json();
                        ui.close_menu();
                    }
                    if ui.button("保存伏笔到 MD").clicked() {
                        self.sync_foreshadows_to_md();
                        ui.close_menu();
                    }
                    if ui.button("保存里程碑到 JSON").clicked() {
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
                    if ui.button("⚙ 编辑器设置…").clicked() {
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
            .exact_width(52.0)
            .show(ctx, |ui| {
                // Toolbar background tint
                let rect = ui.available_rect_before_wrap();
                ui.painter().rect_filled(rect, 0.0, Color32::from_rgb(30, 30, 35));

                ui.vertical_centered(|ui| {
                    ui.add_space(8.0);
                    for panel in [Panel::Novel, Panel::Objects, Panel::Structure, Panel::Llm] {
                        let selected = self.active_panel == panel;
                        // Left accent bar for selected item
                        if selected {
                            let r = ui.next_widget_position();
                            let accent_rect = egui::Rect::from_min_size(
                                r,
                                egui::vec2(3.0, 42.0),
                            );
                            ui.painter().rect_filled(
                                accent_rect, 0.0, Color32::from_rgb(0, 150, 220),
                            );
                        }
                        let text_color = if selected {
                            Color32::WHITE
                        } else {
                            Color32::from_gray(160)
                        };
                        let btn = egui::Button::new(
                            RichText::new(panel.icon()).size(20.0).color(text_color)
                        )
                        .fill(if selected {
                            Color32::from_rgb(45, 45, 55)
                        } else {
                            Color32::TRANSPARENT
                        })
                        .stroke(if selected {
                            egui::Stroke::new(1.0, Color32::from_rgb(0, 150, 220))
                        } else {
                            egui::Stroke::NONE
                        })
                        .rounding(4.0)
                        .frame(true);

                        if ui.add_sized([44.0, 42.0], btn)
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
                const ERROR_WORDS:   &[&str] = &["失败", "错误"];
                const SUCCESS_WORDS: &[&str] = &["完成", "已保存", "已同步", "已加载", "废稿"];

                let status_color = if ERROR_WORDS.iter().any(|w| self.status.contains(w)) {
                    Color32::from_rgb(220, 80, 80)
                } else if SUCCESS_WORDS.iter().any(|w| self.status.contains(w)) {
                    Color32::from_rgb(100, 200, 120)
                } else {
                    Color32::from_gray(180)
                };
                ui.label(RichText::new(&self.status).color(status_color));

                // Auto-save indicator
                if !self.last_auto_save_label.is_empty() {
                    ui.separator();
                    ui.label(
                        RichText::new(format!("💾 自动保存 {}", self.last_auto_save_label))
                            .small()
                            .color(Color32::from_gray(130)),
                    );
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        RichText::new("Ctrl+S 保存  Ctrl+Z 撤销  Ctrl+滚轮 缩放字体  F2 重命名")
                            .color(Color32::from_gray(120))
                            .small(),
                    );
                });
            });
        });
    }

    /// Draw the delete-to-trash confirmation dialog.
    pub(super) fn draw_delete_confirm_dialog(&mut self, ctx: &Context) {
        let path = match self.delete_confirm_path.clone() {
            Some(p) => p,
            None    => return,
        };
        let file_name = path.file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.to_string_lossy().into_owned());

        let mut confirmed = false;
        let mut cancelled = false;

        egui::Window::new("确认删除")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label(format!("将「{file_name}」移入废稿文件夹？"));
                ui.label(
                    RichText::new("此操作可在废稿文件夹中恢复。")
                        .small().color(Color32::from_gray(150)),
                );
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button("移入废稿").clicked() { confirmed = true; }
                    if ui.button("取消").clicked()    { cancelled = true; }
                });
                if ctx.input(|i| i.key_pressed(Key::Escape)) { cancelled = true; }
                if ctx.input(|i| i.key_pressed(Key::Enter))  { confirmed = true; }
            });

        if confirmed {
            self.delete_confirm_path = None;
            let path_to_delete = path.clone();
            self.move_to_trash(&path_to_delete);
        } else if cancelled {
            self.delete_confirm_path = None;
        }
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
            let ctrl_scroll = if ctrl { i.smooth_scroll_delta.y } else { 0.0 };
            (
                ctrl && !shift && i.key_pressed(Key::S),           // Ctrl+S
                ctrl && shift && i.key_pressed(Key::S),            // Ctrl+Shift+S (save json/backup)
                ctrl && !shift && i.key_pressed(Key::Z),           // Ctrl+Z
                ctrl && shift && i.key_pressed(Key::F),            // Ctrl+Shift+F search
                ctrl && !shift && i.key_pressed(Key::B),           // Ctrl+B bold
                ctrl && !shift && i.key_pressed(Key::I),           // Ctrl+I italic
                !ctrl && !shift && i.key_pressed(Key::Tab),        // Tab indent
                ctrl && i.key_pressed(Key::Equals),                 // Ctrl++/= zoom in
                ctrl && i.key_pressed(Key::Minus),                  // Ctrl+- zoom out
                ctrl && i.key_pressed(Key::Num0),                   // Ctrl+0 reset zoom
                ctrl_scroll,                                        // Ctrl+scroll
                !ctrl && !shift && i.key_pressed(Key::F2),         // F2 rename
                ctrl && !shift && i.key_pressed(Key::P),           // Ctrl+P preview toggle
            )
        });
        if input.0 {
            self.save_left();
            if self.md_settings.auto_extract_structure {
                self.extract_structure_from_left();
            }
        }
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
            self.insert_markdown_wrap(ctx, input.4);
        }
        // Tab: insert configurable number of spaces at cursor in left editor
        if input.6 && self.last_focused_left {
            self.insert_tab_spaces(ctx);
        }
        // Ctrl++ / Ctrl+scroll up: increase font size (editor or preview)
        if input.7 {
            if self.left_preview_mode {
                self.md_settings.preview_font_size = (self.md_settings.preview_font_size + 1.0).min(36.0);
            } else {
                self.md_settings.editor_font_size = (self.md_settings.editor_font_size + 1.0).min(36.0);
            }
            self.save_config();
        }
        // Ctrl+- / Ctrl+scroll down: decrease font size
        if input.8 {
            if self.left_preview_mode {
                self.md_settings.preview_font_size = (self.md_settings.preview_font_size - 1.0).max(8.0);
            } else {
                self.md_settings.editor_font_size = (self.md_settings.editor_font_size - 1.0).max(8.0);
            }
            self.save_config();
        }
        // Ctrl+0: reset font size
        if input.9 {
            let def = crate::app::MarkdownSettings::default();
            if self.left_preview_mode {
                self.md_settings.preview_font_size = def.preview_font_size;
            } else {
                self.md_settings.editor_font_size = def.editor_font_size;
            }
            self.save_config();
        }
        // Ctrl+scroll: adjust font size (editor or preview based on current mode)
        if input.10.abs() > CTRL_SCROLL_THRESHOLD {
            let delta = if input.10 > 0.0 { 1.0_f32 } else { -1.0_f32 };
            if self.left_preview_mode {
                self.md_settings.preview_font_size = (self.md_settings.preview_font_size + delta)
                    .clamp(8.0, 36.0);
            } else {
                self.md_settings.editor_font_size = (self.md_settings.editor_font_size + delta)
                    .clamp(8.0, 36.0);
            }
            self.save_config();
        }
        // F2: rename selected file in navigation
        if input.11 {
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
        // Ctrl+P: toggle preview mode (only for markdown files)
        if input.12 {
            let is_md = self.left_file.as_ref().map(|f| f.is_markdown()).unwrap_or(false);
            if is_md {
                self.left_preview_mode = !self.left_preview_mode;
            }
        }
    }

    /// Insert `**...**` (bold) or `*...*` (italic) around the current selection
    /// in the left editor, or insert a template if nothing is selected.
    fn insert_markdown_wrap(&mut self, ctx: &Context, is_bold: bool) {
        let marker = if is_bold { "**" } else { "*" };
        let te_id = egui::Id::new("left_editor_main");
        if let Some(mut state) = egui::text_edit::TextEditState::load(ctx, te_id) {
            if let Some(range) = state.cursor.char_range() {
                let from = range.primary.index.min(range.secondary.index);
                let to   = range.primary.index.max(range.secondary.index);
                if let Some(f) = &mut self.left_file {
                    let chars: Vec<char> = f.content.chars().collect();
                    let selected: String = chars[from..to].iter().collect();
                    let replacement = if from == to {
                        let tmpl = if is_bold { "**粗体**" } else { "*斜体*" };
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
                    let new_cursor = egui::text::CCursorRange::one(
                        egui::text::CCursor::new(new_end));
                    state.cursor.set_char_range(Some(new_cursor));
                    egui::text_edit::TextEditState::store(state, ctx, te_id);
                }
            }
        }
    }

    /// Insert spaces (matching `md_settings.tab_size`) at the cursor in the left editor.
    fn insert_tab_spaces(&mut self, ctx: &Context) {
        let spaces: String = " ".repeat(self.md_settings.tab_size as usize);
        let te_id = egui::Id::new("left_editor_main");
        if let Some(mut state) = egui::text_edit::TextEditState::load(ctx, te_id) {
            if let Some(range) = state.cursor.char_range() {
                let from = range.primary.index.min(range.secondary.index);
                let to   = range.primary.index.max(range.secondary.index);
                if let Some(f) = &mut self.left_file {
                    let chars: Vec<char> = f.content.chars().collect();
                    let mut new_content = String::new();
                    new_content.extend(chars[..from].iter());
                    new_content.push_str(&spaces);
                    new_content.extend(chars[to..].iter());
                    let new_pos = from + spaces.chars().count();
                    f.content = new_content;
                    f.modified = true;
                    let new_cursor = egui::text::CCursorRange::one(
                        egui::text::CCursor::new(new_pos));
                    state.cursor.set_char_range(Some(new_cursor));
                    egui::text_edit::TextEditState::store(state, ctx, te_id);
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

    /// Draw the floating editor settings window.
    pub(super) fn draw_settings_window(&mut self, ctx: &Context) {
        if !self.show_settings_window {
            return;
        }

        let mut open = self.show_settings_window;
        egui::Window::new("⚙ 编辑器设置")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .min_width(320.0)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.add_space(4.0);

                // ── Editor ────────────────────────────────────────────────────
                ui.heading("编辑器");
                ui.add_space(2.0);
                ui.horizontal(|ui| {
                    ui.label("编辑器字体大小:");
                    let prev_esz = self.md_settings.editor_font_size;
                    ui.add(
                        egui::Slider::new(&mut self.md_settings.editor_font_size, 8.0..=36.0)
                            .step_by(1.0)
                            .suffix(" px"),
                    );
                    if (self.md_settings.editor_font_size - prev_esz).abs() > f32::EPSILON {
                        self.save_config();
                    }
                });
                ui.horizontal(|ui| {
                    ui.label("Tab 缩进空格数:");
                    let prev_tab = self.md_settings.tab_size;
                    let mut tab_size = self.md_settings.tab_size as u32;
                    ui.add(egui::Slider::new(&mut tab_size, 1..=8).step_by(1.0));
                    self.md_settings.tab_size = tab_size as u8;
                    if self.md_settings.tab_size != prev_tab { self.save_config(); }
                });
                ui.add_space(2.0);
                let prev_ae = self.md_settings.auto_extract_structure;
                ui.checkbox(
                    &mut self.md_settings.auto_extract_structure,
                    "Ctrl+S 保存时自动从 Markdown 标题提取章节结构",
                );
                if self.md_settings.auto_extract_structure != prev_ae { self.save_config(); }
                ui.label(
                    RichText::new("Ctrl+滚轮 / Ctrl+= / Ctrl+- 实时调整字体大小  Ctrl+P 切换预览")
                        .small().color(Color32::from_gray(140)),
                );

                ui.add_space(6.0);
                ui.separator();

                // ── Markdown preview ───────────────────────────────────────────
                ui.heading("Markdown 预览");
                ui.add_space(2.0);
                ui.horizontal(|ui| {
                    ui.label("预览字体大小:");
                    let prev_size = self.md_settings.preview_font_size;
                    ui.add(
                        egui::Slider::new(&mut self.md_settings.preview_font_size, 8.0..=36.0)
                            .step_by(1.0)
                            .suffix(" px"),
                    );
                    if (self.md_settings.preview_font_size - prev_size).abs() > f32::EPSILON {
                        self.save_config();
                    }
                });
                ui.add_space(2.0);
                let prev = self.md_settings.default_to_preview;
                ui.checkbox(
                    &mut self.md_settings.default_to_preview,
                    "打开 Markdown 文件时默认切换到预览模式",
                );
                if self.md_settings.default_to_preview != prev { self.save_config(); }

                ui.add_space(6.0);
                ui.separator();

                // ── 自动保存 ─────────────────────────────────────────────────────
                ui.heading("自动保存");
                ui.add_space(2.0);
                ui.horizontal(|ui| {
                    ui.label("自动保存间隔:");
                    let prev_int = self.md_settings.auto_save_interval_secs;
                    let mut interval = self.md_settings.auto_save_interval_secs as u32;
                    ui.add(
                        egui::Slider::new(&mut interval, 0..=300)
                            .step_by(10.0)
                            .suffix(" 秒"),
                    );
                    self.md_settings.auto_save_interval_secs = interval;
                    if self.md_settings.auto_save_interval_secs != prev_int {
                        self.save_config();
                    }
                });
                ui.label(
                    RichText::new("0 = 关闭自动保存；状态栏显示上次自动保存时间")
                        .small().color(Color32::from_gray(140)),
                );

                ui.add_space(6.0);
                ui.separator();

                // ── 主题 ─────────────────────────────────────────────────────────
                ui.heading("界面主题");
                ui.add_space(2.0);
                ui.horizontal(|ui| {
                    let prev_theme = self.theme;
                    for &t in crate::app::AppTheme::all() {
                        ui.radio_value(&mut self.theme, t, t.label());
                    }
                    if self.theme != prev_theme { self.save_config(); }
                });

                ui.add_space(6.0);
                ui.separator();

                // ── File tree ──────────────────────────────────────────────────
                ui.heading("文件树");
                ui.add_space(2.0);
                let prev_hide = self.md_settings.hide_json;
                ui.checkbox(
                    &mut self.md_settings.hide_json,
                    "隐藏 .json 文件（推荐：JSON 为内部数据，无需手动编辑）",
                );
                if self.md_settings.hide_json != prev_hide {
                    self.refresh_tree();
                    self.save_config();
                }
                let prev_files_tab = self.md_settings.show_files_tab;
                ui.checkbox(
                    &mut self.md_settings.show_files_tab,
                    "在导航中显示「文件」标签页（默认关闭，使用章节树导航）",
                );
                if self.md_settings.show_files_tab != prev_files_tab {
                    self.save_config();
                }

                ui.add_space(6.0);
                ui.separator();

                // ── Data sync ─────────────────────────────────────────────────
                ui.heading("数据同步");
                ui.add_space(2.0);
                let prev_al = self.auto_load_from_files;
                ui.checkbox(
                    &mut self.auto_load_from_files,
                    "打开项目时自动从文件反向同步数据",
                );
                if self.auto_load_from_files != prev_al { self.save_config(); }

                ui.add_space(8.0);
                ui.separator();
                ui.add_space(4.0);

                if ui.button("重置默认值").clicked() {
                    self.md_settings = crate::app::MarkdownSettings::default();
                    self.refresh_tree();
                    self.save_config();
                }
            });

        // Detect window close via X button and save config
        if !open && self.show_settings_window {
            self.save_config();
        }
        self.show_settings_window = open;
    }

    /// Draw the rename file dialog (triggered by F2 or context menu).
    pub(super) fn draw_rename_dialog(&mut self, ctx: &Context) {
        let mut do_rename: Option<(std::path::PathBuf, String)> = None;
        let mut close = false;

        if let Some(dlg) = &mut self.rename_dialog {
            egui::Window::new("重命名文件")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label(format!("重命名: {}", dlg.path.file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default()));
                    let resp = ui.text_edit_singleline(&mut dlg.new_name);
                    resp.request_focus();
                    if resp.lost_focus() && ctx.input(|i| i.key_pressed(Key::Escape)) {
                        close = true;
                    }
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.button("确认").clicked()
                            || (resp.lost_focus() && ctx.input(|i| i.key_pressed(Key::Enter)))
                        {
                            let new_name = dlg.new_name.trim().to_owned();
                            if !new_name.is_empty() {
                                do_rename = Some((dlg.path.clone(), new_name));
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
            self.rename_dialog = None;
        }
        if let Some((path, new_name)) = do_rename {
            self.rename_file(&path, &new_name);
        }
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
