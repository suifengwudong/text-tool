use std::sync::Arc;
use serde_json::Value;

use super::llm_backend::LlmBackend;
use super::{LlmConfig, WorldObject, StructNode, Foreshadow};

// ── Skill trait ───────────────────────────────────────────────────────────────

/// A discrete capability (tool) that the LLM agent can invoke.
///
/// Each skill is:
/// - Described to the LLM via `name()`, `description()`, and `parameters_schema()`
///   (OpenAI function-calling JSON Schema).
/// - Executed synchronously in the background thread via `execute(&args)`.
/// - Safe to share across threads (`Send + Sync`).
pub trait Skill: Send + Sync {
    #[allow(dead_code)]
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

// ── SkillSet ──────────────────────────────────────────────────────────────────

/// A collection of skills made available to the agent.
pub struct SkillSet(Vec<Arc<dyn Skill>>);

impl SkillSet {
    /// Build the default skill set from a snapshot of the current app data.
    pub fn new(
        objects:     Vec<WorldObject>,
        struct_roots: Vec<StructNode>,
        foreshadows: Vec<Foreshadow>,
    ) -> Self {
        SkillSet(vec![
            Arc::new(ListCharactersSkill(objects.clone())),
            Arc::new(GetCharacterInfoSkill(objects.clone())),
            Arc::new(GetChapterOutlineSkill(struct_roots)),
            Arc::new(SearchForeshadowsSkill(foreshadows)),
        ])
    }

    /// Number of registered skills.
    #[allow(dead_code)]
    pub fn len(&self) -> usize { self.0.len() }

    /// Returns `true` when no skills are registered.
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool { self.0.is_empty() }

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

impl LlmBackend for AgentBackend {
    fn name(&self) -> &'static str { "🤖 Agent (工具调用)" }

    fn complete(&self, config: &LlmConfig, prompt: &str) -> Result<String, String> {
        let model = {
            let p = config.model_path.trim();
            if p.is_empty() { "gpt-4o" } else { p }
        };

        // ── Build initial message list ─────────────────────────────────────────
        let mut messages: Vec<Value> = Vec::new();
        if !config.system_prompt.trim().is_empty() {
            messages.push(serde_json::json!({
                "role": "system",
                "content": config.system_prompt
            }));
        }
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

            let message = json
                .get("choices").and_then(|v| v.get(0))
                .and_then(|v| v.get("message"))
                .ok_or_else(|| format!("无法解析 LLM 响应: {json}"))?;

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
                        "[技能调用] {}({})\n{}\n\n",
                        fn_name, args_str, result_s
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

// ── Tests ─────────────────────────────────────────────────────────────────────

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
        let ss = SkillSet::new(sample_objects(), sample_roots(), sample_foreshadows());
        assert_eq!(ss.len(), 4);
    }

    #[test]
    fn test_skill_set_execute_known() {
        let ss = SkillSet::new(sample_objects(), sample_roots(), sample_foreshadows());
        let r = ss.execute("list_characters", &serde_json::json!({})).unwrap();
        assert_eq!(r.as_array().unwrap().len(), 2);
    }

    #[test]
    fn test_skill_set_execute_unknown() {
        let ss = SkillSet::new(sample_objects(), sample_roots(), sample_foreshadows());
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
        let ss = SkillSet::new(vec![], vec![], vec![]);
        let tools = ss.to_openai_tools();
        assert_eq!(tools.as_array().unwrap().len(), 4);
    }

    #[test]
    fn test_skill_descriptions() {
        let ss = SkillSet::new(vec![], vec![], vec![]);
        let descs = ss.descriptions();
        assert_eq!(descs.len(), 4);
        let names: Vec<String> = descs.iter().map(|(n, _)| n.clone()).collect();
        assert!(names.contains(&"list_characters".to_owned()));
        assert!(names.contains(&"get_character_info".to_owned()));
        assert!(names.contains(&"get_chapter_outline".to_owned()));
        assert!(names.contains(&"search_foreshadows".to_owned()));
    }

    // ── AgentBackend ──────────────────────────────────────────────────────────

    #[test]
    fn test_agent_backend_name() {
        let agent = AgentBackend {
            skills: SkillSet::new(vec![], vec![], vec![]),
        };
        assert_eq!(agent.name(), "🤖 Agent (工具调用)");
    }
}
