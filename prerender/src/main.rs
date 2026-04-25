//! Static HTML generator for SEO-friendly pages.
//!
//! Generates pre-rendered HTML for the landing page and docs pages.
//! These are served directly by nginx for fast initial load and SEO,
//! then replaced by the Yew app when WASM loads.

use anyhow::{Context, Result};
use pulldown_cmark::{Options, Parser, html};
use std::fs;
use std::path::Path;

/// Page metadata for SEO
struct PageMeta {
    title: &'static str,
    description: &'static str,
    path: &'static str,
}

/// How to wrap the rendered markdown content.
#[derive(Clone, Copy)]
enum PageLayout {
    /// Docs page with the docs sidebar visible.
    DocsWithSidebar,
    /// No wrapping; the content provides its own layout.
    Raw,
}

/// Docs page definition (uses the sidebar layout)
struct DocsPage {
    meta: PageMeta,
    markdown_file: &'static str,
    output_file: &'static str,
}

/// A standalone markdown page without the docs sidebar.
/// Used for longer-form narratives and the terms page.
struct MarkdownPage {
    meta: PageMeta,
    markdown_file: &'static str,
    output_file: &'static str,
    /// Tailwind max-width class for the content container
    /// (e.g. "max-w-3xl" for terms, "max-w-4xl" for the guide).
    max_width_class: &'static str,
}

const TERMS_PAGE: PageMeta = PageMeta {
    title: "Terms of Service - TinyLVT",
    description: "Terms of service for TinyLVT, \
        operated by Aperture Beam Technologies, Inc.",
    path: "/terms",
};

const LANDING_PAGE: PageMeta = PageMeta {
    title: "TinyLVT - Fair Allocation for Anything Shared",
    description: "TinyLVT uses auctions to allocate anything people share. \
        From splitting rent among housemates to assigning desks in a workspace, \
        everyone pays according to actual demand.",
    path: "/",
};

const DOCS_PAGES: &[DocsPage] = &[
    DocsPage {
        meta: PageMeta {
            title: "Getting Started - TinyLVT",
            description: "Learn how TinyLVT helps communities allocate shared \
                spaces fairly through auction-based allocation.",
            path: "/docs",
        },
        markdown_file: "getting-started.md",
        output_file: "docs/index.html",
    },
    DocsPage {
        meta: PageMeta {
            title: "Currency Modes - TinyLVT",
            description: "Understand TinyLVT's currency options: internal \
                points, IOUs between members, or prepaid credits.",
            path: "/docs/currency",
        },
        markdown_file: "currency.md",
        output_file: "docs/currency/index.html",
    },
    DocsPage {
        meta: PageMeta {
            title: "Community Setup - TinyLVT",
            description: "Create your community, sites, and spaces in TinyLVT. \
                Step-by-step guide to getting started.",
            path: "/docs/setup",
        },
        markdown_file: "setup.md",
        output_file: "docs/setup/index.html",
    },
    DocsPage {
        meta: PageMeta {
            title: "Auctions - TinyLVT",
            description: "Learn how simultaneous ascending auctions work in \
                TinyLVT, including bidding, pricing, and allocation.",
            path: "/docs/auctions",
        },
        markdown_file: "auctions.md",
        output_file: "docs/auctions/index.html",
    },
    DocsPage {
        meta: PageMeta {
            title: "Desk Allocation Example - TinyLVT",
            description: "How to use TinyLVT for fair desk allocation in \
                shared workspaces like coworking spaces or academic labs.",
            path: "/docs/desk-allocation",
        },
        markdown_file: "desk-allocation.md",
        output_file: "docs/desk-allocation/index.html",
    },
    DocsPage {
        meta: PageMeta {
            title: "Rent Splitting Example - TinyLVT",
            description: "How to use TinyLVT for fair rent splitting among \
                housemates based on room preferences and values.",
            path: "/docs/rent-splitting",
        },
        markdown_file: "rent-splitting.md",
        output_file: "docs/rent-splitting/index.html",
    },
];

