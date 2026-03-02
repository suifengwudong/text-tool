use std::sync::Arc;
use serde_json::Value;

use super::llm_backend::LlmBackend;
use super::{LlmConfig, WorldObject, StructNode, Foreshadow, Milestone};

// ── Skill trait ───────────────────────────────────────────────────────────────

/// A discrete capability (tool) that the LLM agent can invoke.
///
/// Each skill is:
/// - Described to the LLM via `name()`, `description()`, and `parameters_schema()`
///   (OpenAI function-calling JSON Schema).
/// - Executed synchronously in the background thread via `execute(&args)`.
/// - Safe to share across threads (`Send + Sync`).
pub trait Skill: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    /// JSON Schema object describing the function parameters.
    fn parameters_schema(&self) -> Value;
    /// Execute the skill with the given arguments and return a JSON result.
    fn execute(&self, args: &Value) -> Result<Value, String>;

    /// Serialise this skill into the OpenAI `tools` array element format.
    fn to_openai_tool(&self) -> Value {
        serde_json::json!({
            "type": "function",
            "function": {
                "name": self.name(),
                "description": self.description(),
                "parameters": self.parameters_schema()
            }
        })
    }
}

// ── Built-in Skills ───────────────────────────────────────────────────────────

/// List all world objects (characters, locations, items, …) with their kind.
pub struct ListCharactersSkill(pub Vec<WorldObject>);

impl Skill for ListCharactersSkill {
    fn name(&self) -> &str { "list_characters" }

    fn description(&self) -> &str {
        "列出所有世界对象（人物/场景/道具/地点/势力等）的名称和类型"
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({ "type": "object", "properties": {} })
    }

    fn execute(&self, _args: &Value) -> Result<Value, String> {
        let list: Vec<Value> = self.0.iter().map(|o| {
            serde_json::json!({ "name": o.name, "kind": o.kind.label() })
        }).collect();
        Ok(Value::Array(list))
    }
}

/// Retrieve detailed information about a named world object.
pub struct GetCharacterInfoSkill(pub Vec<WorldObject>);

impl Skill for GetCharacterInfoSkill {
    fn name(&self) -> &str { "get_character_info" }

    fn description(&self) -> &str {
        "获取指定人物/对象的详细信息，包括描述、背景故事和关联关系"
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "人物或世界对象的名称"
                }
            },
            "required": ["name"]
        })
    }

    fn execute(&self, args: &Value) -> Result<Value, String> {
        let name = args.get("name")
            .and_then(|v| v.as_str())
            .ok_or("缺少参数 name")?;

        let obj = self.0.iter()
            .find(|o| o.name == name)
            .ok_or_else(|| format!("未找到对象「{name}」"))?;

        Ok(serde_json::json!({
            "name": obj.name,
            "kind": obj.kind.label(),
            "description": obj.description,
            "background": obj.background,
            "links": obj.links.iter().map(|l| serde_json::json!({
                "target": l.target.display_name(),
                "relation": l.kind.label(),
                "note": l.note
            })).collect::<Vec<_>>()
        }))
    }
}

/// Return the chapter / structure outline as a nested JSON tree.
pub struct GetChapterOutlineSkill(pub Vec<StructNode>);

impl Skill for GetChapterOutlineSkill {
    fn name(&self) -> &str { "get_chapter_outline" }

    fn description(&self) -> &str {
        "获取小说章节结构大纲，包含标题、层级类型、摘要和完成状态"
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({ "type": "object", "properties": {} })
    }

    fn execute(&self, _args: &Value) -> Result<Value, String> {
        fn walk(nodes: &[StructNode]) -> Vec<Value> {
            nodes.iter().map(|n| {
                let mut entry = serde_json::json!({
                    "title":   n.title,
                    "kind":    n.kind.label(),
                    "done":    n.done,
                    "summary": n.summary,
                    "tags":    n.linked_objects
                });
                if !n.children.is_empty() {
                    entry["children"] = Value::Array(walk(&n.children));
                }
                entry
            }).collect()
        }
        Ok(Value::Array(walk(&self.0)))
    }
}

