use std::path::{Path, PathBuf};

use super::{TextToolApp, SearchResult, rfd_save_file, rfd_pick_folder};

// ── Full-text search ──────────────────────────────────────────────────────────

impl TextToolApp {
    /// Scan all `.md` and `.json` files under the project root for
    /// `self.search_query` and populate `self.search_results`.
    pub(super) fn run_search(&mut self) {
        self.search_results.clear();
        let query = self.search_query.clone();
        if query.is_empty() {
            return;
        }
        let Some(root) = self.project_root.clone() else {
            self.status = "请先打开一个项目".to_owned();
            return;
        };
        search_dir(&root, &query, &mut self.search_results);
        self.status = format!(
            "搜索「{}」找到 {} 处结果",
            query,
            self.search_results.len()
        );
    }

    // ── Export & Backup ───────────────────────────────────────────────────────

    /// Concatenate all `Content/*.md` files in alphabetical order and save to a
    /// user-chosen file via a save-file dialog.
    pub(super) fn export_chapters_merged(&mut self) {
        let Some(root) = self.project_root.as_ref() else {
            self.status = "请先打开一个项目".to_owned();
            return;
        };
        let content_dir = root.join("Content");
        let mut md_files: Vec<PathBuf> = std::fs::read_dir(&content_dir)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("md"))
            .collect();
        md_files.sort();

        let mut merged = String::new();
        for path in &md_files {
            if let Ok(text) = std::fs::read_to_string(path) {
                let name = path.file_name().unwrap_or_default().to_string_lossy();
                merged.push_str(&format!("# ── {name} ──\n\n"));
                merged.push_str(&text);
                merged.push_str("\n\n");
            }
        }

        let dummy = PathBuf::from("merged.md");
        if let Some(dest) = rfd_save_file(&dummy) {
            match std::fs::write(&dest, &merged) {
                Ok(_) => self.status = format!("已导出合集到 {}", dest.display()),
                Err(e) => self.status = format!("导出失败: {e}"),
            }
        }
    }

    /// Copy the entire project folder to a user-selected destination directory.
    pub(super) fn backup_project(&mut self) {
        let Some(root) = self.project_root.clone() else {
            self.status = "请先打开一个项目".to_owned();
            return;
        };
        let Some(dest_parent) = rfd_pick_folder() else {
            return;
        };
        let folder_name = root.file_name().unwrap_or_default();
        let dest = dest_parent.join(folder_name);
        match copy_dir_all(&root, &dest) {
            Ok(_) => self.status = format!("已备份到 {}", dest.display()),
            Err(e) => self.status = format!("备份失败: {e}"),
        }
    }
}

// ── File utilities ────────────────────────────────────────────────────────────

/// Recursively scan `dir` for lines in `.md` / `.json` files that contain
/// `query`.  Results are appended to `results`.
pub(super) fn search_dir(dir: &Path, query: &str, results: &mut Vec<SearchResult>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            search_dir(&path, query, results);
        } else {
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if ext == "md" || ext == "json" {
                if let Ok(text) = std::fs::read_to_string(&path) {
                    for (line_no, line) in text.lines().enumerate() {
                        if line.contains(query) {
                            results.push(SearchResult {
                                file_path: path.clone(),
                                line_no: line_no + 1,
                                line: line.to_owned(),
                            });
                        }
                    }
                }
            }
        }
    }
}

/// Recursively copy directory `src` to `dst`, creating it if necessary.
pub(super) fn copy_dir_all(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let dst_path = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_all(&entry.path(), &dst_path)?;
        } else {
            std::fs::copy(entry.path(), dst_path)?;
        }
    }
    Ok(())
}
