use egui::{Context, RichText, Color32};
use super::super::{
    TextToolApp, WorldObject, ObjectKind, ObjectLink, LinkTarget, RelationKind,
    StructNode, ObjectViewMode,
};

impl TextToolApp {
    // ── Panel: World Objects ──────────────────────────────────────────────────
    //
    // Left side-panel: object list with kind filter
    // Central panel:   selected object editor + links (object↔object and object↔node)

    pub(in crate::app) fn draw_objects_panel(&mut self, ctx: &Context) {
        // ── Left: object list ──────────────────────────────────────────────────
        let mut open_obj: Option<usize> = None;
        let mut remove_obj: Option<usize> = None;

        egui::SidePanel::left("obj_list")
            .resizable(true)
            .default_width(200.0)
            .min_width(130.0)
            .show(ctx, |ui| {
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.heading("世界对象");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        // View mode toggle
                        let is_card = self.obj_view_mode == ObjectViewMode::Card;
                        if ui.selectable_label(is_card, "🃏 卡片")
                            .on_hover_text("切换到卡片视图").clicked()
                        {
                            self.obj_view_mode = ObjectViewMode::Card;
                        }
                        if ui.selectable_label(!is_card, "☰ 列表")
                            .on_hover_text("切换到列表视图").clicked()
                        {
                            self.obj_view_mode = ObjectViewMode::List;
                        }
                    });
                });
                // JSON sync buttons
                ui.horizontal(|ui| {
                    if ui.small_button("⬆ 保存JSON").on_hover_text("保存世界对象到 Design/世界对象.json").clicked() {
                        self.sync_world_objects_to_json();
                    }
                    if ui.small_button("⬇ 加载JSON").on_hover_text("从 Design/世界对象.json 加载世界对象").clicked() {
                        self.load_world_objects_from_json();
                    }
                });
                ui.separator();

                // Kind filter chips
                ui.horizontal_wrapped(|ui| {
                    let all_sel = self.obj_kind_filter.is_none();
                    if ui.selectable_label(all_sel, "全部").clicked() {
                        self.obj_kind_filter = None;
                    }
                    for k in ObjectKind::all() {
                        let sel = self.obj_kind_filter.as_ref() == Some(k);
                        if ui.selectable_label(sel,
                            format!("{} {}", k.icon(), k.label())).clicked()
                        {
                            if sel {
                                self.obj_kind_filter = None;
                            } else {
                                self.obj_kind_filter = Some(k.clone());
                            }
                        }
                    }
                });
                ui.separator();

                egui::ScrollArea::vertical().id_salt("obj_list_scroll").show(ui, |ui| {
                    if self.obj_view_mode == ObjectViewMode::List {
                        let mut pending_move: Option<(usize, usize)> = None;
                        // Index loop required: we pass `i` as the DnD payload and need
                        // to detect drops by index after the draw pass completes.
                        for i in 0..self.world_objects.len() {
                            let obj = &self.world_objects[i];
                            // Apply kind filter
                            if let Some(ref filter) = self.obj_kind_filter {
                                if &obj.kind != filter { continue; }
                            }
                            let selected = self.selected_obj_idx == Some(i);
                            let label = format!("{} {}", obj.icon(), obj.name);
                            let item_id = egui::Id::new(("wo_drag", i));
                            let ir = ui.dnd_drag_source(item_id, i, |ui| {
                                ui.selectable_label(selected, &label)
                            });
                            // Detect drop onto this item
                            if let Some(payload) = ir.response.dnd_release_payload::<usize>() {
                                let from = *payload;
                                if from != i { pending_move = Some((from, i)); }
                            }
                            ir.response.context_menu(|ui| {
                                if ui.button("删除").clicked() {
                                    remove_obj = Some(i);
                                    ui.close_menu();
                                }
                            });
                            if ir.inner.clicked() { open_obj = Some(i); }
                        }
                        if let Some((from, to)) = pending_move {
                            if from < self.world_objects.len() && to < self.world_objects.len() {
                                let item = self.world_objects.remove(from);
                                self.world_objects.insert(to, item);
                                if let Some(sel) = self.selected_obj_idx {
                                    if sel == from {
                                        self.selected_obj_idx = Some(to);
                                    } else if from < to && sel > from && sel <= to {
                                        self.selected_obj_idx = Some(sel - 1);
                                    } else if from > to && sel >= to && sel < from {
                                        self.selected_obj_idx = Some(sel + 1);
                                    }
                                }
                            }
                        }
                    } else {
                        // Card view: each object as a small styled card
                        for (i, obj) in self.world_objects.iter().enumerate() {
                            // Apply kind filter
                            if let Some(ref filter) = self.obj_kind_filter {
                                if &obj.kind != filter { continue; }
                            }
                            let selected = self.selected_obj_idx == Some(i);
                            let bg = if selected {
                                Color32::from_rgb(0, 80, 140)
                            } else {
                                Color32::from_gray(38)
                            };
                            let card_resp = egui::Frame::none()
                                .fill(bg)
                                .rounding(6.0)
                                .inner_margin(egui::Margin::symmetric(8.0, 6.0))
                                .show(ui, |ui| {
                                    ui.set_min_width(ui.available_width());
                                    ui.horizontal(|ui| {
                                        ui.label(RichText::new(obj.icon()).size(20.0));
                                        ui.vertical(|ui| {
                                            ui.label(RichText::new(&obj.name).strong());
                                            ui.label(
                                                RichText::new(obj.kind.label())
                                                    .small()
                                                    .color(Color32::from_gray(160)),
                                            );
                                            if !obj.description.is_empty() {
                                                let preview: String = obj.description
                                                    .chars().take(30).collect();
                                                let suffix = if obj.description.chars().count() > 30 { "…" } else { "" };
                                                ui.label(
                                                    RichText::new(format!("{preview}{suffix}"))
                                                        .small()
                                                        .color(Color32::from_gray(140)),
                                                );
                                            }
                                            if !obj.links.is_empty() {
                                                ui.label(
                                                    RichText::new(format!("🔗 {} 个关联", obj.links.len()))
                                                        .small()
                                                        .color(Color32::from_rgb(120, 180, 240)),
                                                );
                                            }
                                        });
                                    });
                                })
                                .response;
                            let card_resp = card_resp.interact(egui::Sense::click());
                            card_resp.context_menu(|ui| {
                                if ui.button("删除").clicked() {
                                    remove_obj = Some(i);
                                    ui.close_menu();
                                }
                            });
                            if card_resp.clicked() {
                                open_obj = Some(i);
                            }
                            ui.add_space(4.0);
                        }
                    }
                });

                ui.separator();
                // Add-object row
                ui.horizontal(|ui| {
                    ui.text_edit_singleline(&mut self.new_obj_name)
                        .on_hover_text("输入对象名称");
                    egui::ComboBox::from_id_salt("new_obj_kind")
                        .selected_text(format!("{} {}", self.new_obj_kind.icon(), self.new_obj_kind.label()))
                        .width(80.0)
                        .show_ui(ui, |ui| {
                            for k in ObjectKind::all() {
                                let label = format!("{} {}", k.icon(), k.label());
                                ui.selectable_value(&mut self.new_obj_kind, k.clone(), label);
                            }
                        });
                });
                if ui.button("➕ 添加对象").clicked() {
                    let name = self.new_obj_name.trim().to_owned();
                    if !name.is_empty() {
                        let idx = self.world_objects.len();
                        self.world_objects.push(WorldObject::new(&name, self.new_obj_kind.clone()));
                        self.selected_obj_idx = Some(idx);
                        self.new_obj_name.clear();
                    }
                }
            });

        // Apply deferred mutations
        if let Some(i) = open_obj { self.selected_obj_idx = Some(i); }
        if let Some(i) = remove_obj {
            self.world_objects.remove(i);
            match self.selected_obj_idx {
                Some(s) if s == i => self.selected_obj_idx = None,
                Some(s) if s > i  => self.selected_obj_idx = Some(s - 1),
                _ => {}
            }
        }

        // ── Central: object editor + links ─────────────────────────────────────
        egui::CentralPanel::default().show(ctx, |ui| {
            let Some(idx) = self.selected_obj_idx else {
                ui.centered_and_justified(|ui| {
                    if self.world_objects.is_empty() {
                        ui.label(RichText::new("← 在左侧添加对象以开始编辑").color(Color32::GRAY));
                    } else {
                        ui.label(RichText::new("← 点击左侧对象名称以编辑").color(Color32::GRAY));
                    }
                });
                return;
            };

            // Collect autocomplete lists before mutable borrow
            let obj_names   = self.all_object_names();
            let node_titles = self.all_struct_node_titles();

            let mut do_sync = false;
            let mut do_add_link = false;
            let mut remove_link: Option<usize> = None;

            if let Some(obj) = self.world_objects.get_mut(idx) {
                egui::ScrollArea::vertical().id_salt("obj_editor_scroll").show(ui, |ui| {
                    // Header
                    ui.horizontal(|ui| {
                        ui.heading(format!("{} {}", obj.kind.icon(), obj.name.clone()));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button("💾 同步到 JSON").clicked() { do_sync = true; }
                        });
                    });
                    ui.separator();

                    // Kind selector
                    ui.horizontal(|ui| {
                        ui.label("类型:");
                        for k in ObjectKind::all() {
                            let sel = &obj.kind == k;
                            if ui.selectable_label(sel,
                                format!("{} {}", k.icon(), k.label())).clicked()
                            {
                                obj.kind = k.clone();
                            }
                        }
                    });

                    ui.horizontal(|ui| {
                        ui.label("名称:");
                        ui.text_edit_singleline(&mut obj.name);
                    });

                    ui.add_space(4.0);
                    ui.label("描述 / 核心特质:");
                    ui.add(egui::TextEdit::multiline(&mut obj.description)
                        .desired_rows(3)
                        .desired_width(f32::INFINITY));

                    ui.add_space(4.0);
                    ui.label("背景故事:");
                    ui.add(egui::TextEdit::multiline(&mut obj.background)
                        .desired_rows(4)
                        .desired_width(f32::INFINITY));

                    ui.add_space(8.0);
                    ui.separator();

                    // ── Links (associations) ───────────────────────────────────
                    ui.heading("关联");
                    ui.label(RichText::new(
                        "可关联其他对象（人物、场景…）或章节结构节点（章、节…）"
                    ).color(Color32::from_gray(140)).small());
                    ui.add_space(4.0);

                    // Existing links table
                    if obj.links.is_empty() {
                        ui.label(RichText::new("暂无关联，请在下方添加").color(Color32::GRAY));
                    } else {
                        egui::Grid::new("links_grid")
                            .num_columns(4)
                            .spacing([8.0, 4.0])
                            .striped(true)
                            .show(ui, |ui| {
                                ui.label(RichText::new("目标类型").small());
                                ui.label(RichText::new("目标").small());
                                ui.label(RichText::new("关联类型").small());
                                ui.label(RichText::new("备注").small());
                                ui.end_row();
                                for (li, link) in obj.links.iter().enumerate() {
                                    ui.label(RichText::new(link.target.type_label()).small()
                                        .color(Color32::from_rgb(120, 180, 240)));
                                    ui.label(RichText::new(link.target.display_name()).small());
                                    ui.label(RichText::new(link.kind.label()).small());
                                    ui.label(RichText::new(&link.note).small()
                                        .color(Color32::from_gray(160)));
                                    if ui.small_button("🗑").clicked() {
                                        remove_link = Some(li);
                                    }
                                    ui.end_row();
                                }
                            });
                    }

                    if let Some(li) = remove_link { obj.links.remove(li); }

                    ui.add_space(4.0);
                    ui.separator();
                    ui.label("添加关联:");
                    ui.horizontal(|ui| {
                        // Toggle target type
                        ui.label("类型:");
                        if ui.selectable_label(!self.new_link_is_node, "对象").clicked() {
                            self.new_link_is_node = false;
                        }
                        if ui.selectable_label(self.new_link_is_node, "章节").clicked() {
                            self.new_link_is_node = true;
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.label("目标名称:");
                        let hint = if self.new_link_is_node { "节点标题" } else { "对象名称" };
                        ui.add(egui::TextEdit::singleline(&mut self.new_link_name)
                            .hint_text(hint)
                            .desired_width(120.0));
                        // Auto-complete hint
                        let candidates: Vec<&str> = if self.new_link_is_node {
                            node_titles.iter().map(|s| s.as_str()).collect()
                        } else {
                            obj_names.iter().map(|s| s.as_str()).collect()
                        };
                        if !self.new_link_name.is_empty() {
                            let matches: Vec<&str> = candidates.iter()
                                .filter(|c| c.contains(self.new_link_name.as_str()))
                                .copied()
                                .take(3)
                                .collect();
                            if !matches.is_empty() {
                                ui.label(
                                    RichText::new(matches.join(" / ")).small()
                                        .color(Color32::from_gray(150))
                                );
                            }
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.label("关系类型:");
                        egui::ComboBox::from_id_salt("new_link_rel")
                            .selected_text(self.new_link_rel_kind.label())
                            .width(90.0)
                            .show_ui(ui, |ui| {
                                for k in RelationKind::all() {
                                    ui.selectable_value(
                                        &mut self.new_link_rel_kind, k.clone(), k.label());
                                }
                            });
                        ui.label("备注:");
                        ui.add(egui::TextEdit::singleline(&mut self.new_link_note)
                            .desired_width(100.0));
                        if ui.button("➕").clicked() {
                            let name = self.new_link_name.trim().to_owned();
                            if !name.is_empty() { do_add_link = true; }
                        }
                    });
                });
            }

            // Deferred mutations (outside the obj borrow)
            if do_add_link {
                let name = self.new_link_name.trim().to_owned();
                let target = if self.new_link_is_node {
                    LinkTarget::Node(name)
                } else {
                    LinkTarget::Object(name)
                };
                if let Some(obj) = self.world_objects.get_mut(idx) {
                    obj.links.push(ObjectLink {
                        target,
                        kind: self.new_link_rel_kind.clone(),
                        note: self.new_link_note.trim().to_owned(),
                    });
                }
                self.new_link_name.clear();
                self.new_link_note.clear();
            }

            if do_sync { self.sync_world_objects_to_json(); }

            // ── Reverse-lookup: which structure nodes link to this object? ─────
            // Show in a compact read-only section below the editor.
            let obj_name = self.world_objects.get(idx).map(|o| o.name.clone()).unwrap_or_default();
            let reverse = Self::collect_nodes_linking_object(&self.struct_roots, &obj_name);
            if !reverse.is_empty() {
                egui::TopBottomPanel::bottom("obj_reverse_links")
                    .resizable(false)
                    .show_inside(ui, |ui| {
                        ui.separator();
                        ui.label(
                            RichText::new(format!("📌 章节结构中出现「{}」的节点: {}",
                                obj_name, reverse.join("、")))
                            .small()
                            .color(Color32::from_rgb(120, 190, 120)),
                        );
                    });
            }
        });
    }

    /// Collect titles of all `StructNode`s that list `obj_name` in their `linked_objects`.
    fn collect_nodes_linking_object(roots: &[StructNode], obj_name: &str) -> Vec<String> {
        let mut out = Vec::new();
        fn walk(nodes: &[StructNode], name: &str, out: &mut Vec<String>) {
            for n in nodes {
                if n.linked_objects.iter().any(|o| o == name) {
                    out.push(n.title.clone());
                }
                walk(&n.children, name, out);
            }
        }
        walk(roots, obj_name, &mut out);
        out
    }
}
