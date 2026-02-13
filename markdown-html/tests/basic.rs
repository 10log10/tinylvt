use markdown_html::markdown_html;

#[test]
fn test_heading() {
    let html = markdown_html!("## Hello");
    assert!(html.contains("<h2>"));
    assert!(html.contains("Hello"));
    assert!(html.contains("</h2>"));
}

#[test]
fn test_paragraph() {
    let html = markdown_html!("This is a paragraph.");
    assert!(html.contains("<p>"));
    assert!(html.contains("This is a paragraph."));
}

#[test]
fn test_bold() {
    let html = markdown_html!("This is **bold** text.");
    assert!(html.contains("<strong>bold</strong>"));
}

#[test]
fn test_italic() {
    let html = markdown_html!("This is *italic* text.");
    assert!(html.contains("<em>italic</em>"));
}

#[test]
fn test_link() {
    let html = markdown_html!("[Click here](https://example.com)");
    assert!(html.contains(r#"<a href="https://example.com">Click here</a>"#));
}

#[test]
fn test_list() {
    let html = markdown_html!("- Item 1\n- Item 2");
    assert!(html.contains("<ul>"));
    assert!(html.contains("<li>Item 1</li>"));
    assert!(html.contains("<li>Item 2</li>"));
}

#[test]
fn test_multiline() {
    let html = markdown_html!(
        r#"
# Title

First paragraph.

Second paragraph with **bold**.
"#
    );
    assert!(html.contains("<h1>Title</h1>"));
    assert!(html.contains("<p>First paragraph.</p>"));
    assert!(html.contains("<strong>bold</strong>"));
}