const MARKDOWN_PAGES: &[MarkdownPage] = &[
    MarkdownPage {
        meta: TERMS_PAGE,
        markdown_file: "terms.md",
        output_file: "terms/index.html",
        max_width_class: "max-w-3xl",
    },
    MarkdownPage {
        meta: PageMeta {
            title: "Interactive Guide to Cooperative Auctions - TinyLVT",
            description: "An interactive guide to how cooperative auctions \
                work, with live simulations covering rent splitting, desk \
                allocation, and group decisions.",
            path: "/auction-guide",
        },
        markdown_file: "auction-guide.md",
        output_file: "auction-guide/index.html",
        max_width_class: "max-w-4xl",
    },
];

/// Assets extracted from Trunk's index.html output
struct TrunkAssets {
    /// The CSS link tag (includes integrity hash)
    css_link: String,
    /// The modulepreload and preload links for JS/WASM
    preload_links: String,
    /// The module script that loads WASM
    module_script: String,
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: {} <docs_dir> <output_dir>", args[0]);
        eprintln!(
            "  docs_dir: Path to the docs/ directory with markdown files"
        );
        eprintln!(
            "  output_dir: Path to Trunk's dist/ directory (reads index.html, \
                writes pre-rendered pages)"
        );
        std::process::exit(1);
    }

    let docs_dir = Path::new(&args[1]);
    let output_dir = Path::new(&args[2]);

    // Extract CSS and JS from Trunk's generated index.html
    let assets = extract_trunk_assets(output_dir)?;

    println!("Generating pre-rendered HTML pages...");
    println!("  Docs dir: {}", docs_dir.display());
    println!("  Output dir: {}", output_dir.display());

    // Generate landing page
    generate_landing_page(output_dir, &assets)?;

    // Generate standalone markdown pages (terms, auction guide, etc.)
    for page in MARKDOWN_PAGES {
        generate_markdown_page(docs_dir, output_dir, page, &assets)?;
    }

    // Generate docs pages (with sidebar)
    for page in DOCS_PAGES {
        generate_docs_page(docs_dir, output_dir, page, &assets)?;
    }

    let total = DOCS_PAGES.len() + MARKDOWN_PAGES.len() + 1;
    println!("Done! Generated {} pages.", total);
    Ok(())
}

/// Extract CSS link and module script from Trunk's index.html
fn extract_trunk_assets(output_dir: &Path) -> Result<TrunkAssets> {
    let index_path = output_dir.join("index.html");
    let index_html = fs::read_to_string(&index_path)
        .with_context(|| format!("Failed to read {}", index_path.display()))?;

    // Extract the CSS link tag
    let css_link =
        extract_inclusive(&index_html, "<link rel=\"stylesheet\"", "/>")
            .context("Could not find CSS link in index.html")?;

    // Extract modulepreload and preload links for JS/WASM
    let preload_links = extract_preload_links(&index_html);

    // Extract the module script
    let module_script =
        extract_inclusive(&index_html, "<script type=\"module\">", "</script>")
            .context("Could not find module script in index.html")?;

    Ok(TrunkAssets {
        css_link,
        preload_links,
        module_script,
    })
}

/// Extract all link tags matching a given rel attribute
fn extract_all_links(html: &str, rel: &str) -> Vec<String> {
    let prefix = format!("<link rel=\"{}\"", rel);
    let mut links = Vec::new();
    let mut search_from = 0;

    while let Some(start) = html[search_from..].find(&prefix) {
        let abs_start = search_from + start;
        if let Some(tag) = extract_inclusive(&html[abs_start..], &prefix, ">") {
            search_from = abs_start + tag.len();
            links.push(tag);
        } else {
            break;
        }
    }

    links
}

/// Extract all modulepreload and preload link tags
fn extract_preload_links(html: &str) -> String {
    let mut links = extract_all_links(html, "modulepreload");
    links.extend(extract_all_links(html, "preload"));
    links.join("\n    ")
}

/// Extract a substring including the start and end markers
fn extract_inclusive(haystack: &str, start: &str, end: &str) -> Option<String> {
    let start_idx = haystack.find(start)?;
    let after_start = start_idx + start.len();
    let end_idx = haystack[after_start..].find(end)?;
    Some(haystack[start_idx..after_start + end_idx + end.len()].to_string())
}

/// Generate the landing page HTML
fn generate_landing_page(
    output_dir: &Path,
    assets: &TrunkAssets,
) -> Result<()> {
    let content = landing_page_content();
    let html = render_page(&LANDING_PAGE, &content, assets, PageLayout::Raw);

    let output_path = output_dir.join("landing.html");
    fs::write(&output_path, html)?;
    println!("  Generated: {}", output_path.display());

    Ok(())
}

