//! Compile-time markdown to HTML conversion.
//!
//! Provides `markdown_html!` macro for converting markdown to HTML at compile
//! time, either from inline strings or from files.
//!
//! # Inline Markdown
//!
//! ```rust
//! use markdown_html::markdown_html;
//!
//! let html = markdown_html!("# Hello\n\nWorld");
//! assert!(html.contains("<h1>"));
//! ```
//!
//! # From Files
//!
//! Use the `file:` prefix to load markdown from a file. Paths are relative to
//! the workspace root:
//!
//! ```rust,ignore
//! use markdown_html::markdown_html;
//!
//! let html = markdown_html!(file: "docs/getting-started.md");
//! ```
//!
//! # Sectioned Files
//!
//! A single markdown file can be split into named sections using
//! `<!-- @@section:NAME -->` marker comments. Each marker starts a new
//! section; content runs until the next marker or end of file. Content
//! before the first marker is discarded. Pass `section: "name"` to extract
//! a specific section:
//!
//! ```rust,ignore
//! use markdown_html::markdown_html;
//!
//! let intro = markdown_html!(
//!     file: "ui/src/pages/auction_guide.md",
//!     section: "intro"
//! );
//! ```
//!
//! Section names must match `[A-Za-z0-9_-]+`. Duplicate section names are a
//! compile error. Known limitation: markers inside fenced code blocks are
//! still interpreted as section boundaries.

use proc_macro::TokenStream;
use pulldown_cmark::{Options, Parser, html};
use quote::quote;
use std::collections::HashMap;
use std::path::PathBuf;
use syn::parse::{Parse, ParseStream};
use syn::{LitStr, Token, parse_macro_input};

/// Input for markdown_html - either a string literal or a file reference with
/// an optional section name.
enum MarkdownInput {
    Literal(String),
    File {
        path: String,
        section: Option<String>,
    },
}

impl Parse for MarkdownInput {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        // Check for `file:` prefix
        if input.peek(syn::Ident) {
            let ident: syn::Ident = input.parse()?;
            if ident != "file" {
                return Err(syn::Error::new(
                    ident.span(),
                    "expected string literal or `file: \"path\"`",
                ));
            }
            input.parse::<Token![:]>()?;
            let path: LitStr = input.parse()?;

            let mut section = None;
            if input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
                let key: syn::Ident = input.parse()?;
                if key != "section" {
                    return Err(syn::Error::new(
                        key.span(),
                        "expected `section`",
                    ));
                }
                input.parse::<Token![:]>()?;
                let name: LitStr = input.parse()?;
                section = Some(name.value());
            }

            return Ok(MarkdownInput::File {
                path: path.value(),
                section,
            });
        }

        // Otherwise parse as string literal
        let lit: LitStr = input.parse()?;
        Ok(MarkdownInput::Literal(lit.value()))
    }
}

