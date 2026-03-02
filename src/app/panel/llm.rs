use egui::{RichText, Color32};
use super::super::{TextToolApp, LlmTask, PromptTemplate};

impl TextToolApp {
    // ── Panel: LLM Assistance ─────────────────────────────────────────────────

    pub(in crate::app) fn draw_llm_panel(&mut self, ctx: &egui::Context) {
        // Poll for completed background task each frame
        if let Some(task) = &self.llm_task {
            match task.receiver.try_recv() {
                Ok(Ok(text)) => {
                    self.llm_output = text;
                    self.status = "LLM 补全完成".to_owned();
                    self.llm_task = None;
                    ctx.request_repaint();
                }
                Ok(Err(e)) => {
                    self.llm_output = format!("【错误】{e}");
                    self.status = format!("LLM 调用失败: {e}");
                    self.llm_task = None;
                    ctx.request_repaint();
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {
                    ctx.request_repaint();
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.llm_output = "【错误】后台线程意外断开".to_owned();
                    self.llm_task = None;
                }
            }
        }

        let is_running = self.llm_task.is_some();

        // Collect names before mutable borrows below.
        let char_names: Vec<String> = self.world_objects.iter()
            .map(|o| o.name.clone())
            .collect();

        egui::SidePanel::left("llm_config")
            .resizable(true)
            .default_width(260.0)
            .min_width(180.0)
            .show(ctx, |ui| {
                ui.add_space(4.0);
                ui.heading("LLM 配置");
                ui.separator();

                // ── Backend selector ───────────────────────────────────────────
                ui.label("接口类型:");
                ui.horizontal_wrapped(|ui| {
                    if ui.selectable_label(self.llm_backend_idx == 0, "🧪 模拟模型").clicked() {
                        self.llm_backend_idx = 0;
                    }
                    if ui.selectable_label(self.llm_backend_idx == 1, "🌐 HTTP API").clicked() {
                        self.llm_backend_idx = 1;
                    }
                    if ui.selectable_label(self.llm_backend_idx == 2, "🖥 本地服务器").clicked() {
                        self.llm_backend_idx = 2;
                    }
                    if ui.selectable_label(self.llm_backend_idx == 3, "🤖 Agent").clicked() {
                        self.llm_backend_idx = 3;
                    }
                });
                ui.add_space(4.0);
                ui.separator();

                match self.llm_backend_idx {
                    1 => {
                        // ── HTTP API (Ollama / OpenAI) ─────────────────────────
                        ui.checkbox(&mut self.llm_config.use_local, "本地模型 (Ollama)");
                        ui.add_space(4.0);
                        if self.llm_config.use_local {
                            ui.label("模型名称:");
                            ui.text_edit_singleline(&mut self.llm_config.model_path)
                                .on_hover_text("Ollama 模型名称，如 llama2、phi3 等");
                            ui.add_space(4.0);
                            ui.label("API 地址:");
                            ui.text_edit_singleline(&mut self.llm_config.api_url)
                                .on_hover_text("默认: http://localhost:11434/api/generate");
                        } else {
                            ui.label("API 地址 (OpenAI 兼容):");
                            ui.text_edit_singleline(&mut self.llm_config.api_url)
                                .on_hover_text("如 https://api.openai.com/v1/chat/completions");
                            ui.add_space(4.0);
                            ui.label("模型名称:");
                            ui.text_edit_singleline(&mut self.llm_config.model_path)
                                .on_hover_text("如 gpt-4o、gpt-3.5-turbo 等");
                        }
                        ui.add_space(6.0);
                        ui.label("系统提示词 (可选):");
                        ui.add(egui::TextEdit::multiline(&mut self.llm_config.system_prompt)
                            .desired_rows(3)
                            .desired_width(f32::INFINITY)
                            .hint_text("例如：你是一个专业的小说编辑，请用中文回复。"));
                    }
                    2 => {
                        // ── Local llama.cpp server ─────────────────────────────
                        ui.label(
                            RichText::new("启动 llama.cpp 服务器:\n./server -m model.gguf \\\n  -c 2048 --port 8080")
                                .color(Color32::from_gray(150))
                                .small()
                                .monospace(),
                        );
                        ui.add_space(4.0);
                        ui.label("服务器地址:");
                        ui.text_edit_singleline(&mut self.llm_config.api_url)
                            .on_hover_text("默认: http://127.0.0.1:8080");
                        ui.add_space(6.0);
                        ui.label("系统提示词 (可选):");
                        ui.add(egui::TextEdit::multiline(&mut self.llm_config.system_prompt)
                            .desired_rows(3)
                            .desired_width(f32::INFINITY)
                            .hint_text("例如：你是一个专业的小说编辑，请用中文回复。"));
                    }
                    3 => {
                        // ── Agent (tool-calling loop) ──────────────────────────
                        ui.label(
                            RichText::new("需要支持工具调用的 OpenAI 兼容 API\n（如 gpt-4o、deepseek-chat）")
                                .color(Color32::from_rgb(200, 180, 80))
                                .small(),
                        );
                        ui.add_space(4.0);
                        ui.label("API 地址:");
                        ui.text_edit_singleline(&mut self.llm_config.api_url)
                            .on_hover_text("如 https://api.openai.com/v1/chat/completions");
                        ui.add_space(4.0);
                        ui.label("模型名称:");
                        ui.text_edit_singleline(&mut self.llm_config.model_path)
                            .on_hover_text("如 gpt-4o、deepseek-chat 等");
                        ui.add_space(6.0);
                        ui.label("系统提示词 (可选):");
                        ui.add(egui::TextEdit::multiline(&mut self.llm_config.system_prompt)
                            .desired_rows(2)
                            .desired_width(f32::INFINITY)
                            .hint_text("例如：你是一个专业的小说编辑。"));
                        ui.add_space(6.0);
                        ui.separator();
                        ui.label(RichText::new("当前可用技能:").small()
                            .color(Color32::from_gray(160)));
                        // Snapshot skill metadata (static names — no clone of data)
                        for (name, desc) in &[
                            ("list_characters",    "列出所有世界对象"),
                            ("get_character_info", "获取人物/对象详情"),
                            ("get_chapter_outline","获取章节结构大纲"),
                            ("search_foreshadows", "搜索伏笔列表"),
                        ] {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new("⚡").small()
                                    .color(Color32::from_rgb(100, 200, 120)));
                                ui.label(RichText::new(*name).small().monospace())
                                    .on_hover_text(*desc);
                            });
                        }
                    }
                    _ => {
                        // ── Mock ───────────────────────────────────────────────
                        ui.label(
                            RichText::new("使用内置模拟模型，\n无需配置。")
                                .color(Color32::from_gray(150))
                                .small(),
                        );
                    }
                }

                ui.add_space(8.0);
                ui.label(format!("温度 (Temperature): {:.2}", self.llm_config.temperature));
                ui.add(egui::Slider::new(&mut self.llm_config.temperature, 0.0..=2.0)
                    .step_by(0.05));

                ui.add_space(4.0);
                ui.label(format!("最大 Token: {}", self.llm_config.max_tokens));
                ui.add(egui::Slider::new(&mut self.llm_config.max_tokens, 64..=2048)
                    .step_by(64.0));

                ui.add_space(8.0);
                ui.separator();
                ui.label(
                    RichText::new("支持后端:\n🧪 模拟模型 (无需网络)\n🌐 Ollama / OpenAI API\n🖥 llama.cpp HTTP 服务器")
                        .color(Color32::from_gray(140))
                        .small(),
                );
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("LLM 辅助写作");
            ui.separator();

            // ── Prompt templates ───────────────────────────────────────────────
            // Snapshot context once; used only when a template button is clicked.
            let char_ctx = self.build_character_context();
            ui.label(RichText::new("快速模板:").small().color(Color32::from_gray(160)));
            ui.horizontal_wrapped(|ui| {
                for tmpl in PromptTemplate::all() {
                    if ui.small_button(tmpl.label()).clicked() {
                        let current = self.llm_prompt.clone();
                        self.llm_prompt = tmpl.fill(&char_ctx, &current);
                        self.status = format!("已应用模板: {}", tmpl.label());
                    }
                }
            });

            ui.add_space(4.0);
            ui.separator();

            // ── Structured context injection ───────────────────────────────────
            ui.label(RichText::new("注入结构化上下文 (追加到提示词末尾):").small()
                .color(Color32::from_gray(160)));
            ui.horizontal_wrapped(|ui| {
                if ui.button("👤 注入人物信息").clicked() {
                    let ctx_text = self.build_character_context();
                    if ctx_text.is_empty() {
                        self.status = "世界对象面板中暂无人物，请先添加".to_owned();
                    } else {
                        self.llm_prompt.push_str("\n\n");
                        self.llm_prompt.push_str(&ctx_text);
                        self.status = "已注入人物/世界对象信息".to_owned();
                    }
                }
                if ui.button("📖 注入章节结构").clicked() {
                    let ctx_text = self.build_structure_context();
                    if ctx_text.is_empty() {
                        self.status = "章节结构面板中暂无内容，请先添加".to_owned();
                    } else {
                        self.llm_prompt.push_str("\n\n");
                        self.llm_prompt.push_str(&ctx_text);
                        self.status = "已注入章节结构信息".to_owned();
                    }
                }
            });

            // ── Dialogue style optimisation ────────────────────────────────────
            ui.add_space(4.0);
            ui.separator();
            ui.label(RichText::new("人设对话风格优化:").small().color(Color32::from_gray(160)));
            ui.horizontal(|ui| {
                ui.label("选择人物:");
                egui::ComboBox::from_id_salt("dialogue_char_picker")
                    .selected_text(if self.llm_dialogue_char.is_empty() {
                        "（未选择）".to_owned()
                    } else {
                        self.llm_dialogue_char.clone()
                    })
                    .width(130.0)
                    .show_ui(ui, |ui| {
                        for name in &char_names {
                            ui.selectable_value(
                                &mut self.llm_dialogue_char,
                                name.clone(),
                                name,
                            );
                        }
                    });

                let can_optimize = !self.llm_dialogue_char.is_empty()
                    && !self.llm_prompt.trim().is_empty();
                ui.add_enabled_ui(can_optimize, |ui| {
                    if ui.button("✨ 优化对话风格").clicked() {
                        let char_name = self.llm_dialogue_char.clone();
                        let dialogue_text = self.llm_prompt.clone();
                        if let Some(prompt) =
                            self.build_dialogue_optimization_prompt(&char_name, &dialogue_text)
                        {
                            let backend = self.make_llm_backend();
                            let config  = self.llm_config.clone();
                            self.llm_task = Some(LlmTask::spawn(backend, config, prompt));
                            self.status = format!("正在优化「{}」的对话风格…", char_name);
                        } else {
                            self.status = format!(
                                "未找到人物「{}」，请先在世界对象面板中添加",
                                char_name
                            );
                        }
                    }
                });
            });
            if char_names.is_empty() {
                ui.label(
                    RichText::new("  ← 请先在「世界对象」面板中添加人物")
                        .small()
                        .color(Color32::from_gray(120)),
                );
            }

            ui.add_space(6.0);
            ui.separator();

            // ── Prompt editor ──────────────────────────────────────────────────
            ui.label("提示词 / 上下文:");
            egui::ScrollArea::vertical()
                .id_salt("llm_prompt_scroll")
                .max_height(180.0)
                .show(ui, |ui| {
                    ui.add(
                        egui::TextEdit::multiline(&mut self.llm_prompt)
                            .desired_width(f32::INFINITY)
                            .desired_rows(7)
                            .hint_text("输入提示词，例如：\n续写以下场景：\n或 优化以下对话：\n\n也可用上方快速模板或注入按钮自动填充。")
                    );
                });

            ui.add_space(4.0);
            ui.horizontal(|ui| {
                if is_running {
                    ui.add(egui::Spinner::new());
                    ui.label(RichText::new("正在调用 LLM…").color(Color32::from_rgb(200, 200, 80)));
                    if ui.button("⏹ 取消").clicked() {
                        self.llm_task = None;
                        self.status = "已取消 LLM 调用".to_owned();
                    }
                } else {
                    if ui.button("▶ 调用 LLM 补全").clicked() {
                        let backend = self.make_llm_backend();
                        let config  = self.llm_config.clone();
                        let prompt  = self.llm_prompt.clone();
                        self.llm_task = Some(LlmTask::spawn(backend, config, prompt));
                        self.status = "LLM 调用已提交，后台处理中…".to_owned();
                    }
                    if ui.button("插入到左侧编辑区").clicked() {
                        if !self.llm_output.is_empty() {
                            if let Some(lf) = &mut self.left_file {
                                lf.content.push_str("\n\n");
                                lf.content.push_str(&self.llm_output);
                                lf.modified = true;
                                self.status = "已将 LLM 输出插入左侧编辑区".to_owned();
                            } else {
                                self.status = "请先在小说编辑面板打开 Markdown 文件".to_owned();
                            }
                        }
                    }
                    if ui.button("🗑 清空").clicked() {
                        self.llm_prompt.clear();
                        self.llm_output.clear();
                    }
                }
            });

            ui.add_space(8.0);
            ui.label("输出结果:");
            egui::ScrollArea::vertical()
                .id_salt("llm_output_scroll")
                .show(ui, |ui| {
                    ui.add(
                        egui::TextEdit::multiline(&mut self.llm_output)
                            .desired_width(f32::INFINITY)
                            .desired_rows(12)
                            .hint_text("LLM 输出将显示在这里")
                    );
                });
        });
    }
}