/// Generate a standalone markdown page (no docs sidebar)
fn generate_markdown_page(
    docs_dir: &Path,
    output_dir: &Path,
    page: &MarkdownPage,
    assets: &TrunkAssets,
) -> Result<()> {
    let markdown_path = docs_dir.join(page.markdown_file);
    let markdown = fs::read_to_string(&markdown_path).with_context(|| {
        format!("Failed to read {}", markdown_path.display())
    })?;
    let content = format!(
        r#"<div class="{} mx-auto px-4 py-8"><div class="prose dark:prose-invert max-w-none">{}</div></div>"#,
        page.max_width_class,
        render_markdown(&markdown),
    );
    let html = render_page(&page.meta, &content, assets, PageLayout::Raw);

    let output_path = output_dir.join(page.output_file);
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&output_path, html)?;
    println!("  Generated: {}", output_path.display());

    Ok(())
}

/// Generate a docs page HTML
fn generate_docs_page(
    docs_dir: &Path,
    output_dir: &Path,
    page: &DocsPage,
    assets: &TrunkAssets,
) -> Result<()> {
    let markdown_path = docs_dir.join(page.markdown_file);
    let markdown = fs::read_to_string(&markdown_path).with_context(|| {
        format!("Failed to read {}", markdown_path.display())
    })?;

    let content = render_markdown(&markdown);
    let html =
        render_page(&page.meta, &content, assets, PageLayout::DocsWithSidebar);

    let output_path = output_dir.join(page.output_file);
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&output_path, html)?;
    println!("  Generated: {}", output_path.display());

    Ok(())
}

/// Render markdown to HTML
fn render_markdown(markdown: &str) -> String {
    let options = Options::ENABLE_TABLES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_HEADING_ATTRIBUTES
        | Options::ENABLE_SMART_PUNCTUATION;
    let parser = Parser::new_ext(markdown, options);
    let mut html_output = String::new();
    html::push_html(&mut html_output, parser);
    html_output
}

