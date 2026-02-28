use egui::{Color32, RichText, Ui};
use crate::app::MarkdownSettings;

/// Render Markdown `content` as formatted egui widgets.
///
/// Supports:
/// - ATX headings (`#` … `######`)
/// - Fenced code blocks (``` ``` ```)
/// - Inline code (`` `code` ``)
/// - Blockquotes (`> …`)
/// - Unordered lists (`-`, `*`, `+`)
/// - Ordered lists (`1. …`)
/// - Horizontal rules (`---`, `***`, `___`)
/// - **Bold** and *italic* inline spans
/// - Plain paragraphs with blank-line spacing
pub(in crate::app) fn render_markdown(ui: &mut Ui, content: &str, settings: &MarkdownSettings) {
    let font_size = settings.preview_font_size;
    let mut in_code_block = false;
    let mut code_lines: Vec<&str> = Vec::new();

    for line in content.lines() {
        // ── Fenced code blocks ────────────────────────────────────────────────
        if line.trim_start().starts_with("```") {
            if in_code_block {
                let code_text = code_lines.join("\n");
                code_lines.clear();
                in_code_block = false;
                egui::Frame::none()
                    .fill(Color32::from_gray(28))
                    .inner_margin(8.0)
                    .rounding(4.0)
                    .show(ui, |ui| {
                        ui.add(
                            egui::Label::new(
                                RichText::new(&code_text)
                                    .monospace()
                                    .size(font_size - 1.0)
                                    .color(Color32::from_rgb(200, 220, 180)),
                            )
                            .wrap_mode(egui::TextWrapMode::Wrap),
                        );
                    });
            } else {
                in_code_block = true;
            }
            continue;
        }

        if in_code_block {
            code_lines.push(line);
            continue;
        }

        // ── Blank lines ───────────────────────────────────────────────────────
        if line.trim().is_empty() {
            ui.add_space(4.0);
            continue;
        }

        // ── ATX Headings ─────────────────────────────────────────────────────
        if let Some(rest) = strip_heading(line, 1) {
            ui.add_space(6.0);
            ui.label(RichText::new(rest).size(font_size * 1.8).strong().color(Color32::WHITE));
            ui.separator();
        } else if let Some(rest) = strip_heading(line, 2) {
            ui.add_space(4.0);
            ui.label(RichText::new(rest).size(font_size * 1.5).strong().color(Color32::from_gray(230)));
        } else if let Some(rest) = strip_heading(line, 3) {
            ui.add_space(2.0);
            ui.label(RichText::new(rest).size(font_size * 1.2).strong().color(Color32::from_gray(210)));
        } else if let Some(rest) = strip_heading(line, 4) {
            ui.label(RichText::new(rest).size(font_size).strong().color(Color32::from_gray(200)));
        } else if let Some(rest) = strip_heading(line, 5) {
            ui.label(RichText::new(rest).size(font_size * 0.95).strong().color(Color32::from_gray(190)));
        } else if let Some(rest) = strip_heading(line, 6) {
            ui.label(RichText::new(rest).size(font_size * 0.9).strong().color(Color32::from_gray(180)));
        }

        // ── Horizontal rule ───────────────────────────────────────────────────
        else if is_horizontal_rule(line) {
            ui.separator();
        }

        // ── Blockquote ────────────────────────────────────────────────────────
        else if let Some(rest) = line.strip_prefix("> ").or_else(|| line.strip_prefix(">")) {
            egui::Frame::none()
                .fill(Color32::from_gray(36))
                .inner_margin(egui::Margin { left: 10.0, right: 4.0, top: 2.0, bottom: 2.0 })
                .rounding(2.0)
                .show(ui, |ui| {
                    render_inline_text(ui, rest, font_size * 0.97, Color32::from_gray(180));
                });
        }

        // ── Unordered list ────────────────────────────────────────────────────
        else if let Some(rest) = line.strip_prefix("- ")
            .or_else(|| line.strip_prefix("* "))
            .or_else(|| line.strip_prefix("+ "))
        {
            ui.horizontal(|ui| {
                ui.add_space(8.0);
                ui.label(RichText::new("•").size(font_size).color(Color32::from_gray(160)));
                ui.add_space(2.0);
                render_inline_text(ui, rest, font_size, ui.visuals().text_color());
            });
        }

        // ── Ordered list (digit + ". ") ───────────────────────────────────────
        else if let Some((num, rest)) = parse_ordered_item(line) {
            ui.horizontal(|ui| {
                ui.add_space(8.0);
                ui.label(RichText::new(format!("{num}.")).size(font_size).color(Color32::from_gray(160)));
                ui.add_space(2.0);
                render_inline_text(ui, rest, font_size, ui.visuals().text_color());
            });
        }

        // ── Paragraph ─────────────────────────────────────────────────────────
        else {
            render_inline_text(ui, line, font_size, ui.visuals().text_color());
        }
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Strip `n` leading `#` characters followed by a space (or end of line).
fn strip_heading(line: &str, n: usize) -> Option<&str> {
    let prefix: String = "#".repeat(n);
    if line.starts_with(prefix.as_str()) {
        let after = &line[n..];
        if after.starts_with(' ') {
            Some(after[1..].trim_end())
        } else if after.is_empty() {
            Some("")
        } else {
            None
        }
    } else {
        None
    }
}

/// Return `true` if the line is a Markdown thematic break (`---`, `***`, `___`).
fn is_horizontal_rule(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.len() < 3 {
        return false;
    }
    let first = trimmed.chars().next().unwrap();
    if !matches!(first, '-' | '*' | '_') {
        return false;
    }
    trimmed.chars().all(|c| c == first || c == ' ')
        && trimmed.chars().filter(|&c| c == first).count() >= 3
}

/// If `line` is an ordered-list item (`1. text`), return `(number_str, rest_text)`.
fn parse_ordered_item(line: &str) -> Option<(&str, &str)> {
    let dot = line.find(". ")?;
    let num = &line[..dot];
    if num.chars().all(|c| c.is_ascii_digit()) && !num.is_empty() {
        Some((num, &line[dot + 2..]))
    } else {
        None
    }
}

// ── Inline renderer ───────────────────────────────────────────────────────────

/// Render a single line of text, parsing `**bold**`, `*italic*`, and `` `code` ``.
fn render_inline_text(ui: &mut Ui, text: &str, font_size: f32, default_color: Color32) {
    if !text.contains("**") && !text.contains('*') && !text.contains('`') {
        // Fast path – no inline markup
        ui.add(
            egui::Label::new(RichText::new(text).size(font_size).color(default_color))
                .wrap_mode(egui::TextWrapMode::Wrap),
        );
        return;
    }

    let job = build_inline_job(text, font_size, default_color);
    ui.add(egui::Label::new(job).wrap_mode(egui::TextWrapMode::Wrap));
}

/// Parse inline Markdown spans into an egui `LayoutJob`.
///
/// Recognised spans (processed left-to-right, longest match first):
/// - `**text**` → bold colour / white
/// - `*text*`   → italic
/// - `` `code` `` → monospace with background
fn build_inline_job(text: &str, font_size: f32, default_color: Color32) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::default();
    let bytes = text.as_bytes();
    let len = bytes.len();
    let mut i = 0;
    let mut plain_start = 0;

    let plain_fmt = egui::TextFormat {
        font_id: egui::FontId::proportional(font_size),
        color: default_color,
        ..Default::default()
    };

    macro_rules! flush_plain {
        () => {
            if plain_start < i {
                job.append(&text[plain_start..i], 0.0, plain_fmt.clone());
            }
        };
    }

    while i < len {
        // Bold: **...**  (check before single *)
        if i + 1 < len && bytes[i] == b'*' && bytes[i + 1] == b'*' {
            let open = i;
            flush_plain!();
            i += 2;
            let start = i;
            // find closing **
            let mut found_close = false;
            while i + 1 < len {
                if bytes[i] == b'*' && bytes[i + 1] == b'*' {
                    found_close = true;
                    break;
                }
                i += 1;
            }
            if found_close {
                let bold_text = &text[start..i];
                i += 2; // skip closing **
                if !bold_text.is_empty() {
                    job.append(bold_text, 0.0, egui::TextFormat {
                        font_id: egui::FontId::proportional(font_size),
                        color: Color32::WHITE,
                        ..Default::default()
                    });
                }
            } else {
                // No closing ** found – treat opening ** as literal text
                i = open + 2;
                job.append("**", 0.0, plain_fmt.clone());
                // continue scanning from after the opening **
            }
            plain_start = i;
        }
        // Inline code: `...`
        else if bytes[i] == b'`' {
            flush_plain!();
            i += 1;
            let start = i;
            while i < len && bytes[i] != b'`' {
                i += 1;
            }
            let code_text = &text[start..i];
            if i < len { i += 1; } // skip closing `
            if !code_text.is_empty() {
                job.append(code_text, 0.0, egui::TextFormat {
                    font_id: egui::FontId::monospace(font_size - 1.0),
                    color: Color32::from_rgb(200, 220, 180),
                    background: Color32::from_gray(40),
                    ..Default::default()
                });
            }
            plain_start = i;
        }
        // Italic: *...*  (single asterisk)
        else if bytes[i] == b'*' {
            flush_plain!();
            i += 1;
            let start = i;
            while i < len && bytes[i] != b'*' {
                i += 1;
            }
            let italic_text = &text[start..i];
            if i < len { i += 1; } // skip closing *
            if !italic_text.is_empty() {
                job.append(italic_text, 0.0, egui::TextFormat {
                    font_id: egui::FontId::proportional(font_size),
                    color: Color32::from_gray(200),
                    italics: true,
                    ..Default::default()
                });
            }
            plain_start = i;
        }
        else {
            i += 1;
        }
    }

    // Flush remaining plain text
    if plain_start < text.len() {
        job.append(&text[plain_start..], 0.0, plain_fmt);
    }

    job
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_heading() {
        assert_eq!(strip_heading("# Hello", 1), Some("Hello"));
        assert_eq!(strip_heading("## World", 2), Some("World"));
        assert_eq!(strip_heading("### Test", 3), Some("Test"));
        assert_eq!(strip_heading("#NoSpace", 1), None);
        assert_eq!(strip_heading("# ", 1), Some(""));
    }

    #[test]
    fn test_is_horizontal_rule() {
        assert!(is_horizontal_rule("---"));
        assert!(is_horizontal_rule("***"));
        assert!(is_horizontal_rule("___"));
        assert!(is_horizontal_rule("----"));
        assert!(is_horizontal_rule("- - -"));
        assert!(!is_horizontal_rule("--"));
        assert!(!is_horizontal_rule("abc"));
    }

    #[test]
    fn test_parse_ordered_item() {
        let (num, rest) = parse_ordered_item("1. First item").unwrap();
        assert_eq!(num, "1");
        assert_eq!(rest, "First item");

        let (num2, rest2) = parse_ordered_item("10. Tenth item").unwrap();
        assert_eq!(num2, "10");
        assert_eq!(rest2, "Tenth item");

        assert!(parse_ordered_item("Not a list").is_none());
        assert!(parse_ordered_item("a. Not ordered").is_none());
    }

    #[test]
    fn test_build_inline_job_plain() {
        let color = egui::Color32::WHITE;
        let job = build_inline_job("plain text", 14.0, color);
        // One section: plain text
        assert_eq!(job.sections.len(), 1);
        assert_eq!(&job.text, "plain text");
    }

    #[test]
    fn test_build_inline_job_bold() {
        let color = egui::Color32::WHITE;
        let job = build_inline_job("**bold**", 14.0, color);
        assert_eq!(&job.text, "bold");
    }

    #[test]
    fn test_build_inline_job_italic() {
        let color = egui::Color32::WHITE;
        let job = build_inline_job("*italic*", 14.0, color);
        assert_eq!(&job.text, "italic");
        assert!(job.sections[0].format.italics);
    }

    #[test]
    fn test_build_inline_job_code() {
        let color = egui::Color32::WHITE;
        let job = build_inline_job("`code`", 14.0, color);
        assert_eq!(&job.text, "code");
    }

    #[test]
    fn test_build_inline_job_mixed() {
        let color = egui::Color32::WHITE;
        let job = build_inline_job("Hello **world** and *there*", 14.0, color);
        // "Hello " + "world" + " and " + "there"
        assert_eq!(&job.text, "Hello world and there");
    }

    #[test]
    fn test_build_inline_job_chinese() {
        // Ensure multi-byte UTF-8 characters don't break the parser
        let color = egui::Color32::WHITE;
        let job = build_inline_job("你好 **世界**", 14.0, color);
        assert_eq!(&job.text, "你好 世界");
    }

    #[test]
    fn test_build_inline_job_chinese_italic() {
        let color = egui::Color32::WHITE;
        let job = build_inline_job("*中文斜体*", 14.0, color);
        assert_eq!(&job.text, "中文斜体");
        assert!(job.sections[0].format.italics);
    }

    #[test]
    fn test_build_inline_job_chinese_code() {
        let color = egui::Color32::WHITE;
        let job = build_inline_job("`中文代码`", 14.0, color);
        assert_eq!(&job.text, "中文代码");
    }

    #[test]
    fn test_build_inline_job_unclosed_bold() {
        // Unclosed ** should be treated as literal text, not bold
        let color = egui::Color32::WHITE;
        let job = build_inline_job("**unclosed", 14.0, color);
        assert_eq!(&job.text, "**unclosed");
    }
}
