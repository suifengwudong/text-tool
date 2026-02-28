use egui::Color32;
use serde::{Deserialize, Serialize};

// ── ObjectKind ────────────────────────────────────────────────────────────────

/// The category of a world object (content element).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ObjectKind {
    Character,  // 人物
    Scene,      // 场景
    Location,   // 地点
    Item,       // 道具
    Faction,    // 势力
    Other,      // 其他
}

impl ObjectKind {
    pub fn label(&self) -> &'static str {
        match self {
            ObjectKind::Character => "人物",
            ObjectKind::Scene     => "场景",
            ObjectKind::Location  => "地点",
            ObjectKind::Item      => "道具",
            ObjectKind::Faction   => "势力",
            ObjectKind::Other     => "其他",
        }
    }
    pub fn icon(&self) -> &'static str {
        match self {
            ObjectKind::Character => "👤",
            ObjectKind::Scene     => "🎭",
            ObjectKind::Location  => "📍",
            ObjectKind::Item      => "🗡",
            ObjectKind::Faction   => "🏰",
            ObjectKind::Other     => "⬡",
        }
    }
    pub fn all() -> &'static [ObjectKind] {
        &[
            ObjectKind::Character,
            ObjectKind::Scene,
            ObjectKind::Location,
            ObjectKind::Item,
            ObjectKind::Faction,
            ObjectKind::Other,
        ]
    }
}

// ── RelationKind ──────────────────────────────────────────────────────────────

/// The semantic type of a link between two elements.
/// Works for Object↔Object, Object↔StructNode, and StructNode↔StructNode links.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RelationKind {
    // Object ↔ Object
    Friend,     // 友好
    Enemy,      // 敌对
    Family,     // 亲属
    Owns,       // 持有 (持有某道具)
    LocatedAt,  // 所在 (人物所在地点)
    BelongsTo,  // 所属 (人物所属势力)
    // Object ↔ StructNode
    AppearsIn,  // 出场 (对象在某章节出现)
    MentionedIn,// 提及 (对象在某章节被提及)
    // StructNode ↔ StructNode (non-parent cross links)
    Foreshadows,// 铺垫 (一节为另一节铺垫)
    Resolves,   // 回收 (一节回收另一节的伏笔)
    Parallels,  // 并行 (两节并行叙述)
    // Fallback
    Other,      // 其他
}

impl RelationKind {
    pub fn label(&self) -> &'static str {
        match self {
            RelationKind::Friend      => "友好",
            RelationKind::Enemy       => "敌对",
            RelationKind::Family      => "亲属",
            RelationKind::Owns        => "持有",
            RelationKind::LocatedAt   => "所在",
            RelationKind::BelongsTo   => "所属",
            RelationKind::AppearsIn   => "出场",
            RelationKind::MentionedIn => "提及",
            RelationKind::Foreshadows => "铺垫",
            RelationKind::Resolves    => "回收",
            RelationKind::Parallels   => "并行",
            RelationKind::Other       => "其他",
        }
    }
    pub fn all() -> &'static [RelationKind] {
        &[
            RelationKind::Friend,
            RelationKind::Enemy,
            RelationKind::Family,
            RelationKind::Owns,
            RelationKind::LocatedAt,
            RelationKind::BelongsTo,
            RelationKind::AppearsIn,
            RelationKind::MentionedIn,
            RelationKind::Foreshadows,
            RelationKind::Resolves,
            RelationKind::Parallels,
            RelationKind::Other,
        ]
    }
}

// ── LinkTarget ────────────────────────────────────────────────────────────────

/// What a link points to — another world object (by name) or a structure node
/// (by title).  Using names rather than integer IDs keeps the data human-readable
/// and consistent with the rest of the app, which uses names throughout.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum LinkTarget {
    /// Name of another `WorldObject`.
    Object(String),
    /// Title path of a `StructNode` (e.g. "第一卷/第一章").
    Node(String),
}

impl LinkTarget {
    pub fn display_name(&self) -> &str {
        match self {
            LinkTarget::Object(n) | LinkTarget::Node(n) => n,
        }
    }
    pub fn type_label(&self) -> &'static str {
        match self {
            LinkTarget::Object(_) => "对象",
            LinkTarget::Node(_)   => "章节",
        }
    }
}

