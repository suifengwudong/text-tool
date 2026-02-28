use egui::Color32;
use serde::{Deserialize, Serialize};

// ── Character / Chapter / Foreshadow data ─────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RelationKind {
    Friend,   // 友好
    Enemy,    // 敌对
    Family,   // 亲属
    Other,    // 其他
}

impl RelationKind {
    pub fn label(&self) -> &'static str {
        match self {
            RelationKind::Friend => "友好",
            RelationKind::Enemy => "敌对",
            RelationKind::Family => "亲属",
            RelationKind::Other => "其他",
        }
    }
    pub fn all() -> &'static [RelationKind] {
        &[RelationKind::Friend, RelationKind::Enemy, RelationKind::Family, RelationKind::Other]
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Relationship {
    pub target: String,
    pub kind: RelationKind,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Character {
    pub name: String,
    pub traits: String,
    pub background: String,
    pub relationships: Vec<Relationship>,
}

impl Character {
    pub fn new(name: &str) -> Self {
        Character {
            name: name.to_owned(),
            traits: String::new(),
            background: String::new(),
            relationships: vec![],
        }
    }
}

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
            ChapterTag::Normal => "普通",
            ChapterTag::Climax => "高潮",
            ChapterTag::Foreshadow => "伏笔",
            ChapterTag::Transition => "过渡",
        }
    }
    pub fn all() -> &'static [ChapterTag] {
        &[ChapterTag::Normal, ChapterTag::Climax, ChapterTag::Foreshadow, ChapterTag::Transition]
    }
    pub fn color(&self) -> Color32 {
        match self {
            ChapterTag::Normal => Color32::from_gray(160),
            ChapterTag::Climax => Color32::from_rgb(220, 80, 80),
            ChapterTag::Foreshadow => Color32::from_rgb(80, 160, 220),
            ChapterTag::Transition => Color32::from_rgb(120, 190, 120),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chapter {
    pub title: String,
    pub tag: ChapterTag,
    pub summary: String,
    pub word_count: u32,
    pub done: bool,
}

impl Chapter {
    pub fn new(title: &str) -> Self {
        Chapter {
            title: title.to_owned(),
            tag: ChapterTag::Normal,
            summary: String::new(),
            word_count: 0,
            done: false,
        }
    }
}

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

// ── Panel IDs ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Panel {
    Novel,
    Characters,
    Outline,
    LLM,
}

impl Panel {
    pub fn icon(self) -> &'static str {
        match self {
            Panel::Novel => "📝",
            Panel::Characters => "👤",
            Panel::Outline => "🧭",
            Panel::LLM => "🤖",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Panel::Novel => "小说编辑",
            Panel::Characters => "人设&章节",
            Panel::Outline => "大纲&伏笔",
            Panel::LLM => "LLM辅助",
        }
    }
}
