use std::path::{Path, PathBuf};

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
    /// Build a file tree node, optionally hiding `.json` files.
    pub fn from_path_filtered(path: &Path, hide_json: bool) -> Option<Self> {
        let name = path.file_name()?.to_string_lossy().into_owned();
        if path.is_dir() {
            let mut children: Vec<FileNode> = std::fs::read_dir(path)
                .ok()?
                .filter_map(|e| e.ok())
                .filter_map(|e| FileNode::from_path_filtered(&e.path(), hide_json))
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
            // When hide_json is set, exclude .json files from the visible tree.
            if hide_json && path.extension().and_then(|e| e.to_str()) == Some("json") {
                return None;
            }
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
