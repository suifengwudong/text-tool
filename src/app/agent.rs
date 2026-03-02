use std::sync::Arc;
use serde_json::Value;

use super::llm_backend::LlmBackend;
use super::{LlmConfig, WorldObject, StructNode, Foreshadow, Milestone, ObjectKind,
            StructKind, ChapterTag};

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

// ── AddWorldObjectSkill ───────────────────────────────────────────────────────

/// Add a new world object to the project JSON file.
pub struct AddWorldObjectSkill {
    pub objects: Vec<WorldObject>,
    pub project_root: Option<std::path::PathBuf>,
}

impl Skill for AddWorldObjectSkill {
    fn name(&self) -> &str { "add_world_object" }

    fn description(&self) -> &str {
        "向项目添加新的世界对象（人物/场景/地点/道具/势力）并保存到 Design/世界对象.json；\
         name 为名称，kind 为类型（人物/场景/地点/道具/势力/其他），description 为描述（可选），background 为背景故事（可选）"
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "name":        { "type": "string", "description": "对象名称" },
                "kind":        { "type": "string", "description": "类型：人物/场景/地点/道具/势力/其他" },
                "description": { "type": "string", "description": "描述（可选）" },
                "background":  { "type": "string", "description": "背景故事（可选）" }
            },
            "required": ["name", "kind"]
        })
    }

    fn execute(&self, args: &Value) -> Result<Value, String> {
        let name = args.get("name").and_then(|v| v.as_str()).ok_or("缺少参数 name")?;
        let kind_str = args.get("kind").and_then(|v| v.as_str()).ok_or("缺少参数 kind")?;
        let description = args.get("description").and_then(|v| v.as_str()).unwrap_or("").to_owned();
        let background  = args.get("background").and_then(|v| v.as_str()).unwrap_or("").to_owned();

        if self.objects.iter().any(|o| o.name == name) {
            return Err(format!("对象「{name}」已存在，请使用 update_world_object 修改"));
        }

        let kind = match kind_str {
            "人物" => ObjectKind::Character,
            "场景" => ObjectKind::Scene,
            "地点" => ObjectKind::Location,
            "道具" => ObjectKind::Item,
            "势力" => ObjectKind::Faction,
            _      => ObjectKind::Other,
        };

        let root = self.project_root.as_ref().ok_or("项目未打开")?;
        let mut objects = self.objects.clone();
        let mut new_obj = WorldObject::new(name, kind);
        new_obj.description = description;
        new_obj.background  = background;
        objects.push(new_obj);

        let json = serde_json::to_string_pretty(&objects)
            .map_err(|e| format!("序列化失败: {e}"))?;
        let path = root.join("Design").join("世界对象.json");
        std::fs::create_dir_all(path.parent().unwrap())
            .map_err(|e| format!("创建目录失败: {e}"))?;
        std::fs::write(&path, &json)
            .map_err(|e| format!("写入失败: {e}"))?;

        Ok(serde_json::json!({
            "status": "success",
            "message": format!("已添加对象「{name}」（{kind_str}）到 Design/世界对象.json，请在主界面「从文件加载」以更新显示")
        }))
    }
}

// ── UpdateWorldObjectSkill ────────────────────────────────────────────────────

/// Update the description and/or background of an existing world object.
pub struct UpdateWorldObjectSkill {
    pub objects: Vec<WorldObject>,
    pub project_root: Option<std::path::PathBuf>,
}

impl Skill for UpdateWorldObjectSkill {
    fn name(&self) -> &str { "update_world_object" }

    fn description(&self) -> &str {
        "更新已有世界对象的描述（description）或背景故事（background）并保存到 Design/世界对象.json；\
         name 为要修改的对象名称（必填）"
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "name":        { "type": "string", "description": "要修改的对象名称" },
                "description": { "type": "string", "description": "新的描述（可选，省略则不修改）" },
                "background":  { "type": "string", "description": "新的背景故事（可选，省略则不修改）" }
            },
            "required": ["name"]
        })
    }

    fn execute(&self, args: &Value) -> Result<Value, String> {
        let name = args.get("name").and_then(|v| v.as_str()).ok_or("缺少参数 name")?;

        let mut objects = self.objects.clone();
        let obj = objects.iter_mut()
            .find(|o| o.name == name)
            .ok_or_else(|| format!("未找到对象「{name}」，请先用 add_world_object 添加"))?;

        if let Some(desc) = args.get("description").and_then(|v| v.as_str()) {
            obj.description = desc.to_owned();
        }
        if let Some(bg) = args.get("background").and_then(|v| v.as_str()) {
            obj.background = bg.to_owned();
        }

        let root = self.project_root.as_ref().ok_or("项目未打开")?;
        let json = serde_json::to_string_pretty(&objects)
            .map_err(|e| format!("序列化失败: {e}"))?;
        let path = root.join("Design").join("世界对象.json");
        std::fs::create_dir_all(path.parent().unwrap())
            .map_err(|e| format!("创建目录失败: {e}"))?;
        std::fs::write(&path, &json)
            .map_err(|e| format!("写入失败: {e}"))?;

        Ok(serde_json::json!({
            "status": "success",
            "message": format!("已更新对象「{name}」并保存到 Design/世界对象.json，请在主界面「从文件加载」以更新显示")
        }))
    }
}

