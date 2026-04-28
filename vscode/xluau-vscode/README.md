# XLuau VS Code Extension

This extension adds:

- `.xl` language registration
- syntax highlighting
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

- `<workspace>/target/debug/xluau-lsp(.exe)`
- `xluau-lsp` on your `PATH`

You can override that with the `xluau.server.path` setting.
