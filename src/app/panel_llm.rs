use egui::{Context, RichText, Color32};
use super::TextToolApp;

impl TextToolApp {
    // ── Panel: LLM Assistance ─────────────────────────────────────────────────

    pub(super) fn draw_llm_panel(&mut self, ctx: &Context) {
        egui::SidePanel::left("llm_config")
            .resizable(true)
            .default_width(240.0)
            .min_width(160.0)
            .show(ctx, |ui| {
                ui.add_space(4.0);
                ui.heading("LLM 配置");
                ui.separator();

                ui.checkbox(&mut self.llm_config.use_local, "使用本地模型");
                ui.add_space(4.0);

                if self.llm_config.use_local {
                    ui.label("模型路径:");
                    ui.text_edit_singleline(&mut self.llm_config.model_path)
                        .on_hover_text("本地模型文件路径 (.gguf 等)");
                } else {
                    ui.label("API 地址:");
                    ui.text_edit_singleline(&mut self.llm_config.api_url)
                        .on_hover_text("如 http://localhost:11434/api/generate");
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
                ui.label(RichText::new("支持模型:\nLlama 2 7B、Phi-2\n等本地轻量模型\n或兼容 OpenAI API\n的云端服务")
                    .color(Color32::from_gray(140))
                    .small());
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("LLM 辅助写作");
            ui.separator();

            ui.label("提示词 / 上下文:");
            egui::ScrollArea::vertical()
                .id_salt("llm_prompt_scroll")
                .max_height(200.0)
                .show(ui, |ui| {
                    ui.add(
                        egui::TextEdit::multiline(&mut self.llm_prompt)
                            .desired_width(f32::INFINITY)
                            .desired_rows(8)
                            .hint_text("输入提示词，例如：\n续写以下场景：\n或 优化以下对话：")
                    );
                });

            ui.add_space(4.0);
            ui.horizontal(|ui| {
                if ui.button("▶ 调用 LLM 补全").clicked() {
                    self.llm_output = self.llm_simulate();
                    self.status = "LLM 补全完成（模拟）".to_owned();
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

    /// Placeholder LLM call – returns a simulated response.
    /// Replace with actual HTTP/FFI call when integrating a real model.
    pub(super) fn llm_simulate(&self) -> String {
        if self.llm_prompt.trim().is_empty() {
            return "（提示词为空，请输入内容后再试）".to_owned();
        }
        format!(
            "【模拟输出 – 请配置真实模型】\n\n根据您的提示「{}…」，这里将显示模型生成的文本。\n\n当前配置:\n- {}: {}\n- 温度: {:.2}\n- 最大Token: {}",
            self.llm_prompt.chars().take(30).collect::<String>(),
            if self.llm_config.use_local { "本地模型" } else { "API" },
            if self.llm_config.use_local { &self.llm_config.model_path } else { &self.llm_config.api_url },
            self.llm_config.temperature,
            self.llm_config.max_tokens,
        )
    }
}