// ── DeleteWorldObjectSkill ────────────────────────────────────────────────────

/// Delete a world object by name from the project JSON file.
pub struct DeleteWorldObjectSkill {
    pub objects: Vec<WorldObject>,
    pub project_root: Option<std::path::PathBuf>,
}

impl Skill for DeleteWorldObjectSkill {
    fn name(&self) -> &str { "delete_world_object" }

    fn description(&self) -> &str {
        "从项目中删除指定名称的世界对象，并保存到 Design/世界对象.json；name 为要删除的对象名称"
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "name": { "type": "string", "description": "要删除的对象名称" }
            },
            "required": ["name"]
        })
    }

    fn execute(&self, args: &Value) -> Result<Value, String> {
        let name = args.get("name").and_then(|v| v.as_str()).ok_or("缺少参数 name")?;

        let mut objects = self.objects.clone();
        let before = objects.len();
        objects.retain(|o| o.name != name);
        if objects.len() == before {
            return Err(format!("未找到对象「{name}」"));
        }

        let root = self.project_root.as_ref().ok_or("项目未打开")?;
        let json = serde_json::to_string_pretty(&objects)
            .map_err(|e| format!("序列化失败: {e}"))?;
        let path = root.join("Design").join("世界对象.json");
        std::fs::write(&path, &json)
            .map_err(|e| format!("写入失败: {e}"))?;

        Ok(serde_json::json!({
            "status": "success",
            "message": format!("已删除对象「{name}」并保存到 Design/世界对象.json，请在主界面「从文件加载」以更新显示")
        }))
    }
}

// ── AddChapterNodeSkill ───────────────────────────────────────────────────────

/// Add a new top-level chapter node to the chapter structure JSON.
pub struct AddChapterNodeSkill {
    pub struct_roots: Vec<StructNode>,
    pub project_root: Option<std::path::PathBuf>,
}

impl Skill for AddChapterNodeSkill {
    fn name(&self) -> &str { "add_chapter_node" }

    fn description(&self) -> &str {
        "向章节结构添加新节点（总纲/卷/章/节）并保存到 Design/章节结构.json；\
         title 为节点标题，kind 为层级类型（总纲/卷/章/节），summary 为摘要（可选）"
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "title":   { "type": "string", "description": "节点标题" },
                "kind":    { "type": "string", "description": "层级类型：总纲/卷/章/节" },
                "summary": { "type": "string", "description": "节点摘要（可选）" }
            },
            "required": ["title", "kind"]
        })
    }

    fn execute(&self, args: &Value) -> Result<Value, String> {
        let title = args.get("title").and_then(|v| v.as_str()).ok_or("缺少参数 title")?;
        let kind_str = args.get("kind").and_then(|v| v.as_str()).ok_or("缺少参数 kind")?;
        let summary = args.get("summary").and_then(|v| v.as_str()).unwrap_or("").to_owned();

        let kind = match kind_str {
            "总纲" => StructKind::Outline,
            "卷"   => StructKind::Volume,
            "章"   => StructKind::Chapter,
            _      => StructKind::Section,
        };

        let mut roots = self.struct_roots.clone();
        let mut node = StructNode {
            title:   title.to_owned(),
            kind,
            tag:     ChapterTag::Normal,
            summary,
            done:    false,
            children: vec![],
            linked_objects: vec![],
            node_links: vec![],
        };
        // Set a meaningful summary default if none provided
        if node.summary.is_empty() {
            node.summary = String::new();
        }
        roots.push(node);

        let root = self.project_root.as_ref().ok_or("项目未打开")?;
        let json = serde_json::to_string_pretty(&roots)
            .map_err(|e| format!("序列化失败: {e}"))?;
        let path = root.join("Design").join("章节结构.json");
        std::fs::create_dir_all(path.parent().unwrap())
            .map_err(|e| format!("创建目录失败: {e}"))?;
        std::fs::write(&path, &json)
            .map_err(|e| format!("写入失败: {e}"))?;

        Ok(serde_json::json!({
            "status": "success",
            "message": format!("已添加{kind_str}节点「{title}」到 Design/章节结构.json，请在主界面「从文件加载」以更新显示")
        }))
    }
}

