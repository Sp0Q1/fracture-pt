//! Secure markdown rendering for finding descriptions and report content.
//!
//! Uses comrak with `unsafe_ = false` to prevent raw HTML injection.
//! All output is safe HTML suitable for rendering in templates with `|safe`
//! or embedding in PenText XML report content.

use comrak::{markdown_to_html, Options};

/// Render markdown to safe HTML.
///
/// - GFM extensions enabled (tables, strikethrough, autolinks, task lists)
/// - Raw HTML in input is escaped (not passed through)
/// - Output is safe for use with Tera `|safe` filter
#[must_use]
pub fn render(input: &str) -> String {
    let mut options = Options::default();
    options.extension.strikethrough = true;
    options.extension.table = true;
    options.extension.autolink = true;
    options.extension.tasklist = true;
    // CRITICAL: do not allow raw HTML passthrough — prevents XSS
    // (false is the default, but explicit is better for security-critical code)
    options.render.r#unsafe = false;
    markdown_to_html(input, &options)
}

/// Render markdown to safe HTML for PenText XML embedding.
///
/// Same as `render()` but strips the outer `<p>` wrapper for single-paragraph
/// content to produce cleaner XML. Multi-paragraph content is left as-is.
#[must_use]
pub fn render_for_xml(input: &str) -> String {
    let html = render(input);
    // If it's a single paragraph, strip the outer <p></p> for cleaner XML
    let trimmed = html.trim();
    if trimmed.starts_with("<p>")
        && trimmed.ends_with("</p>")
        && trimmed.matches("<p>").count() == 1
    {
        trimmed
            .strip_prefix("<p>")
            .and_then(|s| s.strip_suffix("</p>"))
            .unwrap_or(trimmed)
            .to_string()
    } else {
        html
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_markdown() {
        assert_eq!(render("**bold**").trim(), "<p><strong>bold</strong></p>");
    }

    #[test]
    fn test_xss_prevention() {
        let input = "<script>alert('xss')</script>";
        let output = render(input);
        assert!(!output.contains("<script>"));
    }

    #[test]
    fn test_image_markdown() {
        let input = "![screenshot](/api/uploads/abc-123)";
        let output = render(input);
        assert!(output.contains("<img src=\"/api/uploads/abc-123\""));
    }

    #[test]
    fn test_render_for_xml_single_paragraph() {
        assert_eq!(render_for_xml("simple text"), "simple text");
    }

    #[test]
    fn test_render_for_xml_multi_paragraph() {
        let output = render_for_xml("para 1\n\npara 2");
        assert!(output.contains("<p>"));
    }
}
