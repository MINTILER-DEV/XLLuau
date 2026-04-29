# XLuau VS Code Extension

This extension adds:

- `.xl` language registration
- syntax highlighting
- completions
- hover
- go to definition
- rename
- quick fixes
- formatting through `xluau-lsp`
- diagnostics through `xluau-lsp`

## Local Development

1. Build the Rust language server:

```bash
cargo build --bin xluau-lsp
```

2. Install the extension dependencies:

```bash
npm install
```

3. Open this folder in VS Code and press `F5`.

## Server Path

By default the extension tries:

- the nearest ancestor workspace folder that contains `target/debug/xluau-lsp(.exe)`
- the nearest ancestor workspace folder that contains `target/release/xluau-lsp(.exe)`
- `xluau-lsp` on your `PATH`

You can override that with the `xluau.server.path` setting.

## Current Behavior Notes

- definitions for `require("@alias/...")` jump to the resolved source file, including index files like `init.xl`
- rename currently supports current-file declaration names and project-wide `require(...)` specifier strings
- quick fixes currently cover `const` to `local`, plus fallback branches for non-exhaustive `switch` and `match`