/// Search through foreshadows (plot seeds) by keyword.
pub struct SearchForeshadowsSkill(pub Vec<Foreshadow>);

impl Skill for SearchForeshadowsSkill {
    fn name(&self) -> &str { "search_foreshadows" }

    fn description(&self) -> &str {
        "搜索伏笔/伏线列表；query 为空时返回全部伏笔"
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "搜索关键词（可选，为空时返回全部伏笔）"
                }
            }
        })
    }

    fn execute(&self, args: &Value) -> Result<Value, String> {
        let q = args.get("query")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_lowercase();

        let results: Vec<Value> = self.0.iter()
            .filter(|f| {
                q.is_empty()
                    || f.name.to_lowercase().contains(&q)
                    || f.description.to_lowercase().contains(&q)
                    || f.related_chapters.iter().any(|c| c.to_lowercase().contains(&q))
            })
            .map(|f| serde_json::json!({
                "name":             f.name,
                "description":      f.description,
                "resolved":         f.resolved,
                "related_chapters": f.related_chapters
            }))
            .collect();

        Ok(Value::Array(results))
    }
}

// ── GetMilestoneStatusSkill ───────────────────────────────────────────────────

/// List all project milestones with their completion status.
pub struct GetMilestoneStatusSkill(pub Vec<Milestone>);

impl Skill for GetMilestoneStatusSkill {
    fn name(&self) -> &str { "get_milestone_status" }

    fn description(&self) -> &str {
        "获取小说项目的所有里程碑及其完成状态"
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({ "type": "object", "properties": {} })
    }

    fn execute(&self, _args: &Value) -> Result<Value, String> {
        let list: Vec<Value> = self.0.iter().map(|m| {
            serde_json::json!({
                "name":        m.name,
                "description": m.description,
                "completed":   m.completed,
            })
        }).collect();
        Ok(Value::Array(list))
    }
}

// ── ListProjectFilesSkill ─────────────────────────────────────────────────────

/// List all `.md` and `.json` files in the project directory.
pub struct ListProjectFilesSkill(pub Option<std::path::PathBuf>);

impl Skill for ListProjectFilesSkill {
    fn name(&self) -> &str { "list_project_files" }

    fn description(&self) -> &str {
        "列出项目中的所有 Markdown（.md/.markdown）、JSON 和 TXT 文件（相对路径）"
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({ "type": "object", "properties": {} })
    }

    fn execute(&self, _args: &Value) -> Result<Value, String> {
        let root = self.0.as_ref().ok_or("项目未打开")?;
        let mut files = Vec::new();
        collect_text_files(root, root, &mut files);
        Ok(Value::Array(files.into_iter().map(|s| Value::String(s)).collect()))
    }
}

fn collect_text_files(root: &std::path::Path, dir: &std::path::Path, out: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    let mut sorted: Vec<_> = entries.filter_map(|e| e.ok()).collect();
    sorted.sort_by_key(|e| e.file_name());
    for entry in sorted {
        let path = entry.path();
        if path.is_dir() {
            collect_text_files(root, &path, out);
        } else if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            if matches!(ext, "md" | "markdown" | "json") {
                if let Ok(rel) = path.strip_prefix(root) {
                    out.push(rel.to_string_lossy().into_owned());
                }
            }
        }
    }
}

// ── GetFileContentSkill ───────────────────────────────────────────────────────

/// Read the full content of a project file by its relative path.
pub struct GetFileContentSkill(pub Option<std::path::PathBuf>);

impl Skill for GetFileContentSkill {
    fn name(&self) -> &str { "get_file_content" }

