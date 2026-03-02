use crate::app::LlmConfig;

// ── LlmBackend trait ──────────────────────────────────────────────────────────

/// Abstraction over different LLM backends.
///
/// Implementations are expected to be called from a **background thread**
/// so that the egui UI is never blocked.  The trait is `Send + Sync` to
/// support being wrapped in `Arc<dyn LlmBackend>` and shared across threads.
pub trait LlmBackend: Send + Sync {
    /// Send a completion request with the given prompt and return the
    /// model's text response, or a human-readable error string on failure.
    fn complete(&self, config: &LlmConfig, prompt: &str) -> Result<String, String>;

    /// Human-readable name shown in the UI.
    fn name(&self) -> &'static str;
}

// ── MockBackend ───────────────────────────────────────────────────────────────

/// Simulated backend – returns a canned response without any network call.
/// Useful for offline development and tests.
pub struct MockBackend;

impl LlmBackend for MockBackend {
    fn name(&self) -> &'static str { "模拟模型" }

    fn complete(&self, config: &LlmConfig, prompt: &str) -> Result<String, String> {
        if prompt.trim().is_empty() {
            return Err("提示词为空，请输入内容后再试".to_owned());
        }
        Ok(format!(
            "【模拟输出 – 请配置真实模型】\n\n根据您的提示「{}…」，这里将显示模型生成的文本。\n\n当前配置:\n- {}: {}\n- 温度: {:.2}\n- 最大Token: {}",
            prompt.chars().take(30).collect::<String>(),
            if config.use_local { "本地模型" } else { "API" },
            if config.use_local { &config.model_path } else { &config.api_url },
            config.temperature,
            config.max_tokens,
        ))
    }
}

// ── ApiBackend ────────────────────────────────────────────────────────────────

/// HTTP API backend – supports both Ollama-style (`/api/generate`) and
/// OpenAI-compatible (`/v1/chat/completions`) endpoints.
///
/// Selection heuristic:
///   - URL ending in `/api/generate`    → Ollama request body
///   - URL ending in `/chat/completions` → OpenAI request body
///   - Otherwise                         → OpenAI request body
pub struct ApiBackend;

impl LlmBackend for ApiBackend {
    fn name(&self) -> &'static str { "HTTP API" }

    fn complete(&self, config: &LlmConfig, prompt: &str) -> Result<String, String> {
        let url = config.api_url.trim_end_matches('/');

        if url.ends_with("/api/generate") {
            Self::call_ollama(config, prompt)
        } else {
            Self::call_openai(config, prompt)
        }
    }
}

impl ApiBackend {
    /// Call an Ollama `/api/generate` endpoint.
    fn call_ollama(config: &LlmConfig, prompt: &str) -> Result<String, String> {
        let model = Self::model_name(config);
        let body = serde_json::json!({
            "model": model,
            "prompt": prompt,
            "stream": false,
            "options": {
                "temperature": config.temperature,
                "num_predict": config.max_tokens,
            }
        });

        let mut response = ureq::post(&config.api_url)
            .send_json(&body)
            .map_err(|e| format!("请求失败 ({}): {e}", config.api_url))?;

        let json: serde_json::Value = response
            .body_mut()
            .read_json()
            .map_err(|e| format!("响应解析失败: {e}"))?;

        json.get("response")
            .and_then(|v| v.as_str())
            .map(|s| s.to_owned())
            .ok_or_else(|| format!("无法从响应中读取 'response' 字段: {json}"))
    }

