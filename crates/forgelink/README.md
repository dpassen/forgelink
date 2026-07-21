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
forgelink = "0.3"
```

```rust
use std::num::NonZero;
use std::path::Path;
use forgelink::{build_link, Lines, RefSpec};

let path = Path::new(".");
let lines = Lines::range(NonZero::new(1).unwrap(), NonZero::new(5).unwrap())?;
let url = build_link(
    path,
    "origin",
    "src/main.rs",
    Some(lines),
    RefSpec::Commit,
    |_| None,
)?;
println!("{url}");
```

The final closure receives the hostname parsed from the Git remote. Returning
`None` uses automatic forge detection and an HTTPS base URL for that host.
`project_link` uses the same convention.

Supply a [`ForgeTarget`](https://docs.rs/forgelink/latest/forgelink/struct.ForgeTarget.html)
to override both the web destination and URL format:

```rust
use std::path::Path;
use forgelink::{build_link, Forge, ForgeTarget, RefSpec};

let target = ForgeTarget::new("https://company.example/services/gitlab", Forge::GitLab)?;
let url = build_link(
    Path::new("."),
    "origin",
    "src/main.rs",
    None,
    RefSpec::Commit,
    move |host| (host == "internal").then_some(target),
)?;
println!("{url}");
```

The library never discovers or reads forgelink CLI configuration. Callers own
host mapping policy and provide already validated targets through the closure.

## License

Licensed under either of [MIT](https://github.com/dpassen/forgelink/blob/main/LICENSE-MIT) or [Apache-2.0](https://github.com/dpassen/forgelink/blob/main/LICENSE-APACHE) at your option.