    fn description(&self) -> &str {
        "读取项目中指定文件的完整内容；path 为相对于项目根目录的路径（如 Content/第一章.md）；支持 .md / .markdown / .json / .txt 文件"
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "相对于项目根目录的文件路径"
                }
            },
            "required": ["path"]
        })
    }

    fn execute(&self, args: &Value) -> Result<Value, String> {
        let rel = args.get("path")
            .and_then(|v| v.as_str())
            .ok_or("缺少参数 path")?;

        let root = self.0.as_ref().ok_or("项目未打开")?;

        // Prevent directory traversal outside the project root.
        let candidate = root.join(rel);
        let canonical_root = std::fs::canonicalize(root)
            .map_err(|e| format!("无法解析项目路径: {e}"))?;
        let canonical_file = std::fs::canonicalize(&candidate)
            .map_err(|e| format!("文件不存在或无法访问: {e}"))?;
        if !canonical_file.starts_with(&canonical_root) {
            return Err("拒绝访问项目目录之外的文件".to_owned());
        }

        // Only allow text files.
        let ext = candidate.extension().and_then(|e| e.to_str()).unwrap_or("");
        if !matches!(ext, "md" | "markdown" | "json" | "txt") {
            return Err("仅支持读取 .md / .json / .txt 文件".to_owned());
        }

        let content = std::fs::read_to_string(&canonical_file)
            .map_err(|e| format!("读取失败: {e}"))?;

        Ok(serde_json::json!({
            "path":    rel,
            "content": content,
            "length":  content.chars().count(),
        }))
    }
}

// ── SkillSet ──────────────────────────────────────────────────────────────────

/// A collection of skills made available to the agent.
pub struct SkillSet(Vec<Arc<dyn Skill>>);

impl SkillSet {
    /// Build the default skill set from a snapshot of the current app data.
    pub fn new(
        objects:      Vec<WorldObject>,
        struct_roots: Vec<StructNode>,
        foreshadows:  Vec<Foreshadow>,
        milestones:   Vec<Milestone>,
        project_root: Option<std::path::PathBuf>,
    ) -> Self {
        SkillSet(vec![
            Arc::new(ListCharactersSkill(objects.clone())),
            Arc::new(GetCharacterInfoSkill(objects.clone())),
            Arc::new(GetChapterOutlineSkill(struct_roots)),
            Arc::new(SearchForeshadowsSkill(foreshadows)),
            Arc::new(GetMilestoneStatusSkill(milestones)),
            Arc::new(ListProjectFilesSkill(project_root.clone())),
            Arc::new(GetFileContentSkill(project_root)),
        ])
    }

    /// Number of registered skills.
    #[allow(dead_code)]
    pub fn len(&self) -> usize { self.0.len() }

    /// Returns `true` when no skills are registered.
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool { self.0.is_empty() }

    /// Names of all registered skills (used for auto-generating system prompts).
    pub fn tool_names(&self) -> Vec<&str> {
        self.0.iter().map(|s| s.name()).collect()
    }

    /// Names and descriptions of all skills (for displaying in the UI).
    #[allow(dead_code)]
    pub fn descriptions(&self) -> Vec<(String, String)> {
        self.0.iter()
            .map(|s| (s.name().to_owned(), s.description().to_owned()))
            .collect()
    }

    /// Serialise into the OpenAI `tools` array.
    pub fn to_openai_tools(&self) -> Value {
        Value::Array(self.0.iter().map(|s| s.to_openai_tool()).collect())
    }

    /// Execute the named skill with the given JSON arguments.
    pub fn execute(&self, name: &str, args: &Value) -> Result<Value, String> {
        self.0.iter()
            .find(|s| s.name() == name)
            .ok_or_else(|| format!("未知技能: {name}"))?
            .execute(args)
    }
}

// ── AgentBackend ──────────────────────────────────────────────────────────────

/// Maximum number of LLM ↔ tool-call roundtrips before giving up.
const MAX_ROUNDS: usize = 5;

/// An LLM backend that uses the **OpenAI tool-calling / function-calling API**
/// to run an agent loop.
///
/// On each round the backend:
///  1. Sends the accumulated conversation + `tools` list to the LLM.
///  2. If the model returns `tool_calls`, executes each locally and appends
///     the results, then goes to step 1.
///  3. Once the model returns a plain text response, returns it (prefixed by
///     a human-readable tool-call log so the user can see what the agent did).
///
/// **Requires an OpenAI-compatible endpoint that supports function/tool calling.**
/// Endpoints that don't support tools (e.g. plain llama.cpp without a
/// function-calling model) will still work — they simply return content
/// without `tool_calls`, so the loop terminates after one round.
pub struct AgentBackend {
    pub skills: SkillSet,
}