/// Converts markdown to HTML at compile time.
///
/// # Inline markdown
///
/// ```rust
/// use markdown_html::markdown_html;
///
/// let html = markdown_html!("# Hello\n\nWorld");
/// assert!(html.contains("<h1>"));
/// ```
///
/// # From file
///
/// ```rust,ignore
/// use markdown_html::markdown_html;
///
/// // Path relative to workspace root
/// let html = markdown_html!(file: "docs/page.md");
/// ```
///
/// # From a section of a file
///
/// ```rust,ignore
/// use markdown_html::markdown_html;
///
/// let html = markdown_html!(
///     file: "ui/src/pages/auction_guide.md",
///     section: "intro"
/// );
/// ```
#[proc_macro]
pub fn markdown_html(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as MarkdownInput);

    match input {
        MarkdownInput::Literal(s) => {
            let html_output = markdown_to_html(&s);
            quote! { #html_output }.into()
        }
        MarkdownInput::File { path, section } => {
            let full_path = match resolve_file_path(&path) {
                Ok(p) => p,
                Err(e) => return e.to_compile_error().into(),
            };

            let content = match std::fs::read_to_string(&full_path) {
                Ok(c) => c,
                Err(e) => {
                    return syn::Error::new(
                        proc_macro2::Span::call_site(),
                        format!(
                            "Failed to read {}: {}",
                            full_path.display(),
                            e
                        ),
                    )
                    .to_compile_error()
                    .into();
                }
            };

            let markdown = match section {
                None => content,
                Some(name) => match extract_section(&content, &name) {
                    Ok(body) => body,
                    Err(e) => {
                        return syn::Error::new(
                            proc_macro2::Span::call_site(),
                            format_section_error(&e, &path),
                        )
                        .to_compile_error()
                        .into();
                    }
                },
            };

            let html_output = markdown_to_html(&markdown);

            // Canonicalize so the embedded path is stable and free of `..`
            // segments. `include_bytes!` makes rustc treat the file as a
            // build dependency, so changes trigger a rebuild. Wrapping in a
            // block with `const _` keeps the expression valid in `const`
            // context (e.g. `const FOO: &str = markdown_html!(...)`).
            let abs_path = full_path
                .canonicalize()
                .unwrap_or(full_path)
                .to_string_lossy()
                .into_owned();

            quote! {
                {
                    const _: &[u8] = include_bytes!(#abs_path);
                    #html_output
                }
            }
            .into()
        }
    }
}

fn resolve_file_path(relative_path: &str) -> syn::Result<PathBuf> {
    // Get the workspace root from CARGO_MANIFEST_DIR. For workspace members
    // this points to the member crate, so we also try one level up to find
    // paths rooted at the workspace.
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")
        .expect("CARGO_MANIFEST_DIR not set");

    let manifest_path = PathBuf::from(&manifest_dir);
    let direct_path = manifest_path.join(relative_path);
    let workspace_path = manifest_path.join("..").join(relative_path);

    if direct_path.exists() {
        Ok(direct_path)
    } else if workspace_path.exists() {
        Ok(workspace_path)
    } else {
        Err(syn::Error::new(
            proc_macro2::Span::call_site(),
            format!(
                "File not found: {} (tried {} and {})",
                relative_path,
                direct_path.display(),
                workspace_path.display()
            ),
        ))
    }
}

#[derive(Debug)]
pub(crate) enum SectionError {
    NoMarkers,
    NotFound {
        requested: String,
        available: Vec<String>,
    },
    Duplicate {
        name: String,
        first_line: usize,
        second_line: usize,
    },
    InvalidName {
        name: String,
        line: usize,
    },
}

fn format_section_error(err: &SectionError, file: &str) -> String {
    match err {
        SectionError::NoMarkers => {
            format!("file {file} contains no @@section markers")
        }
        SectionError::NotFound {
            requested,
            available,
        } => {
            format!(
                "section '{requested}' not found in {file}. \
                 Available sections: {}",
                available.join(", ")
            )
        }
        SectionError::Duplicate {
            name,
            first_line,
            second_line,
        } => {
            format!(
                "duplicate section '{name}' in {file} \
                 at lines {first_line} and {second_line}"
            )
        }
        SectionError::InvalidName { name, line } => {
            format!(
                "invalid section name '{name}' in {file} at line {line} \
                 (allowed: alphanumeric, underscore, hyphen)"
            )
        }
    }
}

/// Parses a section marker line. Returns the section name if the line is a
/// marker, `None` otherwise. Handles surrounding whitespace and CRLF.
fn parse_marker(line: &str) -> Option<&str> {
    let t = line.trim();
    let t = t.strip_prefix("<!--")?.trim_start();
    let t = t.strip_prefix("@@section:")?;
    let t = t.strip_suffix("-->")?.trim_end();
    let name = t.trim();
    if name.is_empty() {
        return None;
    }
    Some(name)
}

fn is_valid_section_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// Extracts the body of a named section from a markdown file. Content before
/// the first marker is discarded. Duplicate names and invalid names are
/// errors.
pub(crate) fn extract_section(
    md: &str,
    wanted: &str,
) -> Result<String, SectionError> {
    #[derive(Clone)]
    struct Span {
        name: String,
        // 1-based line number of the marker, for human-facing error messages.
        marker_line: usize,
        // 0-based indices into `lines`, for slicing out the section body.
        body_start: usize,
        body_end: usize,
    }

    let lines: Vec<&str> = md.split_inclusive('\n').collect();
    let mut spans: Vec<Span> = Vec::new();

    for (idx, line) in lines.iter().enumerate() {
        if let Some(name) = parse_marker(line) {
            if !is_valid_section_name(name) {
                return Err(SectionError::InvalidName {
                    name: name.to_string(),
                    line: idx + 1,
                });
            }
            if let Some(prev) = spans.last_mut() {
                prev.body_end = idx;
            }
            spans.push(Span {
                name: name.to_string(),
                marker_line: idx + 1,
                body_start: idx + 1,
                body_end: lines.len(),
            });
        }
    }

    if spans.is_empty() {
        return Err(SectionError::NoMarkers);
    }

    let mut seen: HashMap<&str, usize> = HashMap::new();
    for span in &spans {
        if let Some(&first_line) = seen.get(span.name.as_str()) {
            return Err(SectionError::Duplicate {
                name: span.name.clone(),
                first_line,
                second_line: span.marker_line,
            });
        }
        seen.insert(&span.name, span.marker_line);
    }

    let found = spans.iter().find(|s| s.name == wanted);
    match found {
        Some(span) => {
            let body: String = lines[span.body_start..span.body_end]
                .iter()
                .copied()
                .collect();
            Ok(body)
        }
        None => Err(SectionError::NotFound {
            requested: wanted.to_string(),
            available: spans.into_iter().map(|s| s.name).collect(),
        }),
    }
}

fn markdown_to_html(markdown: &str) -> String {
    // ENABLE_HEADING_ATTRIBUTES allows {#id} and {.class} syntax on headings.
    // This is safe because this macro only processes trusted content at compile
    // time. User-supplied markdown uses a separate runtime renderer.
    let options = Options::ENABLE_TABLES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_SMART_PUNCTUATION
        | Options::ENABLE_HEADING_ATTRIBUTES;

    let parser = Parser::new_ext(markdown, options);
    let mut html_output = String::new();
    html::push_html(&mut html_output, parser);

    html_output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_marker_basic() {
        assert_eq!(parse_marker("<!-- @@section:foo -->"), Some("foo"));
    }

    #[test]
    fn parse_marker_trailing_whitespace() {
        assert_eq!(parse_marker("<!-- @@section:foo    -->\n"), Some("foo"));
    }

    #[test]
    fn parse_marker_crlf() {
        assert_eq!(parse_marker("<!-- @@section:foo -->\r\n"), Some("foo"));
    }

    #[test]
    fn parse_marker_not_a_marker() {
        assert_eq!(parse_marker("<!-- regular comment -->"), None);
        assert_eq!(parse_marker("# heading"), None);
        assert_eq!(parse_marker(""), None);
    }

    #[test]
    fn extract_basic_split() {
        let md = "\
<!-- @@section:a -->
first body
<!-- @@section:b -->
second body
";
        assert_eq!(extract_section(md, "a").unwrap(), "first body\n");
        assert_eq!(extract_section(md, "b").unwrap(), "second body\n");
    }

    #[test]
    fn extract_discards_preamble() {
        let md = "\
preamble content
more preamble
<!-- @@section:a -->
body
";
        assert_eq!(extract_section(md, "a").unwrap(), "body\n");
    }

    #[test]
    fn extract_empty_body() {
        let md = "\
<!-- @@section:a -->
<!-- @@section:b -->
body
";
        assert_eq!(extract_section(md, "a").unwrap(), "");
        assert_eq!(extract_section(md, "b").unwrap(), "body\n");
    }

    #[test]
    fn extract_crlf() {
        let md = "<!-- @@section:a -->\r\nbody\r\n<!-- @@section:b -->\r\n";
        assert_eq!(extract_section(md, "a").unwrap(), "body\r\n");
    }

    #[test]
    fn extract_no_markers() {
        let md = "just some markdown\nno markers here\n";
        assert!(matches!(
            extract_section(md, "a"),
            Err(SectionError::NoMarkers)
        ));
    }

    #[test]
    fn extract_not_found_lists_available() {
        let md = "\
<!-- @@section:alpha -->
x
<!-- @@section:beta -->
y
";
        match extract_section(md, "gamma") {
            Err(SectionError::NotFound {
                requested,
                available,
            }) => {
                assert_eq!(requested, "gamma");
                assert_eq!(available, vec!["alpha", "beta"]);
            }
            other => panic!("expected NotFound, got {other:?}"),
        }
    }

    #[test]
    fn extract_duplicate_is_error() {
        let md = "\
<!-- @@section:a -->
first
<!-- @@section:a -->
second
";
        match extract_section(md, "a") {
            Err(SectionError::Duplicate {
                name,
                first_line,
                second_line,
            }) => {
                assert_eq!(name, "a");
                assert_eq!(first_line, 1);
                assert_eq!(second_line, 3);
            }
            other => panic!("expected Duplicate, got {other:?}"),
        }
    }

    #[test]
    fn extract_invalid_name() {
        let md = "<!-- @@section:has space -->\nbody\n";
        assert!(matches!(
            extract_section(md, "has space"),
            Err(SectionError::InvalidName { .. })
        ));
    }
}
