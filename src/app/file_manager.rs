use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};

// ── File tree node ────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct FileNode {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    pub expanded: bool,
    pub children: Vec<FileNode>,
}

impl FileNode {
    pub fn from_path(path: &Path) -> Option<Self> {
        let name = path.file_name()?.to_string_lossy().into_owned();
        if path.is_dir() {
            let mut children: Vec<FileNode> = std::fs::read_dir(path)
                .ok()?
                .filter_map(|e| e.ok())
                .filter_map(|e| FileNode::from_path(&e.path()))
                .collect();
            children.sort_by(|a, b| {
                b.is_dir.cmp(&a.is_dir).then(a.name.cmp(&b.name))
            });
            Some(FileNode {
                name,
                path: path.to_owned(),
                is_dir: true,
                expanded: true,
                children,
            })
        } else {
            Some(FileNode {
                name,
                path: path.to_owned(),
                is_dir: false,
                expanded: false,
                children: vec![],
            })
        }
    }
}

// ── Outline entry (used for JSON sync) ───────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutlineEntry {
    pub level: u8,
    pub title: String,
    pub children: Vec<OutlineEntry>,
}

/// Parse Markdown headings into a flat list, then nest them.
pub fn parse_outline(markdown: &str) -> Vec<OutlineEntry> {
    let mut entries: Vec<(u8, String)> = vec![];
    for line in markdown.lines() {
        if let Some(rest) = line.strip_prefix("######") {
            entries.push((6, rest.trim().to_owned()));
        } else if let Some(rest) = line.strip_prefix("#####") {
            entries.push((5, rest.trim().to_owned()));
        } else if let Some(rest) = line.strip_prefix("####") {
            entries.push((4, rest.trim().to_owned()));
        } else if let Some(rest) = line.strip_prefix("###") {
            entries.push((3, rest.trim().to_owned()));
        } else if let Some(rest) = line.strip_prefix("##") {
            entries.push((2, rest.trim().to_owned()));
        } else if let Some(rest) = line.strip_prefix('#') {
            if rest.starts_with(' ') || rest.is_empty() {
                entries.push((1, rest.trim().to_owned()));
            }
        }
    }
    nest_entries(&entries, 1)
}

fn nest_entries(flat: &[(u8, String)], depth: u8) -> Vec<OutlineEntry> {
    let mut result = vec![];
    let mut i = 0;
    while i < flat.len() {
        let (lvl, title) = &flat[i];
        if *lvl == depth {
            // collect children (next level)
            let mut j = i + 1;
            while j < flat.len() && flat[j].0 > depth {
                j += 1;
            }
            let children = nest_entries(&flat[i + 1..j], depth + 1);
            result.push(OutlineEntry {
                level: depth,
                title: title.clone(),
                children,
            });
            i = j;
        } else if *lvl > depth {
            // skip - will be picked up by parent
            i += 1;
        } else {
            break;
        }
    }
    result
}

// ── Open file ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct OpenFile {
    pub path: PathBuf,
    pub content: String,
    pub modified: bool,
}

impl OpenFile {
    pub fn new(path: PathBuf, content: String) -> Self {
        OpenFile { path, content, modified: false }
    }

    pub fn save(&mut self) -> std::io::Result<()> {
        std::fs::write(&self.path, &self.content)?;
        self.modified = false;
        Ok(())
    }

    pub fn title(&self) -> String {
        let name = self.path.file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "untitled".to_owned());
        if self.modified {
            format!("● {name}")
        } else {
            name
        }
    }

    pub fn is_markdown(&self) -> bool {
        matches!(
            self.path.extension().and_then(|e| e.to_str()),
            Some("md") | Some("markdown")
        )
    }

    pub fn is_json(&self) -> bool {
        matches!(
            self.path.extension().and_then(|e| e.to_str()),
            Some("json")
        )
    }
}

// ── Thin wrappers around rfd ──────────────────────────────────────────────────

pub fn rfd_pick_folder() -> Option<PathBuf> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        rfd::FileDialog::new().pick_folder()
    }
    #[cfg(target_arch = "wasm32")]
    {
        None
    }
}

pub fn rfd_save_file(hint: &Path) -> Option<PathBuf> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let ext = hint.extension().and_then(|e| e.to_str()).unwrap_or("txt");
        let name = hint.file_name().and_then(|n| n.to_str()).unwrap_or("file");
        rfd::FileDialog::new()
            .set_file_name(name)
            .add_filter("文件", &[ext])
            .save_file()
    }
    #[cfg(target_arch = "wasm32")]
    {
        None
    }
}