// ── AddForeshadowSkill ────────────────────────────────────────────────────────

/// Add a new foreshadow entry to the project.
pub struct AddForeshadowSkill {
    pub foreshadows: Vec<Foreshadow>,
    pub project_root: Option<std::path::PathBuf>,
}

impl Skill for AddForeshadowSkill {
    fn name(&self) -> &str { "add_foreshadow" }

    fn description(&self) -> &str {
        "向项目添加新伏笔并追加到 Content/伏笔.md；\
         name 为伏笔名称，description 为描述（可选），related_chapters 为关联章节列表（可选，逗号分隔）"
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "name":             { "type": "string", "description": "伏笔名称" },
                "description":      { "type": "string", "description": "伏笔描述（可选）" },
                "related_chapters": { "type": "string", "description": "关联章节，逗号分隔（可选）" }
            },
            "required": ["name"]
        })
    }

    fn execute(&self, args: &Value) -> Result<Value, String> {
        let name = args.get("name").and_then(|v| v.as_str()).ok_or("缺少参数 name")?;
        let description = args.get("description").and_then(|v| v.as_str()).unwrap_or("").to_owned();
        let related_raw = args.get("related_chapters").and_then(|v| v.as_str()).unwrap_or("");
        let related_chapters: Vec<String> = if related_raw.is_empty() {
            vec![]
        } else {
            related_raw.split(',').map(|s| s.trim().to_owned()).filter(|s| !s.is_empty()).collect()
        };

        if self.foreshadows.iter().any(|f| f.name == name) {
            return Err(format!("伏笔「{name}」已存在"));
        }

        let root = self.project_root.as_ref().ok_or("项目未打开")?;
        let path = root.join("Content").join("伏笔.md");

        // Append to existing file (or create new).
        let existing = std::fs::read_to_string(&path).unwrap_or_default();
        let header = if existing.trim().is_empty() { "# 伏笔列表\n\n".to_owned() } else { String::new() };
        let mut entry = format!("## {} ⏳ 未解决\n\n", name);
        if !description.is_empty() {
            entry.push_str(&format!("{}\n\n", description));
        }
        if !related_chapters.is_empty() {
            entry.push_str(&format!("**关联章节**: {}\n\n", related_chapters.join("、")));
        }
        std::fs::create_dir_all(path.parent().unwrap())
            .map_err(|e| format!("创建目录失败: {e}"))?;
        std::fs::write(&path, format!("{}{}{}", existing, header, entry))
            .map_err(|e| format!("写入失败: {e}"))?;

        Ok(serde_json::json!({
            "status": "success",
            "message": format!("已添加伏笔「{name}」到 Content/伏笔.md，请在主界面「从文件加载」以更新显示")
        }))
    }
}

// ── ResolveForeshadowSkill ────────────────────────────────────────────────────

/// Mark an existing foreshadow as resolved in the project Markdown file.
pub struct ResolveForeshadowSkill {
    pub foreshadows: Vec<Foreshadow>,
    pub project_root: Option<std::path::PathBuf>,
}

impl Skill for ResolveForeshadowSkill {
    fn name(&self) -> &str { "resolve_foreshadow" }

    fn description(&self) -> &str {
        "将指定伏笔标记为已解决，并更新 Content/伏笔.md；name 为要解决的伏笔名称"
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "name": { "type": "string", "description": "要标记为已解决的伏笔名称" }
            },
            "required": ["name"]
        })
    }

    fn execute(&self, args: &Value) -> Result<Value, String> {
        let name = args.get("name").and_then(|v| v.as_str()).ok_or("缺少参数 name")?;

        if !self.foreshadows.iter().any(|f| f.name == name) {
            return Err(format!("未找到伏笔「{name}」"));
        }

        let root = self.project_root.as_ref().ok_or("项目未打开")?;
        let path = root.join("Content").join("伏笔.md");
        let content = std::fs::read_to_string(&path)
            .map_err(|e| format!("读取 Content/伏笔.md 失败: {e}"))?;

        // Replace "## {name} ⏳ 未解决" with "## {name} ✅ 已解决"
        let updated = content.replace(
            &format!("## {} ⏳ 未解决", name),
            &format!("## {} ✅ 已解决", name),
        );
        // Also handle case where the foreshadow was added without the status suffix
        let updated = if updated == content {
            content.replace(
                &format!("## {}", name),
                &format!("## {} ✅ 已解决", name),
            )
        } else {
            updated
        };

        std::fs::write(&path, &updated)
            .map_err(|e| format!("写入失败: {e}"))?;

        Ok(serde_json::json!({
            "status": "success",
            "message": format!("已将伏笔「{name}」标记为已解决，请在主界面「从文件加载」以更新显示")
        }))
    }
}

