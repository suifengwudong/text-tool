use egui::{Context, RichText, Color32};
use super::super::{TextToolApp, Character, Chapter, ChapterTag, Relationship, RelationKind};

impl TextToolApp {
    // ── Panel: Characters & Chapters ─────────────────────────────────────────

    pub(in crate::app) fn draw_characters_panel(&mut self, ctx: &Context) {
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
}