// ── ObjectLink ────────────────────────────────────────────────────────────────

/// A directed association from a `WorldObject` to another element.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectLink {
    pub target: LinkTarget,
    pub kind: RelationKind,
    pub note: String,
}

// ── WorldObject ───────────────────────────────────────────────────────────────

/// A unified "content element": character, scene, location, item, faction, …
/// Replaces the old `Character` struct and extends it with a `kind` discriminant
/// and a generalised `links` list that can point to other objects *or* to
/// structure nodes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldObject {
    pub name: String,
    pub kind: ObjectKind,
    /// Core traits / description (what was `traits` in the old Character).
    pub description: String,
    pub background: String,
    pub links: Vec<ObjectLink>,
}

impl WorldObject {
    pub fn new(name: &str, kind: ObjectKind) -> Self {
        WorldObject {
            name: name.to_owned(),
            kind,
            description: String::new(),
            background: String::new(),
            links: vec![],
        }
    }
    pub fn icon(&self) -> &'static str { self.kind.icon() }
}

// ── ChapterTag ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ChapterTag {
    Normal,     // 普通
    Climax,     // 高潮
    Foreshadow, // 伏笔
    Transition, // 过渡
}

impl ChapterTag {
    pub fn label(&self) -> &'static str {
        match self {
            ChapterTag::Normal     => "普通",
            ChapterTag::Climax     => "高潮",
            ChapterTag::Foreshadow => "伏笔",
            ChapterTag::Transition => "过渡",
        }
    }
    pub fn all() -> &'static [ChapterTag] {
        &[ChapterTag::Normal, ChapterTag::Climax, ChapterTag::Foreshadow, ChapterTag::Transition]
    }
    pub fn color(&self) -> Color32 {
        match self {
            ChapterTag::Normal     => Color32::from_gray(160),
            ChapterTag::Climax     => Color32::from_rgb(220, 80, 80),
            ChapterTag::Foreshadow => Color32::from_rgb(80, 160, 220),
            ChapterTag::Transition => Color32::from_rgb(120, 190, 120),
        }
    }
}

// ── StructKind ────────────────────────────────────────────────────────────────

/// The hierarchical level of a structure node.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum StructKind {
    Outline,  // 总纲
    Volume,   // 卷
    Chapter,  // 章
    Section,  // 节
}

impl StructKind {
    pub fn label(&self) -> &'static str {
        match self {
            StructKind::Outline => "总纲",
            StructKind::Volume  => "卷",
            StructKind::Chapter => "章",
            StructKind::Section => "节",
        }
    }
    pub fn icon(&self) -> &'static str {
        match self {
            StructKind::Outline => "📋",
            StructKind::Volume  => "📚",
            StructKind::Chapter => "📖",
            StructKind::Section => "📑",
        }
    }
    pub fn all() -> &'static [StructKind] {
        &[StructKind::Outline, StructKind::Volume, StructKind::Chapter, StructKind::Section]
    }
    /// The natural child kind when adding a child to this level.
    pub fn default_child_kind(&self) -> StructKind {
        match self {
            StructKind::Outline => StructKind::Volume,
            StructKind::Volume  => StructKind::Chapter,
            StructKind::Chapter => StructKind::Section,
            StructKind::Section => StructKind::Section,
        }
    }
}

// ── NodeLink ──────────────────────────────────────────────────────────────────

/// A non-parent cross-link between two structure nodes (e.g. a chapter that
/// foreshadows another chapter many levels away).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeLink {
    /// Title of the target node.
    pub target_title: String,
    pub kind: RelationKind,
    pub note: String,
}

// ── StructNode ────────────────────────────────────────────────────────────────