    /// Call an OpenAI-compatible `/v1/chat/completions` endpoint.
    fn call_openai(config: &LlmConfig, prompt: &str) -> Result<String, String> {
        let model = Self::model_name(config);

        // Build messages array; include system prompt if configured.
        let mut messages = Vec::new();
        if !config.system_prompt.trim().is_empty() {
            messages.push(serde_json::json!({"role": "system", "content": config.system_prompt}));
        }
        messages.push(serde_json::json!({"role": "user", "content": prompt}));

        let body = serde_json::json!({
            "model": model,
            "messages": messages,
            "temperature": config.temperature,
            "max_tokens": config.max_tokens,
        });

        let mut response = ureq::post(&config.api_url)
            .send_json(&body)
            .map_err(|e| format!("请求失败 ({}): {e}", config.api_url))?;

        let json: serde_json::Value = response
            .body_mut()
            .read_json()
            .map_err(|e| format!("响应解析失败: {e}"))?;

        json.get("choices")
            .and_then(|v| v.get(0))
            .and_then(|v| v.get("message"))
            .and_then(|v| v.get("content"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_owned())
            .ok_or_else(|| format!("无法从响应中读取 choices[0].message.content: {json}"))
    }

    /// Extract a model name from the config: use model_path if set, else a default.
    fn model_name(config: &LlmConfig) -> &str {
        let path = config.model_path.trim();
        if !path.is_empty() { path } else { "llama2" }
    }
}

// ── LocalServerBackend ────────────────────────────────────────────────────────

/// Backend for a locally-running llama.cpp HTTP server (native `/completion`
/// endpoint, **not** Ollama and not the OpenAI-compat shim).
///
/// Start a llama.cpp server with:
///   `./server -m model.gguf -c 2048 --host 127.0.0.1 --port 8080`
///
/// The server exposes `POST /completion` with the following schema:
///   Request:  `{ "prompt": "...", "temperature": 0.7, "n_predict": 512,
///                "system_prompt": { "prompt": "...", "anti_prompt": "..." } }`
///   Response: `{ "content": "..." }`
pub struct LocalServerBackend;

impl LlmBackend for LocalServerBackend {
    fn name(&self) -> &'static str { "本地服务器 (llama.cpp)" }

    fn complete(&self, config: &LlmConfig, prompt: &str) -> Result<String, String> {
        let url = config.api_url.trim_end_matches('/');
        // Append /completion if the URL doesn't already end with it.
        let endpoint = if url.ends_with("/completion") {
            url.to_owned()
        } else {
            format!("{url}/completion")
        };

        let mut body = serde_json::json!({
            "prompt": prompt,
            "temperature": config.temperature,
            "n_predict": config.max_tokens,
            "stream": false,
        });

        // Attach system prompt if provided (llama.cpp native format).
        if !config.system_prompt.trim().is_empty() {
            body["system_prompt"] = serde_json::json!({
                "prompt": config.system_prompt,
                "anti_prompt": ""
            });
        }

        let mut response = ureq::post(&endpoint)
            .send_json(&body)
            .map_err(|e| format!("请求失败 ({}): {e}", endpoint))?;

        let json: serde_json::Value = response
            .body_mut()
            .read_json()
            .map_err(|e| format!("响应解析失败: {e}"))?;

        json.get("content")
            .and_then(|v| v.as_str())
            .map(|s| s.to_owned())
            .ok_or_else(|| format!("无法从响应中读取 'content' 字段: {json}"))
    }
}

// ── PromptTemplate ────────────────────────────────────────────────────────────

/// Predefined prompt templates for common novel-writing tasks.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PromptTemplate {
    /// Continue writing from the given passage.
    Continuation,
    /// Expand / elaborate the given scene description.
    Expansion,
    /// Rewrite the given dialogue in a specific character's voice.
    DialogueOptimize,
    /// Generate a character summary from a description.
    CharacterSummary,
}

impl PromptTemplate {
    /// All templates in display order.
    pub fn all() -> &'static [PromptTemplate] {
        &[
            PromptTemplate::Continuation,
            PromptTemplate::Expansion,
            PromptTemplate::DialogueOptimize,
            PromptTemplate::CharacterSummary,
        ]
    }

    /// Short label shown on the button.
    pub fn label(self) -> &'static str {
        match self {
            PromptTemplate::Continuation     => "续写正文",
            PromptTemplate::Expansion        => "扩写场景",
            PromptTemplate::DialogueOptimize => "优化对话",
            PromptTemplate::CharacterSummary => "生成人物简介",
        }
    }

    /// Build the full prompt string.
    ///
    /// `context` – any background text (character info, chapter summary, …).
    /// `input`   – the user's current draft text / selection.
    pub fn fill(self, context: &str, input: &str) -> String {
        let ctx_block = if context.trim().is_empty() {
            String::new()
        } else {
            format!("{}\n\n", context.trim())
        };

        match self {
            PromptTemplate::Continuation => format!(
                "{ctx_block}请在下面段落之后续写正文，保持风格一致，不重复已有内容：\n\n{input}\n\n续写："
            ),
            PromptTemplate::Expansion => format!(
                "{ctx_block}请对下面的场景描写进行扩写，丰富细节、提升画面感：\n\n{input}\n\n扩写后："
            ),
            PromptTemplate::DialogueOptimize => format!(
                "{ctx_block}请将下面的对话改写，使其更符合人物性格，语言更自然生动：\n\n{input}\n\n改写后："
            ),
            PromptTemplate::CharacterSummary => format!(
                "{ctx_block}请根据以下人物信息，生成一段简洁的人物简介（100字以内）：\n\n{input}\n\n简介："
            ),
        }
    }
}

