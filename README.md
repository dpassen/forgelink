# forgelink

Generate shareable URLs to files and line ranges in hosted git repositories,
from the command line.

Inspired by the Emacs package [git-link](https://github.com/sshaw/git-link).

## Supported Forges

- GitHub
- GitLab
- SourceHut
- Bitbucket
- Codeberg (including forge.fedoraproject.org)

## Installation

```sh
cargo install forgelink-cli
```

This installs a binary named `forgelink`.

## Usage

By default forgelink generates a stable URL pinned to the current commit SHA:

```sh
$ forgelink src/main.rs
https://github.com/user/repo/blob/abc123def.../src/main.rs
```

Append `:N` for a single line or `:N-M` for a range:

```sh
$ forgelink src/main.rs:42
https://github.com/user/repo/blob/abc123def.../src/main.rs#L42

$ forgelink src/main.rs:42-55
https://github.com/user/repo/blob/abc123def.../src/main.rs#L42-L55
```

Target a remote other than `origin` with `--remote`:

```sh
$ forgelink --remote upstream src/main.rs
https://github.com/upstream-owner/repo/blob/abc123def.../src/main.rs
```

Use the current branch name instead of the commit SHA with `--branch`:

```sh
$ forgelink --branch src/main.rs
https://github.com/user/repo/blob/main/src/main.rs
```

`--branch` requires HEAD to be on a named branch. It errors on a detached
HEAD, which includes Jujutsu (`jj`) working copies. Use the default
commit-pinned link in that case.

Generate a link to the project homepage with `--project`:

```sh
$ forgelink --project
https://github.com/user/repo
```

Copy the URL to the clipboard with `--copy`, which still prints it to stdout
so piping keeps working:

```sh
$ forgelink --copy src/main.rs
https://github.com/user/repo/blob/abc123def.../src/main.rs
```

Open the URL in your default browser with `--open`, which also still prints
it. `--copy` and `--open` can be combined:

```sh
$ forgelink --open src/main.rs
https://github.com/user/repo/blob/abc123def.../src/main.rs
```

Clipboard and browser support are default-on cargo features (`clipboard` and
`browser`). Build with `--no-default-features` to drop the `arboard` and `open`
dependencies, which also removes the `--copy` and `--open` flags. On Linux
under X11 the clipboard is owned by the running process, so the copied URL may
not persist after forgelink exits. macOS and Windows are unaffected.

It works from any subdirectory inside the repository:

```sh
$ cd src && forgelink main.rs
https://github.com/user/repo/blob/abc123def.../src/main.rs
```

You can also pass an absolute path to link to a file in any repository,
regardless of your current directory:

```sh
$ forgelink ~/Developer/other-repo/src/main.rs
https://github.com/user/other-repo/blob/abc123def.../src/main.rs
```

## Editor integration

forgelink takes a `path:line` argument and prints a URL, so any editor that can
run a shell command with the current buffer path and cursor line can bind it
directly, with no plugin required.

### Helix

In `~/.config/helix/config.toml`:

```toml
[keys.normal.space]
o = ":sh forgelink %{file_path_absolute}:%{cursor_line} --copy"
```

`space o` copies a link to the current line. `%{file_path_absolute}` is used so
it works regardless of the directory Helix was launched from.

### Kakoune

In `~/.config/kak/kakrc`:

```kak
define-command -docstring 'copy a forge link to the current line' forge-link %{
    nop %sh{ forgelink "$kak_buffile:$kak_cursor_line" --copy }
}
map global user o ': forge-link<ret>' -docstring 'forge link'
```

### Neovim

The following is a Lua implementation for using forgelink in Neovim. It will send a link to the file path in normal mode and a link to the line(s) selected in visual mode to the system clipboard.

```lua
if vim.fn.exepath('forgelink') ~= '' then
    vim.keymap.set('n', '<leader>cf',
        function()
            local curFile = vim.api.nvim_buf_get_name(0)
            local output = vim.fn.system('forgelink ' .. curFile)
            vim.fn.setreg('+', output)
        end,
        { desc = "Copy URL to Git forge for current file to clipboard" }
    )
    vim.keymap.set('v', '<leader>cf',
        function()
            vim.api.nvim_feedkeys(vim.api.nvim_replace_termcodes("<Esc>", true, false, true), "x", false)
            local curFile = vim.api.nvim_buf_get_name(0)
            local startLine = vim.fn.line("'<")
            local endLine = vim.fn.line("'>")
            local lineRef = startLine == endLine and tostring(startLine) or (startLine .. '-' .. endLine)
            local output = vim.fn.system('forgelink ' .. curFile .. ':' .. lineRef)
            vim.fn.setreg('+', output)
        end,
        { desc = "Copy URL to Git forge for current file with selected line numbers to clipboard" }
    )
end
```

## License

Licensed under either of [MIT](https://github.com/dpassen/forgelink/blob/main/LICENSE-MIT) or [Apache-2.0](https://github.com/dpassen/forgelink/blob/main/LICENSE-APACHE) at your option.