/// A hierarchical structure element (总纲 / 卷 / 章 / 节).
/// Replaces the old flat `Chapter` and adds nesting, a `kind` discriminant,
/// a list of linked world-objects, and cross-node links.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructNode {
    pub title: String,
    pub kind: StructKind,
    pub tag: ChapterTag,
    pub summary: String,
    pub done: bool,
    /// Nested children (e.g. a Volume contains Chapters).
    pub children: Vec<StructNode>,
    /// Names of `WorldObject`s associated with this node.
    pub linked_objects: Vec<String>,
    /// Non-parent cross-links to other structure nodes.
    pub node_links: Vec<NodeLink>,
}

impl StructNode {
    pub fn new(title: &str, kind: StructKind) -> Self {
        StructNode {
            title: title.to_owned(),
            kind,
            tag: ChapterTag::Normal,
            summary: String::new(),
            done: false,
            children: vec![],
            linked_objects: vec![],
            node_links: vec![],
        }
    }

    /// Total number of leaf nodes (nodes without children).
    pub fn leaf_count(&self) -> usize {
        if self.children.is_empty() {
            1
        } else {
            self.children.iter().map(|c| c.leaf_count()).sum()
        }
    }

    /// Number of done leaf nodes.
    pub fn done_count(&self) -> usize {
        if self.children.is_empty() {
            if self.done { 1 } else { 0 }
        } else {
            self.children.iter().map(|c| c.done_count()).sum()
        }
    }
}

// ── Tree helpers ──────────────────────────────────────────────────────────────

/// Navigate immutably into a tree of `StructNode`s by index path.
#[allow(dead_code)]
pub fn node_at<'a>(roots: &'a [StructNode], path: &[usize]) -> Option<&'a StructNode> {
    if path.is_empty() { return None; }
    let node = roots.get(path[0])?;
    if path.len() == 1 { Some(node) } else { node_at(&node.children, &path[1..]) }
}

/// Navigate mutably into a tree of `StructNode`s by index path.
pub fn node_at_mut<'a>(roots: &'a mut Vec<StructNode>, path: &[usize]) -> Option<&'a mut StructNode> {
    if path.is_empty() { return None; }
    if path.len() == 1 {
        return roots.get_mut(path[0]);
    }
    let node = roots.get_mut(path[0])?;
    node_at_mut(&mut node.children, &path[1..])
}

/// Collect the flat title of every node in the tree (depth-first).
pub fn all_node_titles(roots: &[StructNode]) -> Vec<String> {
    let mut out = Vec::new();
    fn walk(nodes: &[StructNode], out: &mut Vec<String>) {
        for n in nodes {
            out.push(n.title.clone());
            walk(&n.children, out);
        }
    }
    walk(roots, &mut out);
    out
}

// ── Foreshadow ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Foreshadow {
    pub name: String,
    pub description: String,
    pub related_chapters: Vec<String>,
    pub resolved: bool,
}

impl Foreshadow {
    pub fn new(name: &str) -> Self {
        Foreshadow {
            name: name.to_owned(),
            description: String::new(),
            related_chapters: vec![],
            resolved: false,
        }
    }
}

// ── LLM config ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct LlmConfig {
    pub model_path: String,
    pub api_url: String,
    pub temperature: f32,
    pub max_tokens: u32,
    pub use_local: bool,
}

// ── Markdown rendering settings ───────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct MarkdownSettings {
    /// Base font size used when rendering the preview.
    pub preview_font_size: f32,
    /// When a Markdown file is opened, default to preview mode.
    pub default_to_preview: bool,
}

impl Default for MarkdownSettings {
    fn default() -> Self {
        MarkdownSettings {
            preview_font_size: 14.0,
            default_to_preview: false,
        }
    }
}

// ── Panel IDs ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Panel {
    Novel,
    /// 世界对象设计 (人物 / 场景 / 地点 / 道具 / 势力)
    Objects,
    /// 章节结构设计 (总纲 / 卷 / 章 / 节)
    Structure,
    LLM,
}

impl Panel {
    pub fn icon(self) -> &'static str {
        match self {
            Panel::Novel     => "📝",
            Panel::Objects   => "🌐",
            Panel::Structure => "🏗",
            Panel::LLM       => "🤖",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Panel::Novel     => "小说编辑",
            Panel::Objects   => "世界对象",
            Panel::Structure => "章节结构",
            Panel::LLM       => "LLM辅助",
        }
    }
}