impl AgentBackend {
    /// Static backend name — allows callers to read the name without
    /// constructing a full `AgentBackend` (which requires a `SkillSet`).
    pub const BACKEND_NAME: &'static str = "Agent (工具调用)";
}

impl LlmBackend for AgentBackend {
    fn name(&self) -> &'static str { Self::BACKEND_NAME }

    fn complete(&self, config: &LlmConfig, prompt: &str) -> Result<String, String> {
        let model = {
            let p = config.model_path.trim();
            if p.is_empty() { "gpt-4o" } else { p }
        };

        // ── Build initial message list ─────────────────────────────────────────
        let mut messages: Vec<Value> = Vec::new();

        // Use user-provided system prompt if present; otherwise inject a default
        // one that describes the agent's purpose and available tools.
        let sys_text = if config.system_prompt.trim().is_empty() {
            let tool_names = self.skills.tool_names().join("、");
            format!(
                "你是一个专业的小说写作助手，正在协助作者完善小说项目。\
                 你可以通过以下工具查阅项目数据：{}。\
                 请优先使用工具获取最新数据，再给出回答。\
                 始终用中文回复。",
                tool_names
            )
        } else {
            config.system_prompt.trim().to_owned()
        };
        messages.push(serde_json::json!({ "role": "system", "content": sys_text }));
        messages.push(serde_json::json!({ "role": "user", "content": prompt }));

        let tools = self.skills.to_openai_tools();
        let mut agent_log = String::new();

        // ── Agent loop ────────────────────────────────────────────────────────
        for round in 0..MAX_ROUNDS {
            let body = serde_json::json!({
                "model":       model,
                "messages":    messages,
                "tools":       tools,
                "tool_choice": "auto",
                "temperature": config.temperature,
                "max_tokens":  config.max_tokens,
            });

            let mut response = ureq::post(&config.api_url)
                .send_json(&body)
                .map_err(|e| format!("请求失败 ({}): {e}", config.api_url))?;

            let json: serde_json::Value = response
                .body_mut()
                .read_json()
                .map_err(|e| format!("响应解析失败: {e}"))?;

            // Surface API-level errors (e.g. auth failure, model not found).
            if let Some(err_obj) = json.get("error") {
                let msg = err_obj.get("message").and_then(|v| v.as_str())
                    .unwrap_or("未知错误");
                return Err(format!("API 错误: {msg} (原始响应: {err_obj})"));
            }

            let message = json
                .get("choices").and_then(|v| v.get(0))
                .and_then(|v| v.get("message"))
                .ok_or_else(|| format!("无法解析 LLM 响应 (轮次 {}/{MAX_ROUNDS}): {json}", round + 1))?;

            // ── Handle tool calls ──────────────────────────────────────────────
            if let Some(calls) = message.get("tool_calls").and_then(|v| v.as_array()) {
                if calls.is_empty() { break; }

                // Append the assistant message (with its tool_calls) to history.
                messages.push(message.clone());

                for call in calls {
                    let call_id  = call.get("id").and_then(|v| v.as_str()).unwrap_or("?");
                    let fn_name  = call.get("function")
                        .and_then(|v| v.get("name")).and_then(|v| v.as_str())
                        .unwrap_or("");
                    let args_str = call.get("function")
                        .and_then(|v| v.get("arguments")).and_then(|v| v.as_str())
                        .unwrap_or("{}");

                    let args: Value = serde_json::from_str(args_str)
                        .unwrap_or(Value::Object(Default::default()));

                    let result   = self.skills.execute(fn_name, &args);
                    let result_s = match &result {
                        Ok(v)  => serde_json::to_string_pretty(v).unwrap_or_else(|_| v.to_string()),
                        Err(e) => format!("{{\"error\": \"{e}\"}}"),
                    };

                    // Record tool invocation for user visibility.
                    agent_log.push_str(&format!(
                        "[技能调用 {}/{}] {}({})\n{}\n\n",
                        round + 1, MAX_ROUNDS, fn_name, args_str, result_s
                    ));

                    messages.push(serde_json::json!({
                        "role":         "tool",
                        "tool_call_id": call_id,
                        "content":      result_s
                    }));
                }

                if round + 1 == MAX_ROUNDS {
                    return Err(format!(
                        "Agent 达到最大轮次限制 ({MAX_ROUNDS})，请精简请求或减少所需技能数量"
                    ));
                }
                // Continue to next round.
            } else {
                // No tool calls — return the final answer.
                let content = message.get("content")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_owned();

                if content.trim().is_empty() {
                    return Err("Agent 返回了空响应，请检查模型配置或简化请求".to_owned());
                }

                return if agent_log.is_empty() {
                    Ok(content)
                } else {
                    Ok(format!("{agent_log}---\n{content}"))
                };
            }
        }

        Err("Agent 未能生成最终回复".to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{ObjectKind, StructKind, ObjectLink, LinkTarget, RelationKind};

    fn sample_objects() -> Vec<WorldObject> {
        let mut ch = WorldObject::new("李明", ObjectKind::Character);
        ch.description = "冷静理性".to_owned();
        ch.background  = "前任侦探".to_owned();
        ch.links.push(ObjectLink {
            target: LinkTarget::Object("陈薇".to_owned()),
            kind:   RelationKind::Friend,
            note:   String::new(),
        });
        let loc = WorldObject::new("旧警局", ObjectKind::Location);
        vec![ch, loc]
    }

    fn sample_roots() -> Vec<StructNode> {
        let mut vol = StructNode::new("第一卷", StructKind::Volume);
        let mut ch1 = StructNode::new("第一章", StructKind::Chapter);
        ch1.summary = "主角登场".to_owned();
        ch1.done = true;
        vol.children.push(ch1);
        vol.children.push(StructNode::new("第二章", StructKind::Chapter));
        vec![vol]
    }

    fn sample_foreshadows() -> Vec<Foreshadow> {
        let mut f = Foreshadow::new("神秘信封");
        f.description = "第一章出现的信封".to_owned();
        f.related_chapters = vec!["第一章".to_owned(), "第三章".to_owned()];
        vec![f, Foreshadow::new("断剑")]
    }

    fn sample_milestones() -> Vec<Milestone> {
        let mut m1 = Milestone::new("完成第一章");
        m1.completed = true;
        m1.description = "第一章草稿".to_owned();
        let m2 = Milestone::new("完成第二章");
        vec![m1, m2]
    }

    fn make_skill_set() -> SkillSet {
        SkillSet::new(sample_objects(), sample_roots(), sample_foreshadows(),
                      sample_milestones(), None)
    }

    // ── Skill: list_characters ─────────────────────────────────────────────────

    #[test]
    fn test_list_characters_skill() {
        let skill = ListCharactersSkill(sample_objects());
        let result = skill.execute(&serde_json::json!({})).unwrap();
        let arr = result.as_array().unwrap();
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[0]["name"], "李明");
        assert_eq!(arr[0]["kind"], "人物");
        assert_eq!(arr[1]["name"], "旧警局");
        assert_eq!(arr[1]["kind"], "地点");
    }

    // ── Skill: get_character_info ─────────────────────────────────────────────

    #[test]
    fn test_get_character_info_found() {
        let skill = GetCharacterInfoSkill(sample_objects());
        let result = skill.execute(&serde_json::json!({"name": "李明"})).unwrap();
        assert_eq!(result["name"], "李明");
        assert_eq!(result["description"], "冷静理性");
        assert_eq!(result["background"], "前任侦探");
        let links = result["links"].as_array().unwrap();
        assert_eq!(links.len(), 1);
        assert_eq!(links[0]["target"], "陈薇");
        assert_eq!(links[0]["relation"], "友好");
    }

    #[test]
    fn test_get_character_info_not_found() {
        let skill = GetCharacterInfoSkill(sample_objects());
        let result = skill.execute(&serde_json::json!({"name": "张三"}));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("未找到"));
    }