// ── WriteFileContentSkill ─────────────────────────────────────────────────────

/// Write or append text content to a project Markdown file.
pub struct WriteFileContentSkill(pub Option<std::path::PathBuf>);

impl Skill for WriteFileContentSkill {
    fn name(&self) -> &str { "write_file_content" }

    fn description(&self) -> &str {
        "将文本内容写入项目中的 Markdown 文件；path 为相对于项目根目录的路径（如 Content/第一章.md）；\
         content 为要写入的文本；mode 为写入模式：overwrite（覆盖）或 append（追加，默认）"
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path":    { "type": "string", "description": "相对于项目根目录的 .md 文件路径" },
                "content": { "type": "string", "description": "要写入的文本内容" },
                "mode":    { "type": "string", "description": "写入模式：overwrite（覆盖）或 append（追加，默认）" }
            },
            "required": ["path", "content"]
        })
    }

    fn execute(&self, args: &Value) -> Result<Value, String> {
        let rel = args.get("path").and_then(|v| v.as_str()).ok_or("缺少参数 path")?;
        let content = args.get("content").and_then(|v| v.as_str()).ok_or("缺少参数 content")?;
        let mode = args.get("mode").and_then(|v| v.as_str()).unwrap_or("append");

        let root = self.0.as_ref().ok_or("项目未打开")?;

        // Check extension first (cheap, no I/O)
        let candidate = root.join(rel);
        let ext = candidate.extension().and_then(|e| e.to_str()).unwrap_or("");
        if !matches!(ext, "md" | "markdown") {
            return Err("仅支持写入 .md 文件".to_owned());
        }

        // Security: prevent path traversal
        let canonical_root = std::fs::canonicalize(root)
            .map_err(|e| format!("无法解析项目路径: {e}"))?;
        // If the file doesn't exist yet, check the parent directory instead
        let canonical_file = if candidate.exists() {
            std::fs::canonicalize(&candidate)
                .map_err(|e| format!("文件路径解析失败: {e}"))?
        } else {
            let parent = candidate.parent().ok_or("无效路径")?;
            let canonical_parent = std::fs::canonicalize(parent)
                .map_err(|_| "父目录不存在".to_owned())?;
            canonical_parent.join(candidate.file_name().ok_or("无效文件名")?)
        };
        if !canonical_file.starts_with(&canonical_root) {
            return Err("拒绝写入项目目录之外的文件".to_owned());
        }

        std::fs::create_dir_all(canonical_file.parent().unwrap())
            .map_err(|e| format!("创建目录失败: {e}"))?;

        if mode == "overwrite" {
            std::fs::write(&canonical_file, content)
                .map_err(|e| format!("写入失败: {e}"))?;
        } else {
            // append mode
            use std::io::Write;
            let mut file = std::fs::OpenOptions::new()
                .create(true).append(true)
                .open(&canonical_file)
                .map_err(|e| format!("打开文件失败: {e}"))?;
            file.write_all(content.as_bytes())
                .map_err(|e| format!("追加写入失败: {e}"))?;
        }

        Ok(serde_json::json!({
            "status":  "success",
            "path":    rel,
            "mode":    mode,
            "message": format!("已成功以「{mode}」模式写入 {rel}")
        }))
    }
}

// ── GetTextTemplatesSkill ─────────────────────────────────────────────────────

/// Return a catalogue of common novel-writing text templates.
pub struct GetTextTemplatesSkill;

impl Skill for GetTextTemplatesSkill {
    fn name(&self) -> &str { "get_text_templates" }