// ── LlmTask ───────────────────────────────────────────────────────────────────

/// State for a non-blocking LLM request running on a background thread.
/// The UI polls `try_recv()` each frame to check for completion.
pub struct LlmTask {
    pub receiver: std::sync::mpsc::Receiver<Result<String, String>>,
}

impl LlmTask {
    /// Spawn a background thread that calls `backend.complete(config, prompt)` and
    /// sends the result back through the returned `LlmTask`.
    pub fn spawn(
        backend: std::sync::Arc<dyn LlmBackend>,
        config: LlmConfig,
        prompt: String,
    ) -> Self {
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let result = backend.complete(&config, &prompt);
            let _ = tx.send(result);
        });
        LlmTask { receiver: rx }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::LlmConfig;

    fn default_config() -> LlmConfig {
        LlmConfig {
            model_path: String::new(),
            api_url: "http://localhost:11434/api/generate".to_owned(),
            temperature: 0.7,
            max_tokens: 512,
            use_local: true,
            system_prompt: String::new(),
        }
    }

    #[test]
    fn test_mock_backend_empty_prompt() {
        let backend = MockBackend;
        let result = backend.complete(&default_config(), "");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("提示词为空"));
    }

    #[test]
    fn test_mock_backend_with_prompt() {
        let backend = MockBackend;
        let result = backend.complete(&default_config(), "写一段开场白");
        assert!(result.is_ok());
        let text = result.unwrap();
        assert!(text.contains("模拟输出"));
    }

    #[test]
    fn test_mock_backend_name() {
        let backend = MockBackend;
        assert_eq!(backend.name(), "模拟模型");
    }

    #[test]
    fn test_api_backend_name() {
        let backend = ApiBackend;
        assert_eq!(backend.name(), "HTTP API");
    }

    #[test]
    fn test_local_server_backend_name() {
        let backend = LocalServerBackend;
        assert_eq!(backend.name(), "本地服务器 (llama.cpp)");
    }

    #[test]
    fn test_llm_task_mock() {
        let backend: std::sync::Arc<dyn LlmBackend> = std::sync::Arc::new(MockBackend);
        let task = LlmTask::spawn(backend, default_config(), "测试提示词".to_owned());
        let result = task.receiver.recv_timeout(std::time::Duration::from_secs(2));
        assert!(result.is_ok());
        assert!(result.unwrap().is_ok());
    }

    // ── PromptTemplate tests ──────────────────────────────────────────────────

    #[test]
    fn test_prompt_template_labels() {
        assert_eq!(PromptTemplate::Continuation.label(),     "续写正文");
        assert_eq!(PromptTemplate::Expansion.label(),        "扩写场景");
        assert_eq!(PromptTemplate::DialogueOptimize.label(), "优化对话");
        assert_eq!(PromptTemplate::CharacterSummary.label(), "生成人物简介");
    }

    #[test]
    fn test_prompt_template_continuation_no_context() {
        let prompt = PromptTemplate::Continuation.fill("", "主角走进了森林。");
        assert!(prompt.contains("续写正文") || prompt.contains("续写："));
        assert!(prompt.contains("主角走进了森林。"));
        // No leading blank context block
        assert!(!prompt.starts_with('\n'));
    }

    #[test]
    fn test_prompt_template_continuation_with_context() {
        let prompt = PromptTemplate::Continuation.fill("## 人物：李明", "他停下了脚步。");
        assert!(prompt.contains("李明"));
        assert!(prompt.contains("他停下了脚步。"));
    }

    #[test]
    fn test_prompt_template_expansion() {
        let prompt = PromptTemplate::Expansion.fill("", "夜晚，城市灯火通明。");
        assert!(prompt.contains("扩写"));
        assert!(prompt.contains("夜晚，城市灯火通明。"));
    }

    #[test]
    fn test_prompt_template_dialogue_optimize() {
        let prompt = PromptTemplate::DialogueOptimize.fill("性格：冷静", "\"我不在乎。\"");
        assert!(prompt.contains("对话"));
        assert!(prompt.contains("冷静"));
    }

    #[test]
    fn test_prompt_template_character_summary() {
        let prompt = PromptTemplate::CharacterSummary.fill("", "姓名：张三，年龄：25，职业：侦探。");
        assert!(prompt.contains("人物简介") || prompt.contains("简介："));
        assert!(prompt.contains("张三"));
    }

    #[test]
    fn test_prompt_template_all() {
        assert_eq!(PromptTemplate::all().len(), 4);
    }
}

