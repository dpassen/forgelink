# forgelink

![forgelink banner](https://raw.githubusercontent.com/dpassen/forgelink/main/assets/forgelink-banner.svg)

[![crates.io](https://img.shields.io/crates/v/forgelink.svg)](https://crates.io/crates/forgelink)

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
use std::num::NonZero;
use std::path::Path;
use forgelink::{build_link, Lines, RefSpec};

let path = Path::new(".");
let lines = Lines::range(NonZero::new(1).unwrap(), NonZero::new(5).unwrap())?;
let url = build_link(path, "origin", "src/main.rs", Some(lines), RefSpec::Commit)?;
println!("{url}");
```

## License

Licensed under either of [MIT](https://github.com/dpassen/forgelink/blob/main/LICENSE-MIT) or [Apache-2.0](https://github.com/dpassen/forgelink/blob/main/LICENSE-APACHE) at your option.