    fn description(&self) -> &str {
        "返回常见小说写作文本模板列表，包括开场描写、场景转换、心理描写、对话引入等模板，\
         供 LLM 参考用于辅助写作；category 为可选过滤类别"
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "category": {
                    "type": "string",
                    "description": "可选过滤类别：开场/场景/心理/对话/结尾/转折；为空时返回全部"
                }
            }
        })
    }

    fn execute(&self, args: &Value) -> Result<Value, String> {
        let filter = args.get("category").and_then(|v| v.as_str()).unwrap_or("").to_lowercase();

        let all_templates: Vec<(&str, &str, &str)> = vec![
            ("开场", "环境开场",   "【时间/地点】，【环境描写：光线、声音、气味】。【人物出场动作】。"),
            ("开场", "悬念开场",   "【异常事件或诡异现象】，没有人知道，这只是【即将到来的更大事件】的开始。"),
            ("开场", "动作开场",   "【人物姓名】【正在进行的紧张动作】，【简短心理活动或感官描写】。"),
            ("场景", "场景切换",   "【时间过渡词】，场景从【地点A】转移到【地点B】。【新场景的第一印象描写】。"),
            ("场景", "天气渲染",   "【天气状态】笼罩着【地点】，【象征性意象】，仿佛预示着【即将发生的事】。"),
            ("场景", "夜晚氛围",   "夜幕低垂，【地点】沉浸在一片【氛围词：静谧/诡异/温柔】之中。【细节描写】。"),
            ("心理", "内心独白",   "【人物姓名】在心里默默地想：【具体想法或疑问】。这种感觉【延伸描写】。"),
            ("心理", "情感波动",   "【情绪词：愤怒/喜悦/悲伤】如同【比喻】席卷而来，【人物姓名】【相应的行为反应】。"),
            ("心理", "犹豫决断",   "【人物】在【选项A】和【选项B】之间来回权衡。最终，【ta做出的选择】。"),
            ("对话", "冲突对话",   "「【人物A的强硬或质问语气】」\n「【人物B的反应或反驳】」\n两人的目光在空气中碰撞。"),
            ("对话", "柔和对话",   "「【人物A轻声说的话】」\n沉默了片刻，【人物B】才【ta的回应动作或话语】。"),
            ("对话", "揭示秘密",   "「其实……」【人物A】深吸一口气，「【重要秘密或转折信息】」"),
            ("结尾", "章节悬念",   "【概括本章收尾状态】，然而【人物】还不知道，更大的【挑战/危机/惊喜】正在等着ta。"),
            ("结尾", "情感余韵",   "【人物】【动作或离去】，只留下【遗留物或氛围】，和那句话：「【令人回味的台词】」。"),
            ("结尾", "伏笔收束",   "直到此刻，【之前埋下的伏笔】的真相终于浮出水面——【揭示内容】。"),
            ("转折", "反转揭示",   "然而事情并非如此。真相是——【颠覆认知的信息】。"),
            ("转折", "时间跳跃",   "三年后。【新时间点的场景或人物现状描写】。一切都变了，又好像什么都没变。"),
            ("转折", "视角转换",   "如果换一个角度来看——【从对立方或第三方视角重新解读同一事件】。"),
        ];

        let templates: Vec<Value> = all_templates.iter()
            .filter(|(cat, _, _)| filter.is_empty() || cat.contains(filter.as_str()))
            .map(|(cat, name, tmpl)| serde_json::json!({
                "category": cat,
                "name":     name,
                "template": tmpl,
            }))
            .collect();

        Ok(Value::Array(templates))
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
            // ── Read-only skills ──────────────────────────────────────────────
            Arc::new(ListCharactersSkill(objects.clone())),
            Arc::new(GetCharacterInfoSkill(objects.clone())),
            Arc::new(GetChapterOutlineSkill(struct_roots.clone())),
            Arc::new(SearchForeshadowsSkill(foreshadows.clone())),
            Arc::new(GetMilestoneStatusSkill(milestones)),
            Arc::new(ListProjectFilesSkill(project_root.clone())),
            Arc::new(GetFileContentSkill(project_root.clone())),
            Arc::new(GetTextTemplatesSkill),
            // ── Write / mutation skills ───────────────────────────────────────
            Arc::new(AddWorldObjectSkill    { objects: objects.clone(), project_root: project_root.clone() }),
            Arc::new(UpdateWorldObjectSkill { objects: objects.clone(), project_root: project_root.clone() }),
            Arc::new(DeleteWorldObjectSkill { objects,                  project_root: project_root.clone() }),
            Arc::new(AddChapterNodeSkill    { struct_roots,             project_root: project_root.clone() }),
            Arc::new(AddForeshadowSkill     { foreshadows: foreshadows.clone(), project_root: project_root.clone() }),
            Arc::new(ResolveForeshadowSkill { foreshadows,              project_root: project_root.clone() }),
            Arc::new(WriteFileContentSkill(project_root)),
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
        assert_eq!(ss.len(), 15);
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
        assert_eq!(tools.as_array().unwrap().len(), 15);
    }

    #[test]
    fn test_skill_set_tool_names() {
        let ss = SkillSet::new(vec![], vec![], vec![], vec![], None);
        let names = ss.tool_names();
        assert_eq!(names.len(), 15);
        // Read-only skills
        assert!(names.contains(&"list_characters"));
        assert!(names.contains(&"get_character_info"));
        assert!(names.contains(&"get_chapter_outline"));
        assert!(names.contains(&"search_foreshadows"));
        assert!(names.contains(&"get_milestone_status"));
        assert!(names.contains(&"list_project_files"));
        assert!(names.contains(&"get_file_content"));
        assert!(names.contains(&"get_text_templates"));
        // Write/mutation skills
        assert!(names.contains(&"add_world_object"));
        assert!(names.contains(&"update_world_object"));
        assert!(names.contains(&"delete_world_object"));
        assert!(names.contains(&"add_chapter_node"));
        assert!(names.contains(&"add_foreshadow"));
        assert!(names.contains(&"resolve_foreshadow"));
        assert!(names.contains(&"write_file_content"));
    }

    #[test]
    fn test_skill_descriptions() {
        let ss = SkillSet::new(vec![], vec![], vec![], vec![], None);
        let descs = ss.descriptions();
        assert_eq!(descs.len(), 15);
        let names: Vec<String> = descs.iter().map(|(n, _)| n.clone()).collect();
        assert!(names.contains(&"list_characters".to_owned()));
        assert!(names.contains(&"get_character_info".to_owned()));
        assert!(names.contains(&"get_chapter_outline".to_owned()));
        assert!(names.contains(&"search_foreshadows".to_owned()));
        assert!(names.contains(&"get_milestone_status".to_owned()));
        assert!(names.contains(&"list_project_files".to_owned()));
        assert!(names.contains(&"get_file_content".to_owned()));
        assert!(names.contains(&"get_text_templates".to_owned()));
        assert!(names.contains(&"add_world_object".to_owned()));
        assert!(names.contains(&"update_world_object".to_owned()));
        assert!(names.contains(&"delete_world_object".to_owned()));
        assert!(names.contains(&"add_chapter_node".to_owned()));
        assert!(names.contains(&"add_foreshadow".to_owned()));
        assert!(names.contains(&"resolve_foreshadow".to_owned()));
        assert!(names.contains(&"write_file_content".to_owned()));
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

    // ── Skill: add_world_object ───────────────────────────────────────────────

    #[test]
    fn test_add_world_object_skill() {
        let dir = std::env::temp_dir().join("qingmo_test_add_obj");
        std::fs::create_dir_all(dir.join("Design")).unwrap();

        let skill = AddWorldObjectSkill { objects: sample_objects(), project_root: Some(dir.clone()) };
        let result = skill.execute(&serde_json::json!({
            "name": "新角色", "kind": "人物", "description": "神秘人物"
        })).unwrap();
        assert_eq!(result["status"], "success");

        // Verify file was written with 3 objects
        let content = std::fs::read_to_string(dir.join("Design").join("世界对象.json")).unwrap();
        let objs: Vec<WorldObject> = serde_json::from_str(&content).unwrap();
        assert_eq!(objs.len(), 3);
        assert!(objs.iter().any(|o| o.name == "新角色"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_add_world_object_duplicate() {
        let dir = std::env::temp_dir().join("qingmo_test_add_obj_dup");
        std::fs::create_dir_all(dir.join("Design")).unwrap();
        let skill = AddWorldObjectSkill { objects: sample_objects(), project_root: Some(dir.clone()) };
        let result = skill.execute(&serde_json::json!({"name": "李明", "kind": "人物"}));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("已存在"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_add_world_object_no_project() {
        let skill = AddWorldObjectSkill { objects: vec![], project_root: None };
        let result = skill.execute(&serde_json::json!({"name": "测试", "kind": "其他"}));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("项目未打开"));
    }

    // ── Skill: update_world_object ────────────────────────────────────────────

    #[test]
    fn test_update_world_object_skill() {
        let dir = std::env::temp_dir().join("qingmo_test_upd_obj");
        std::fs::create_dir_all(dir.join("Design")).unwrap();

        let skill = UpdateWorldObjectSkill { objects: sample_objects(), project_root: Some(dir.clone()) };
        let result = skill.execute(&serde_json::json!({
            "name": "李明", "description": "更新后的描述"
        })).unwrap();
        assert_eq!(result["status"], "success");

        let content = std::fs::read_to_string(dir.join("Design").join("世界对象.json")).unwrap();
        let objs: Vec<WorldObject> = serde_json::from_str(&content).unwrap();
        let obj = objs.iter().find(|o| o.name == "李明").unwrap();
        assert_eq!(obj.description, "更新后的描述");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_update_world_object_not_found() {
        let dir = std::env::temp_dir().join("qingmo_test_upd_obj_nf");
        std::fs::create_dir_all(dir.join("Design")).unwrap();
        let skill = UpdateWorldObjectSkill { objects: sample_objects(), project_root: Some(dir.clone()) };
        let result = skill.execute(&serde_json::json!({"name": "不存在的人", "description": "x"}));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("未找到"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    // ── Skill: delete_world_object ────────────────────────────────────────────

    #[test]
    fn test_delete_world_object_skill() {
        let dir = std::env::temp_dir().join("qingmo_test_del_obj");
        std::fs::create_dir_all(dir.join("Design")).unwrap();

        let skill = DeleteWorldObjectSkill { objects: sample_objects(), project_root: Some(dir.clone()) };
        let result = skill.execute(&serde_json::json!({"name": "李明"})).unwrap();
        assert_eq!(result["status"], "success");

        let content = std::fs::read_to_string(dir.join("Design").join("世界对象.json")).unwrap();
        let objs: Vec<WorldObject> = serde_json::from_str(&content).unwrap();
        assert_eq!(objs.len(), 1);
        assert!(!objs.iter().any(|o| o.name == "李明"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_delete_world_object_not_found() {
        let dir = std::env::temp_dir().join("qingmo_test_del_obj_nf");
        std::fs::create_dir_all(dir.join("Design")).unwrap();
        // Write existing JSON first so delete doesn't panic on missing file
        let json = serde_json::to_string_pretty(&sample_objects()).unwrap();
        std::fs::write(dir.join("Design").join("世界对象.json"), &json).unwrap();

        let skill = DeleteWorldObjectSkill { objects: sample_objects(), project_root: Some(dir.clone()) };
        let result = skill.execute(&serde_json::json!({"name": "不存在"}));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("未找到"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    // ── Skill: add_chapter_node ───────────────────────────────────────────────

    #[test]
    fn test_add_chapter_node_skill() {
        let dir = std::env::temp_dir().join("qingmo_test_add_ch");
        std::fs::create_dir_all(dir.join("Design")).unwrap();

        let skill = AddChapterNodeSkill { struct_roots: sample_roots(), project_root: Some(dir.clone()) };
        let result = skill.execute(&serde_json::json!({
            "title": "第二卷", "kind": "卷", "summary": "第二部分"
        })).unwrap();
        assert_eq!(result["status"], "success");

        let content = std::fs::read_to_string(dir.join("Design").join("章节结构.json")).unwrap();
        let roots: Vec<StructNode> = serde_json::from_str(&content).unwrap();
        assert_eq!(roots.len(), 2);
        assert!(roots.iter().any(|n| n.title == "第二卷"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    // ── Skill: add_foreshadow ─────────────────────────────────────────────────

    #[test]
    fn test_add_foreshadow_skill() {
        let dir = std::env::temp_dir().join("qingmo_test_add_fs");
        std::fs::create_dir_all(dir.join("Content")).unwrap();

        let skill = AddForeshadowSkill { foreshadows: sample_foreshadows(), project_root: Some(dir.clone()) };
        let result = skill.execute(&serde_json::json!({
            "name": "新伏笔", "description": "描述", "related_chapters": "第一章,第二章"
        })).unwrap();
        assert_eq!(result["status"], "success");

        let content = std::fs::read_to_string(dir.join("Content").join("伏笔.md")).unwrap();
        assert!(content.contains("新伏笔"));
        assert!(content.contains("第一章"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_add_foreshadow_duplicate() {
        let skill = AddForeshadowSkill { foreshadows: sample_foreshadows(), project_root: None };
        let result = skill.execute(&serde_json::json!({"name": "神秘信封"}));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("已存在"));
    }

    // ── Skill: resolve_foreshadow ─────────────────────────────────────────────

    #[test]
    fn test_resolve_foreshadow_skill() {
        let dir = std::env::temp_dir().join("qingmo_test_resolve_fs");
        std::fs::create_dir_all(dir.join("Content")).unwrap();

        // Write an initial foreshadow file
        let initial = "# 伏笔列表\n\n## 神秘信封 ⏳ 未解决\n\n第一章出现的信封\n\n";
        std::fs::write(dir.join("Content").join("伏笔.md"), initial).unwrap();

        let mut fs = sample_foreshadows();
        let skill = ResolveForeshadowSkill { foreshadows: fs.clone(), project_root: Some(dir.clone()) };
        let result = skill.execute(&serde_json::json!({"name": "神秘信封"})).unwrap();
        assert_eq!(result["status"], "success");

        let content = std::fs::read_to_string(dir.join("Content").join("伏笔.md")).unwrap();
        assert!(content.contains("✅ 已解决"));
        assert!(!content.contains("⏳ 未解决"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_resolve_foreshadow_not_found() {
        let skill = ResolveForeshadowSkill { foreshadows: sample_foreshadows(), project_root: None };
        let result = skill.execute(&serde_json::json!({"name": "不存在"}));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("未找到"));
    }

    // ── Skill: write_file_content ─────────────────────────────────────────────

    #[test]
    fn test_write_file_content_overwrite() {
        let dir = std::env::temp_dir().join("qingmo_test_write_file");
        std::fs::create_dir_all(dir.join("Content")).unwrap();
        std::fs::write(dir.join("Content").join("ch1.md"), "old content").unwrap();

        let skill = WriteFileContentSkill(Some(dir.clone()));
        let result = skill.execute(&serde_json::json!({
            "path": "Content/ch1.md", "content": "new content", "mode": "overwrite"
        })).unwrap();
        assert_eq!(result["status"], "success");
        let content = std::fs::read_to_string(dir.join("Content").join("ch1.md")).unwrap();
        assert_eq!(content, "new content");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_write_file_content_append() {
        let dir = std::env::temp_dir().join("qingmo_test_append_file");
        std::fs::create_dir_all(dir.join("Content")).unwrap();
        std::fs::write(dir.join("Content").join("ch2.md"), "line1\n").unwrap();

        let skill = WriteFileContentSkill(Some(dir.clone()));
        let result = skill.execute(&serde_json::json!({
            "path": "Content/ch2.md", "content": "line2\n"
        })).unwrap();
        assert_eq!(result["status"], "success");
        let content = std::fs::read_to_string(dir.join("Content").join("ch2.md")).unwrap();
        assert!(content.contains("line1"));
        assert!(content.contains("line2"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_write_file_content_rejects_json() {
        let dir = std::env::temp_dir().join("qingmo_test_write_json");
        std::fs::create_dir_all(&dir).unwrap();
        let skill = WriteFileContentSkill(Some(dir.clone()));
        let result = skill.execute(&serde_json::json!({
            "path": "Design/data.json", "content": "{}"
        }));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("仅支持写入"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_write_file_content_rejects_traversal() {
        let dir = std::env::temp_dir().join("qingmo_test_write_trav");
        std::fs::create_dir_all(&dir).unwrap();
        let skill = WriteFileContentSkill(Some(dir.clone()));
        let result = skill.execute(&serde_json::json!({
            "path": "../../evil.md", "content": "pwned"
        }));
        assert!(result.is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    // ── Skill: get_text_templates ─────────────────────────────────────────────

    #[test]
    fn test_get_text_templates_all() {
        let skill = GetTextTemplatesSkill;
        let result = skill.execute(&serde_json::json!({})).unwrap();
        let arr = result.as_array().unwrap();
        // Should return all 18 templates
        assert!(arr.len() >= 15);
        // Each entry should have category, name, template
        assert!(arr[0]["category"].is_string());
        assert!(arr[0]["name"].is_string());
        assert!(arr[0]["template"].is_string());
    }

    #[test]
    fn test_get_text_templates_filtered() {
        let skill = GetTextTemplatesSkill;
        let result = skill.execute(&serde_json::json!({"category": "开场"})).unwrap();
        let arr = result.as_array().unwrap();
        assert!(arr.len() >= 2);
        for item in arr {
            assert_eq!(item["category"], "开场");
        }
    }

    #[test]
    fn test_get_text_templates_no_match() {
        let skill = GetTextTemplatesSkill;
        let result = skill.execute(&serde_json::json!({"category": "xyz_不存在"})).unwrap();
        let arr = result.as_array().unwrap();
        assert_eq!(arr.len(), 0);
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
