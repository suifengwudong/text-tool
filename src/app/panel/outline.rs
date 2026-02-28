use egui::{Context, RichText, Color32};
use super::super::{
    TextToolApp, StructNode, StructKind, ChapterTag, NodeLink, RelationKind,
    Foreshadow, node_at_mut,
};

impl TextToolApp {
    // ── Panel: Chapter Structure ──────────────────────────────────────────────
    //
    // Left: hierarchical struct tree (总纲/卷/章/节) with add/remove/reorder
    // Central: selected node editor + linked objects + node cross-links
    // Bottom strip: progress tracking + foreshadow management

    pub(in crate::app) fn draw_structure_panel(&mut self, ctx: &Context) {
        // Collect pending tree mutations here to apply after draw passes
        let mut add_root: Option<(String, StructKind)> = None;
        let mut add_child: Option<(Vec<usize>, String, StructKind)> = None;
        let mut remove_node: Option<Vec<usize>> = None;
        let mut move_up: Option<Vec<usize>> = None;

        // ── Left: struct tree ──────────────────────────────────────────────────
        egui::SidePanel::left("struct_tree")
            .resizable(true)
            .default_width(240.0)
            .min_width(160.0)
            .show(ctx, |ui| {
                ui.add_space(4.0);
                ui.heading("章节结构");
                ui.separator();

                // Add root node controls
                ui.horizontal(|ui| {
                    ui.add(egui::TextEdit::singleline(&mut self.new_node_title)
                        .hint_text("标题")
                        .desired_width(90.0));
                    egui::ComboBox::from_id_salt("new_node_kind")
                        .selected_text(format!("{} {}", self.new_node_kind.icon(), self.new_node_kind.label()))
                        .width(70.0)
                        .show_ui(ui, |ui| {
                            for k in StructKind::all() {
                                let label = format!("{} {}", k.icon(), k.label());
                                ui.selectable_value(&mut self.new_node_kind, k.clone(), label);
                            }
                        });
                    if ui.button("➕").on_hover_text("添加根节点").clicked() {
                        let title = self.new_node_title.trim().to_owned();
                        if !title.is_empty() {
                            add_root = Some((title, self.new_node_kind.clone()));
                            self.new_node_title.clear();
                        }
                    }
                });
                ui.separator();

                egui::ScrollArea::vertical().id_salt("struct_tree_scroll").show(ui, |ui| {
                    let roots_snapshot = self.struct_roots.clone();
                    let selected = self.selected_node_path.clone();
                    Self::draw_struct_tree(
                        ui, &roots_snapshot, &selected, &[],
                        &mut add_child, &mut remove_node, &mut move_up,
                        &mut self.selected_node_path,
                    );
                });

                ui.separator();
                if ui.button("💾 同步结构到 JSON").clicked() {
                    self.sync_struct_to_json();
                }
            });

        // ── Apply deferred tree mutations ──────────────────────────────────────
        if let Some((title, kind)) = add_root {
            let idx = self.struct_roots.len();
            self.struct_roots.push(StructNode::new(&title, kind));
            self.selected_node_path = vec![idx];
        }
        if let Some((parent_path, title, kind)) = add_child {
            if let Some(parent) = node_at_mut(&mut self.struct_roots, &parent_path) {
                let child_idx = parent.children.len();
                parent.children.push(StructNode::new(&title, kind));
                let mut new_path = parent_path.clone();
                new_path.push(child_idx);
                self.selected_node_path = new_path;
            }
        }
        if let Some(path) = remove_node {
            Self::remove_node_at(&mut self.struct_roots, &path);
            if self.selected_node_path.starts_with(&path) {
                self.selected_node_path.clear();
            }
        }
        if let Some(path) = move_up {
            Self::move_node_up(&mut self.struct_roots, &path);
            // Adjust selection if it was pointing at the moved node
            if let Some(last) = path.last() {
                if *last > 0 {
                    let mut new_path = path.clone();
                    *new_path.last_mut().unwrap() -= 1;
                    if self.selected_node_path == path {
                        self.selected_node_path = new_path;
                    }
                }
            }
        }

        // ── Central: node editor ───────────────────────────────────────────────
        egui::CentralPanel::default().show(ctx, |ui| {
            // Top strip: progress overview derived from all struct nodes
            let (total, done) = Self::count_progress(&self.struct_roots);
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    ui.heading("进度追踪");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("💾 同步伏笔到 MD").clicked() {
                            self.sync_foreshadows_to_md();
                        }
                    });
                });
                ui.separator();
                if total == 0 {
                    ui.label(RichText::new("暂无叶节点，请在左侧添加章/节").color(Color32::GRAY));
                } else {
                    ui.horizontal(|ui| {
                        ui.label(format!("叶节点完成度: {done}/{total}"));
                        ui.add(egui::ProgressBar::new(done as f32 / total as f32)
                            .desired_width(180.0));
                    });
                }
            });
            ui.add_space(4.0);

            // ── Selected node detail ───────────────────────────────────────────
            if self.selected_node_path.is_empty() {
                ui.separator();
                // Foreshadow management when nothing is selected
                self.draw_foreshadow_section(ui);
                return;
            }

            // Collect data before mutable borrow
            let obj_names   = self.all_object_names();
            let node_titles = self.all_struct_node_titles();
            let path = self.selected_node_path.clone();

            let mut do_add_obj_link  = false;
            let mut do_add_node_link = false;
            // Set to Some(child_idx) when the inline "add child" button is clicked.
            let mut add_inline_child: Option<usize> = None;

            if let Some(node) = node_at_mut(&mut self.struct_roots, &path) {
                egui::ScrollArea::vertical().id_salt("node_editor_scroll").show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.heading(format!("{} {}",
                            node.kind.icon(), node.title.clone()));
                        // Add child button
                        let child_kind = node.kind.default_child_kind();
                        if ui.button(format!("➕ 添加子{}", child_kind.label()))
                            .on_hover_text("添加子节点到此项").clicked()
                        {
                            let child_idx = node.children.len();
                            node.children.push(StructNode::new(
                                &format!("新{}", child_kind.label()),
                                child_kind.clone(),
                            ));
                            // Signal to update selection after this borrow ends.
                            add_inline_child = Some(child_idx);
                        }
                    });
                    ui.separator();

                    // Kind selector
                    ui.horizontal(|ui| {
                        ui.label("层级:");
                        for k in StructKind::all() {
                            let sel = &node.kind == k;
                            if ui.selectable_label(sel,
                                format!("{} {}", k.icon(), k.label())).clicked()
                            {
                                node.kind = k.clone();
                            }
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.label("标题:");
                        ui.text_edit_singleline(&mut node.title);
                    });
                    ui.horizontal(|ui| {
                        ui.label("标签:");
                        for tag in ChapterTag::all() {
                            let sel = &node.tag == tag;
                            if ui.selectable_label(sel,
                                RichText::new(tag.label()).color(tag.color())).clicked()
                            {
                                node.tag = tag.clone();
                            }
                        }
                        ui.checkbox(&mut node.done, "已完成");
                    });
                    ui.label("摘要:");
                    ui.add(egui::TextEdit::multiline(&mut node.summary)
                        .desired_rows(3)
                        .desired_width(f32::INFINITY));

                    ui.add_space(6.0);
                    ui.separator();

                    // ── Linked world objects ───────────────────────────────────
                    ui.label(RichText::new("关联的世界对象:").strong());
                    if node.linked_objects.is_empty() {
                        ui.label(RichText::new("（暂无关联对象）").color(Color32::GRAY).small());
                    } else {
                        let mut rm: Option<usize> = None;
                        for (i, name) in node.linked_objects.iter().enumerate() {
                            ui.horizontal(|ui| {
                                ui.label(format!("• {name}"));
                                if ui.small_button("🗑").clicked() { rm = Some(i); }
                            });
                        }
                        if let Some(i) = rm { node.linked_objects.remove(i); }
                    }
                    ui.horizontal(|ui| {
                        ui.add(egui::TextEdit::singleline(&mut self.new_node_obj_link)
                            .hint_text("对象名称")
                            .desired_width(120.0));
                        // Autocomplete hint
                        if !self.new_node_obj_link.is_empty() {
                            let m: Vec<&str> = obj_names.iter()
                                .filter(|n| n.contains(self.new_node_obj_link.as_str()))
                                .map(|s| s.as_str()).take(3).collect();
                            if !m.is_empty() {
                                ui.label(RichText::new(m.join(" / ")).small()
                                    .color(Color32::from_gray(150)));
                            }
                        }
                        if ui.button("➕ 关联对象").clicked() {
                            do_add_obj_link = true;
                        }
                    });

                    ui.add_space(6.0);
                    ui.separator();

                    // ── Cross-node links ───────────────────────────────────────
                    ui.label(RichText::new("跨节点关联 (非父子):").strong());
                    ui.label(RichText::new(
                        "可在此记录与其他章节节点的铺垫/回收/并行等关系"
                    ).color(Color32::from_gray(140)).small());
                    if node.node_links.is_empty() {
                        ui.label(RichText::new("（暂无跨节点关联）").color(Color32::GRAY).small());
                    } else {
                        let mut rm: Option<usize> = None;
                        egui::Grid::new("node_links_grid")
                            .num_columns(3)
                            .spacing([8.0, 4.0])
                            .striped(true)
                            .show(ui, |ui| {
                                for (i, nl) in node.node_links.iter().enumerate() {
                                    ui.label(RichText::new(&nl.target_title).small());
                                    ui.label(RichText::new(nl.kind.label()).small()
                                        .color(Color32::from_rgb(200, 160, 100)));
                                    ui.label(RichText::new(&nl.note).small()
                                        .color(Color32::from_gray(160)));
                                    if ui.small_button("🗑").clicked() { rm = Some(i); }
                                    ui.end_row();
                                }
                            });
                        if let Some(i) = rm { node.node_links.remove(i); }
                    }
                    ui.horizontal(|ui| {
                        ui.add(egui::TextEdit::singleline(&mut self.new_node_link_title)
                            .hint_text("目标节点标题")
                            .desired_width(110.0));
                        // Autocomplete
                        if !self.new_node_link_title.is_empty() {
                            let m: Vec<&str> = node_titles.iter()
                                .filter(|t| t.contains(self.new_node_link_title.as_str()))
                                .map(|s| s.as_str()).take(3).collect();
                            if !m.is_empty() {
                                ui.label(RichText::new(m.join(" / ")).small()
                                    .color(Color32::from_gray(150)));
                            }
                        }
                        egui::ComboBox::from_id_salt("new_node_link_kind")
                            .selected_text(self.new_node_link_kind.label())
                            .width(80.0)
                            .show_ui(ui, |ui| {
                                for k in RelationKind::all() {
                                    ui.selectable_value(
                                        &mut self.new_node_link_kind, k.clone(), k.label());
                                }
                            });
                        ui.add(egui::TextEdit::singleline(&mut self.new_node_link_note)
                            .hint_text("备注")
                            .desired_width(80.0));
                        if ui.button("➕").clicked() {
                            let t = self.new_node_link_title.trim().to_owned();
                            if !t.is_empty() { do_add_node_link = true; }
                        }
                    });
                });
            }

            // Deferred: update selection after inline child add
            if let Some(child_idx) = add_inline_child {
                let mut new_path = path.clone();
                new_path.push(child_idx);
                self.selected_node_path = new_path;
            }
            // Deferred: add linked object
            if do_add_obj_link {
                let name = self.new_node_obj_link.trim().to_owned();
                if let Some(node) = node_at_mut(&mut self.struct_roots, &path) {
                    if !node.linked_objects.contains(&name) {
                        node.linked_objects.push(name);
                    }
                }
                self.new_node_obj_link.clear();
            }
            // Deferred: add node cross-link
            if do_add_node_link {
                let title = self.new_node_link_title.trim().to_owned();
                if let Some(node) = node_at_mut(&mut self.struct_roots, &path) {
                    node.node_links.push(NodeLink {
                        target_title: title,
                        kind: self.new_node_link_kind.clone(),
                        note: self.new_node_link_note.trim().to_owned(),
                    });
                }
                self.new_node_link_title.clear();
                self.new_node_link_note.clear();
            }

            // Foreshadow section at the bottom
            ui.separator();
            self.draw_foreshadow_section(ui);
        });
    }

    // ── Struct tree recursive renderer ────────────────────────────────────────

    #[allow(clippy::too_many_arguments)]
    fn draw_struct_tree(
        ui: &mut egui::Ui,
        nodes: &[StructNode],
        selected: &[usize],
        path: &[usize],
        add_child: &mut Option<(Vec<usize>, String, StructKind)>,
        remove_node: &mut Option<Vec<usize>>,
        move_up: &mut Option<Vec<usize>>,
        selected_path: &mut Vec<usize>,
    ) {
        for (i, node) in nodes.iter().enumerate() {
            let mut cur_path = path.to_vec();
            cur_path.push(i);

            let is_selected = *selected == cur_path;
            let indent = path.len() as f32 * 14.0;

            ui.horizontal(|ui| {
                ui.add_space(indent);
                let label = format!("{} {}", node.kind.icon(), node.title);
                let resp = ui.selectable_label(is_selected, &label);
                if resp.clicked() {
                    *selected_path = cur_path.clone();
                }
                resp.context_menu(|ui| {
                    let child_kind = node.kind.default_child_kind();
                    if ui.button(format!("➕ 添加子{}", child_kind.label())).clicked() {
                        *add_child = Some((
                            cur_path.clone(),
                            format!("新{}", child_kind.label()),
                            child_kind,
                        ));
                        ui.close_menu();
                    }
                    if i > 0 && ui.button("↑ 上移").clicked() {
                        *move_up = Some(cur_path.clone());
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("🗑 删除").clicked() {
                        *remove_node = Some(cur_path.clone());
                        ui.close_menu();
                    }
                });
                // Done indicator
                let done_icon = if node.done { "✅" } else { "⏳" };
                ui.label(RichText::new(done_icon).small());
                // Tag badge
                if node.tag != ChapterTag::Normal {
                    ui.label(RichText::new(node.tag.label())
                        .small().color(node.tag.color()));
                }
            });

            if !node.children.is_empty() {
                Self::draw_struct_tree(
                    ui, &node.children, selected, &cur_path,
                    add_child, remove_node, move_up, selected_path,
                );
            }
        }
    }

    // ── Tree mutation helpers ──────────────────────────────────────────────────

    fn remove_node_at(roots: &mut Vec<StructNode>, path: &[usize]) {
        if path.is_empty() { return; }
        if path.len() == 1 {
            if path[0] < roots.len() { roots.remove(path[0]); }
            return;
        }
        if let Some(parent) = node_at_mut(roots, &path[..path.len() - 1]) {
            let idx = *path.last().unwrap();
            if idx < parent.children.len() { parent.children.remove(idx); }
        }
    }

    fn move_node_up(roots: &mut Vec<StructNode>, path: &[usize]) {
        if path.is_empty() { return; }
        let idx = *path.last().unwrap();
        if idx == 0 { return; }
        if path.len() == 1 {
            roots.swap(idx - 1, idx);
            return;
        }
        if let Some(parent) = node_at_mut(roots, &path[..path.len() - 1]) {
            if idx < parent.children.len() {
                parent.children.swap(idx - 1, idx);
            }
        }
    }

    fn count_progress(roots: &[StructNode]) -> (usize, usize) {
        let total: usize = roots.iter().map(|n| n.leaf_count()).sum();
        let done:  usize = roots.iter().map(|n| n.done_count()).sum();
        (total, done)
    }

    // ── Foreshadow sub-section (shared with no-selection state) ───────────────

    fn draw_foreshadow_section(&mut self, ui: &mut egui::Ui) {
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
                        self.foreshadows.push(Foreshadow::new(&name));
                        self.selected_fs_idx = Some(idx);
                        self.new_fs_name.clear();
                    }
                }
            });

            ui.add_space(4.0);

            ui.columns(2, |cols| {
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
    }
}
