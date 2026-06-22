# forgelink

Forge detection and URL generation for hosted git repositories.

This is the library crate behind the [`forgelink`](https://crates.io/crates/forgelink-cli) CLI tool.

## Supported Forges

- GitHub
- GitLab
- SourceHut
- Bitbucket
- Codeberg (including forge.fedoraproject.org)

## Usage

```toml
[dependencies]
forgelink = "0.1"
```

```rust
use std::path::Path;
use forgelink::{build_link, resolve_ref};

let path = Path::new(".");
let git_ref = resolve_ref(path)?;
let url = build_link(path, "origin", git_ref, "src/main.rs", None, None)?;
println!("{url}");
```

## License

Licensed under either of [MIT](https://github.com/dpassen/forgelink/blob/main/LICENSE-MIT) or [Apache-2.0](https://github.com/dpassen/forgelink/blob/main/LICENSE-APACHE) at your option.
