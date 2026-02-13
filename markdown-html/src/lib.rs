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

use proc_macro::TokenStream;
use pulldown_cmark::{Options, Parser, html};
use quote::quote;
use std::path::PathBuf;
use syn::parse::{Parse, ParseStream};
use syn::{LitStr, Token, parse_macro_input};

/// Input for markdown_html - either a string literal or `file: "path"`
enum MarkdownInput {
    Literal(String),
    File(String),
}

impl Parse for MarkdownInput {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        // Check for `file:` prefix
        if input.peek(syn::Ident) {
            let ident: syn::Ident = input.parse()?;
            if ident == "file" {
                input.parse::<Token![:]>()?;
                let path: LitStr = input.parse()?;
                return Ok(MarkdownInput::File(path.value()));
            } else {
                return Err(syn::Error::new(
                    ident.span(),
                    "expected string literal or `file: \"path\"`",
                ));
            }
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
#[proc_macro]
pub fn markdown_html(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as MarkdownInput);

    let markdown = match input {
        MarkdownInput::Literal(s) => s,
        MarkdownInput::File(relative_path) => {
            // Get the workspace root from CARGO_MANIFEST_DIR
            // For workspace members, this points to the member crate, so we
            // need to go up to find the workspace root where docs/ lives
            let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")
                .expect("CARGO_MANIFEST_DIR not set");

            // Try the path relative to manifest dir first, then try going up
            // one level (for workspace members)
            let manifest_path = PathBuf::from(&manifest_dir);
            let direct_path = manifest_path.join(&relative_path);
            let workspace_path = manifest_path.join("..").join(&relative_path);

            let full_path = if direct_path.exists() {
                direct_path
            } else if workspace_path.exists() {
                workspace_path
            } else {
                return syn::Error::new(
                    proc_macro2::Span::call_site(),
                    format!(
                        "File not found: {} (tried {} and {})",
                        relative_path,
                        direct_path.display(),
                        workspace_path.display()
                    ),
                )
                .to_compile_error()
                .into();
            };

            match std::fs::read_to_string(&full_path) {
                Ok(content) => content,
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
            }
        }
    };

    let html_output = markdown_to_html(&markdown);

    let expanded = quote! {
        #html_output
    };

    expanded.into()
}

fn markdown_to_html(markdown: &str) -> String {
    let options = Options::ENABLE_TABLES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_SMART_PUNCTUATION;

    let parser = Parser::new_ext(markdown, options);
    let mut html_output = String::new();
    html::push_html(&mut html_output, parser);

    html_output
}