    #[test]
    fn test_get_character_info_missing_param() {
        let skill = GetCharacterInfoSkill(sample_objects());
        let result = skill.execute(&serde_json::json!({}));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("缺少参数"));
    }

    // ── Skill: get_chapter_outline ─────────────────────────────────────────────

    #[test]
    fn test_get_chapter_outline_skill() {
        let skill = GetChapterOutlineSkill(sample_roots());
        let result = skill.execute(&serde_json::json!({})).unwrap();
        let arr = result.as_array().unwrap();
        assert_eq!(arr.len(), 1);                       // one volume
        assert_eq!(arr[0]["title"], "第一卷");
        let children = arr[0]["children"].as_array().unwrap();
        assert_eq!(children.len(), 2);
        assert_eq!(children[0]["title"], "第一章");
        assert_eq!(children[0]["done"], true);
        assert_eq!(children[0]["summary"], "主角登场");
        assert_eq!(children[1]["done"], false);
    }

    // ── Skill: search_foreshadows ─────────────────────────────────────────────

    #[test]
    fn test_search_foreshadows_all() {
        let skill = SearchForeshadowsSkill(sample_foreshadows());
        let result = skill.execute(&serde_json::json!({})).unwrap();
        assert_eq!(result.as_array().unwrap().len(), 2);
    }

    #[test]
    fn test_search_foreshadows_query() {
        let skill = SearchForeshadowsSkill(sample_foreshadows());
        let result = skill.execute(&serde_json::json!({"query": "信封"})).unwrap();
        let arr = result.as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["name"], "神秘信封");
    }

    #[test]
    fn test_search_foreshadows_no_match() {
        let skill = SearchForeshadowsSkill(sample_foreshadows());
        let result = skill.execute(&serde_json::json!({"query": "xyz999"})).unwrap();
        assert_eq!(result.as_array().unwrap().len(), 0);
    }

    // ── SkillSet ───────────────────────────────────────────────────────────────

    #[test]
    fn test_skill_set_len() {
        let ss = make_skill_set();
        assert_eq!(ss.len(), 7);
    }

    #[test]
    fn test_skill_set_execute_known() {
        let ss = make_skill_set();
        let r = ss.execute("list_characters", &serde_json::json!({})).unwrap();
        assert_eq!(r.as_array().unwrap().len(), 2);
    }

    #[test]
    fn test_skill_set_execute_unknown() {
        let ss = make_skill_set();
        let r = ss.execute("nonexistent_skill", &serde_json::json!({}));
        assert!(r.is_err());
        assert!(r.unwrap_err().contains("未知技能"));
    }

    #[test]
    fn test_skill_to_openai_tool_shape() {
        let skill = ListCharactersSkill(vec![]);
        let tool = skill.to_openai_tool();
        assert_eq!(tool["type"], "function");
        assert!(tool["function"]["name"].is_string());
        assert!(tool["function"]["description"].is_string());
        assert!(tool["function"]["parameters"].is_object());
    }

    #[test]
    fn test_skill_set_to_openai_tools() {
        let ss = SkillSet::new(vec![], vec![], vec![], vec![], None);
        let tools = ss.to_openai_tools();
        assert_eq!(tools.as_array().unwrap().len(), 7);
    }

    #[test]
    fn test_skill_set_tool_names() {
        let ss = SkillSet::new(vec![], vec![], vec![], vec![], None);
        let names = ss.tool_names();
        assert_eq!(names.len(), 7);
        assert!(names.contains(&"list_characters"));
        assert!(names.contains(&"get_character_info"));
        assert!(names.contains(&"get_chapter_outline"));
        assert!(names.contains(&"search_foreshadows"));
        assert!(names.contains(&"get_milestone_status"));
        assert!(names.contains(&"list_project_files"));
        assert!(names.contains(&"get_file_content"));
    }

    #[test]
    fn test_skill_descriptions() {
        let ss = SkillSet::new(vec![], vec![], vec![], vec![], None);
        let descs = ss.descriptions();
        assert_eq!(descs.len(), 7);
        let names: Vec<String> = descs.iter().map(|(n, _)| n.clone()).collect();
        assert!(names.contains(&"list_characters".to_owned()));
        assert!(names.contains(&"get_character_info".to_owned()));
        assert!(names.contains(&"get_chapter_outline".to_owned()));
        assert!(names.contains(&"search_foreshadows".to_owned()));
        assert!(names.contains(&"get_milestone_status".to_owned()));
        assert!(names.contains(&"list_project_files".to_owned()));
        assert!(names.contains(&"get_file_content".to_owned()));
    }

    // ── Skill: get_milestone_status ───────────────────────────────────────────

    #[test]
    fn test_get_milestone_status_skill() {
        let skill = GetMilestoneStatusSkill(sample_milestones());
        let result = skill.execute(&serde_json::json!({})).unwrap();
        let arr = result.as_array().unwrap();
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[0]["name"], "完成第一章");
        assert_eq!(arr[0]["completed"], true);
        assert_eq!(arr[1]["name"], "完成第二章");
        assert_eq!(arr[1]["completed"], false);
    }

    // ── Skill: list_project_files ─────────────────────────────────────────────

    #[test]
    fn test_list_project_files_no_project() {
        let skill = ListProjectFilesSkill(None);
        let result = skill.execute(&serde_json::json!({}));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("项目未打开"));
    }

    #[test]
    fn test_list_project_files_with_project() {
        let dir = std::env::temp_dir().join("qingmo_test_list_files");
        std::fs::create_dir_all(dir.join("Content")).unwrap();
        std::fs::write(dir.join("Content").join("ch1.md"), "hello").unwrap();
        std::fs::write(dir.join("Content").join("data.json"), "{}").unwrap();

        let skill = ListProjectFilesSkill(Some(dir.clone()));
        let result = skill.execute(&serde_json::json!({})).unwrap();
        let arr = result.as_array().unwrap();
        let names: Vec<&str> = arr.iter().filter_map(|v| v.as_str()).collect();
        assert!(names.iter().any(|s| s.contains("ch1.md")));
        assert!(names.iter().any(|s| s.contains("data.json")));

        let _ = std::fs::remove_dir_all(&dir);
    }

    // ── Skill: get_file_content ───────────────────────────────────────────────

    #[test]
    fn test_get_file_content_no_project() {
        let skill = GetFileContentSkill(None);
        let result = skill.execute(&serde_json::json!({"path": "Content/test.md"}));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("项目未打开"));
    }

    #[test]
    fn test_get_file_content_reads_file() {
        let dir = std::env::temp_dir().join("qingmo_test_get_content");
        std::fs::create_dir_all(dir.join("Content")).unwrap();
        std::fs::write(dir.join("Content").join("story.md"), "# 故事\n\n正文内容").unwrap();

        let skill = GetFileContentSkill(Some(dir.clone()));
        let result = skill.execute(&serde_json::json!({"path": "Content/story.md"})).unwrap();
        assert_eq!(result["path"], "Content/story.md");
        assert!(result["content"].as_str().unwrap().contains("正文内容"));
        assert!(result["length"].as_u64().unwrap() > 0);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_get_file_content_missing_param() {
        let skill = GetFileContentSkill(Some(std::env::temp_dir()));
        let result = skill.execute(&serde_json::json!({}));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("缺少参数"));
    }

    #[test]
    fn test_get_file_content_rejects_path_traversal() {
        let dir = std::env::temp_dir().join("qingmo_test_traversal");
        std::fs::create_dir_all(&dir).unwrap();

        let skill = GetFileContentSkill(Some(dir.clone()));
        let result = skill.execute(&serde_json::json!({"path": "../../etc/passwd"}));
        // Should fail – either "file not found" or "directory traversal" rejection
        assert!(result.is_err());

        let _ = std::fs::remove_dir_all(&dir);
    }

    // ── AgentBackend ──────────────────────────────────────────────────────────

    #[test]
    fn test_agent_backend_name() {
        let agent = AgentBackend {
            skills: SkillSet::new(vec![], vec![], vec![], vec![], None),
        };
        assert_eq!(agent.name(), AgentBackend::BACKEND_NAME);
        assert_eq!(agent.name(), "Agent (工具调用)");
    }

    #[test]
    fn test_agent_backend_name_const() {
        // BACKEND_NAME must not contain the broken emoji (🤖)
        assert!(!AgentBackend::BACKEND_NAME.contains('\u{1F916}'));
        assert!(AgentBackend::BACKEND_NAME.contains("Agent"));
    }
}