/// Render a complete HTML page
fn render_page(
    meta: &PageMeta,
    content: &str,
    assets: &TrunkAssets,
    layout: PageLayout,
) -> String {
    let content_wrapper = match layout {
        PageLayout::DocsWithSidebar => format!(
            r#"<div class="flex min-h-0">
<aside class="hidden md:block w-64 flex-shrink-0 border-r border-neutral-200 dark:border-neutral-700 bg-white dark:bg-neutral-900">
<nav class="py-4">
<div class="px-4 pb-2 text-xs font-semibold uppercase tracking-wider text-neutral-500 dark:text-neutral-400">Documentation</div>
<ul>
<li><a href="/docs" class="block px-4 py-2 text-sm transition-colors text-neutral-600 dark:text-neutral-400 hover:text-neutral-900 dark:hover:text-white hover:bg-neutral-100 dark:hover:bg-neutral-800">Getting Started</a></li>
<li><a href="/docs/currency" class="block px-4 py-2 text-sm transition-colors text-neutral-600 dark:text-neutral-400 hover:text-neutral-900 dark:hover:text-white hover:bg-neutral-100 dark:hover:bg-neutral-800">Currency Modes</a></li>
<li><a href="/docs/setup" class="block px-4 py-2 text-sm transition-colors text-neutral-600 dark:text-neutral-400 hover:text-neutral-900 dark:hover:text-white hover:bg-neutral-100 dark:hover:bg-neutral-800">Community Setup</a></li>
<li><a href="/docs/auctions" class="block px-4 py-2 text-sm transition-colors text-neutral-600 dark:text-neutral-400 hover:text-neutral-900 dark:hover:text-white hover:bg-neutral-100 dark:hover:bg-neutral-800">Auctions</a></li>
<li><a href="/docs/desk-allocation" class="block px-4 py-2 text-sm transition-colors text-neutral-600 dark:text-neutral-400 hover:text-neutral-900 dark:hover:text-white hover:bg-neutral-100 dark:hover:bg-neutral-800">Desk Allocation</a></li>
<li><a href="/docs/rent-splitting" class="block px-4 py-2 text-sm transition-colors text-neutral-600 dark:text-neutral-400 hover:text-neutral-900 dark:hover:text-white hover:bg-neutral-100 dark:hover:bg-neutral-800">Rent Splitting</a></li>
</ul>
</nav>
</aside>
<div class="flex-1 min-w-0">
<div class="max-w-4xl mx-auto px-4 py-8">
<div class="prose dark:prose-invert max-w-none">
{}
</div>
</div>
</div>
</div>"#,
            content
        ),
        PageLayout::Raw => content.to_string(),
    };

    format!(
        r##"<!DOCTYPE html>
<html lang="en" class="dark">
<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1, maximum-scale=1.0">
    <title>{title}</title>
    <meta name="description" content="{description}">

    <!-- Open Graph -->
    <meta property="og:type" content="website">
    <meta property="og:title" content="{title}">
    <meta property="og:description" content="{description}">
    <meta property="og:url" content="https://tinylvt.com{path}">
    <meta property="og:site_name" content="TinyLVT">

    <!-- Twitter Card -->
    <meta name="twitter:card" content="summary">
    <meta name="twitter:title" content="{title}">
    <meta name="twitter:description" content="{description}">

    <link rel="icon" type="image/svg+xml" href="/favicon.svg">
    {css_link}
    {preload_links}
    <script>
        // Set dark mode immediately to prevent flash
        (function() {{
            const storedTheme = localStorage.getItem('theme-mode');
            if (storedTheme === 'light' ||
                (storedTheme !== 'dark' && !window.matchMedia('(prefers-color-scheme: dark)').matches)) {{
                document.documentElement.classList.remove('dark');
            }}
        }})();
    </script>
</head>
<body>
<div class="min-h-screen bg-white dark:bg-neutral-900 text-neutral-900 dark:text-neutral-100 transition-colors flex flex-col">
<header class="bg-white dark:bg-neutral-900 border-b border-neutral-200 dark:border-neutral-700">
<div class="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8">
<div class="flex flex-wrap justify-between items-center gap-2 min-h-16 py-2">
<div class="flex items-center gap-4 sm:gap-8">
<a href="/" class="text-xl font-black text-neutral-900 dark:text-white hover:text-neutral-700 dark:hover:text-neutral-300">TinyLVT</a>
<nav class="flex gap-4 sm:gap-6">
<a href="/docs" class="text-sm text-neutral-600 dark:text-neutral-400 hover:text-neutral-900 dark:hover:text-white">Docs</a>
</nav>
</div>
<div class="flex items-center gap-2 sm:gap-4">
<a href="/login" class="text-sm text-neutral-600 dark:text-neutral-400 hover:text-neutral-900 dark:hover:text-white border border-neutral-300 dark:border-neutral-600 px-3 py-1 rounded-md hover:bg-neutral-100 dark:hover:bg-neutral-800 transition-colors">Login</a>
<div class="p-2 rounded-md"><div class="w-5 h-5"></div></div>
</div>
</div>
</div>
</header>
<main class="w-full max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-8 flex-grow">
{content}
</main>
<footer class="bg-white dark:bg-neutral-900 border-t border-neutral-200 dark:border-neutral-700 mt-auto">
<div class="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-4 space-y-2">
<div class="flex flex-wrap justify-center gap-x-6 gap-y-1">
<a href="/terms" class="text-sm text-neutral-500 dark:text-neutral-400 hover:text-neutral-900 dark:hover:text-white">Terms</a>
<a href="https://github.com/10log10/tinylvt" target="_blank" rel="noopener noreferrer" class="text-sm text-neutral-500 dark:text-neutral-400 hover:text-neutral-900 dark:hover:text-white">Source</a>
<a href="mailto:info@aperturebeam.com" class="text-sm text-neutral-500 dark:text-neutral-400 hover:text-neutral-900 dark:hover:text-white">Contact</a>
</div>
<p class="text-xs text-neutral-400 dark:text-neutral-500 text-center">Aperture Beam Technologies, Inc.</p>
</div>
</footer>
</div>
{module_script}
</body>
</html>"##,
        title = meta.title,
        description = meta.description,
        path = meta.path,
        css_link = assets.css_link,
        preload_links = assets.preload_links,
        module_script = assets.module_script,
        content = content_wrapper,
    )
}

