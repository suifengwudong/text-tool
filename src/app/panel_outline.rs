use egui::{Context, RichText, Color32};
use super::{TextToolApp, OutlineEntry, parse_outline};

impl TextToolApp {
    // ── Panel: Outline & Foreshadowing ────────────────────────────────────────

    pub(super) fn draw_outline_panel(&mut self, ctx: &Context) {
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
                            self.foreshadows.push(super::Foreshadow::new(&name));
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

    pub(super) fn draw_outline_entries(ui: &mut egui::Ui, entries: &[OutlineEntry], depth: usize) {
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
}
