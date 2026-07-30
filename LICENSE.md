# License

Copyright (c) 2025 10log10

TinyLVT is licensed per crate, under two licenses.

The `payloads/` crate is licensed MIT (see [LICENSE-MIT](LICENSE-MIT)). It contains the shared types and API client, permissively licensed so that any software, including closed-source software such as self-hosted bidding agents and integrations, can build against TinyLVT instances.

All other crates (`api/`, `ui/`, `dev-server/`, `test-helpers/`, `markdown-html/`, `screenshots/`, `prerender/`) are licensed AGPL-3.0-only (see [LICENSE-AGPL](LICENSE-AGPL)). An auction is only legitimate if participants can inspect the rules they are bidding under. The AGPL keeps that true for every version of TinyLVT, not just this one: anyone who operates a modified instance must offer its users the source. No fork of this mechanism can be a black box.

Everything else in this repository (documentation, scripts, configuration, and any other files outside the crates listed above) is licensed AGPL-3.0-only unless a file states otherwise. As an exception, the top-level build and tooling configuration (`Cargo.toml`, `Cargo.lock`, `rustfmt.toml`) is MIT, so that it can travel with the MIT-licensed `payloads` crate (published crates inline the workspace manifest's shared dependency declarations).

Each crate's `Cargo.toml` declares its license, and crates published to crates.io include the license text. All contributions are accepted under MIT regardless of crate; see [CONTRIBUTING.md](CONTRIBUTING.md).