/// Static landing page content (mirrors LoggedOutHomePage component)
fn landing_page_content() -> String {
    r##"<div class="space-y-16">
<div class="flex flex-col lg:flex-row gap-10 lg:items-start max-w-7xl mx-auto">
<div class="lg:w-5/12 space-y-8 pt-12">
<p class="text-4xl sm:text-3xl font-semibold text-neutral-900 dark:text-neutral-100">Fair allocation for anything&nbsp;shared.</p>
<p class="text-lg text-neutral-600 dark:text-neutral-400">When housemates share a rental, who gets the master bedroom? When a team shares an office, who gets the window desk? Traditional methods leave someone feeling shortchanged.</p>
<p class="text-lg text-neutral-600 dark:text-neutral-400">TinyLVT uses auctions to allocate spaces fairly. Everyone bids what each space is worth to them. Spaces go to those who value them most, and the proceeds are shared equally.</p>
<div class="bg-neutral-100 dark:bg-neutral-800 rounded-lg p-6 border border-neutral-200 dark:border-neutral-700">
<p class="text-lg font-medium text-neutral-900 dark:text-neutral-100">You only pay what others would have paid.</p>
<p class="text-neutral-600 dark:text-neutral-400 mt-2">If you win a space, you pay just enough to outbid the next-highest bidder—not your maximum. This encourages honest bidding and ensures fair prices.</p>
</div>
<div class="flex flex-col sm:flex-row gap-4 justify-center">
<button class="inline-block px-8 py-3 text-lg font-semibold text-white bg-neutral-900 hover:bg-neutral-700 dark:bg-neutral-100 dark:text-neutral-900 dark:hover:bg-neutral-300 rounded transition-colors">Sign Up</button>
<button class="inline-block px-8 py-3 text-lg font-semibold text-neutral-900 dark:text-neutral-100 border-2 border-neutral-900 dark:border-neutral-100 hover:bg-neutral-100 dark:hover:bg-neutral-800 rounded transition-colors">Learn How It Works</button>
</div>
</div>
<div class="lg:w-7/12">
<!-- Placeholder for interactive auction demo (rendered by Yew).
     Min-height sized for the rent-splitting scenario at max
     width (~800px) to minimize layout shift when WASM hydrates. -->
<div class="lg:min-h-[77.63rem]"></div>
</div>
</div>
<div class="max-w-3xl mx-auto space-y-4">
<h2 class="text-2xl font-semibold text-neutral-900 dark:text-neutral-100">Why "LVT"?</h2>
<p class="text-lg text-neutral-600 dark:text-neutral-400">TinyLVT is based on the principles of land value taxation (LVT). Land value taxes:</p>
<ul class="space-y-2 text-lg text-neutral-600 dark:text-neutral-400 list-disc pl-6">
<li>allocate scarce resources <span class="italic">("land")</span></li>
<li>by assessing their rental value <span class="italic">("value")</span></li>
<li>and capturing and redistributing that value to the community <span class="italic">("tax")</span></li>
</ul>
<p class="text-lg text-neutral-600 dark:text-neutral-400">Land value taxes ensure resources are used well and guarantee equal access, even if the resource possession itself is unequal. The redistribution compensates those who are excluded from the resource for their share of its value.</p>
<p class="text-lg text-neutral-600 dark:text-neutral-400">TinyLVT is a pure implementation of land value taxation. Resource value and allocation are precisely determined with auctions. Distributions are direct payments to each community member.</p>
</div>
<div class="max-w-5xl mx-auto space-y-8">
<div class="text-center space-y-4">
<h2 class="text-2xl font-semibold text-neutral-900 dark:text-neutral-100">The auction interface</h2>
</div>
<div class="hidden lg:block">
<div style="aspect-ratio: 2740 / 3507;" class="w-full rounded-lg"></div>
</div>
<div class="lg:hidden">
<div style="aspect-ratio: 1640 / 3507;" class="w-full rounded-lg"></div>
</div>
</div>
<div class="max-w-5xl mx-auto">
<h2 class="text-2xl font-semibold text-neutral-900 dark:text-neutral-100 mb-6 text-center">Examples</h2>
<div class="grid grid-cols-1 md:grid-cols-3 gap-6">
<div class="bg-neutral-50 dark:bg-neutral-800/50 rounded-lg p-6 border border-neutral-200 dark:border-neutral-700">
<h3 class="text-lg font-semibold text-neutral-900 dark:text-neutral-100 mb-3">Splitting rent</h3>
<p class="text-neutral-600 dark:text-neutral-400 text-sm mb-4">Three housemates, three rooms, $3,000/month total rent. The auction determines assignments and rent adjustments:</p>
<div class="space-y-2 text-sm">
<div class="flex justify-between"><span class="text-neutral-600 dark:text-neutral-400">Alice — Master bedroom</span><span class="font-medium text-neutral-900 dark:text-neutral-100">$1,150</span></div>
<div class="flex justify-between"><span class="text-neutral-600 dark:text-neutral-400">Bob — Middle room</span><span class="font-medium text-neutral-900 dark:text-neutral-100">$950</span></div>
<div class="flex justify-between"><span class="text-neutral-600 dark:text-neutral-400">Carol — Small room</span><span class="font-medium text-neutral-900 dark:text-neutral-100">$900</span></div>
</div>
<p class="text-neutral-500 dark:text-neutral-500 text-xs mt-4">Everyone pays according to room value. Total: still $3,000.</p>
</div>
<div class="bg-neutral-50 dark:bg-neutral-800/50 rounded-lg p-6 border border-neutral-200 dark:border-neutral-700">
<h3 class="text-lg font-semibold text-neutral-900 dark:text-neutral-100 mb-3">Allocating desks</h3>
<p class="text-neutral-600 dark:text-neutral-400 text-sm mb-4">20 grad students, 14 desks. Each term:</p>
<ul class="space-y-2 text-sm text-neutral-600 dark:text-neutral-400">
<li class="flex items-start gap-2"><span class="text-neutral-400">1.</span><span>Everyone receives 100 points</span></li>
<li class="flex items-start gap-2"><span class="text-neutral-400">2.</span><span>Bid on desks you want</span></li>
<li class="flex items-start gap-2"><span class="text-neutral-400">3.</span><span>Winners pay points; others save theirs</span></li>
</ul>
<p class="text-neutral-500 dark:text-neutral-500 text-xs mt-4">Fair allocation without real money changing hands.</p>
</div>
<div class="bg-neutral-50 dark:bg-neutral-800/50 rounded-lg p-6 border border-neutral-200 dark:border-neutral-700">
<h3 class="text-lg font-semibold text-neutral-900 dark:text-neutral-100 mb-3">Assigning market stalls</h3>
<p class="text-neutral-600 dark:text-neutral-400 text-sm mb-4">A street fair with 30 vendor spots. Corner stalls have extra frontage; spots near the entrance get more foot traffic.</p>
<ul class="space-y-2 text-sm text-neutral-600 dark:text-neutral-400">
<li class="flex items-start gap-2"><span class="text-neutral-400">•</span><span>Vendors bid on preferred spots</span></li>
<li class="flex items-start gap-2"><span class="text-neutral-400">•</span><span>Prime locations cost more</span></li>
<li class="flex items-start gap-2"><span class="text-neutral-400">•</span><span>Revenue offsets event costs</span></li>
</ul>
<p class="text-neutral-500 dark:text-neutral-500 text-xs mt-4">Market-based pricing without awkward negotiations.</p>
</div>
</div>
</div>
<div class="py-4 flex flex-col sm:flex-row gap-4 justify-center">
<button class="inline-block px-8 py-3 text-lg font-semibold text-white bg-neutral-900 hover:bg-neutral-700 dark:bg-neutral-100 dark:text-neutral-900 dark:hover:bg-neutral-300 rounded transition-colors">Sign Up</button>
<button class="inline-block px-8 py-3 text-lg font-semibold text-neutral-900 dark:text-neutral-100 border-2 border-neutral-900 dark:border-neutral-100 hover:bg-neutral-100 dark:hover:bg-neutral-800 rounded transition-colors">Learn How It Works</button>
</div>
<div class="max-w-2xl mx-auto">
<div class="flex flex-col sm:flex-row gap-6 justify-center text-center">
<div class="flex-1 invisible"><p class="text-3xl font-bold">&nbsp;</p><p class="text-sm mt-1">&nbsp;</p></div>
<div class="flex-1 invisible"><p class="text-3xl font-bold">&nbsp;</p><p class="text-sm mt-1">&nbsp;</p></div>
</div>
</div>
</div>"##.to_string()
}
