# Tooling and Project Setup

This page covers what the current repository supports today.

## Compiler Commands

### Build the whole project

```bash
cargo run -- build
```

This reads `xluau.config.json`, compiles matching `.xl` files, and writes `.luau` output under `outDir`.

### Build one file

```bash
cargo run -- build src/main.xl
```

### Check without writing files

```bash
cargo run -- check
```

### Check one file

```bash
cargo run -- check src/main.xl
```

## File Extensions

- `.xl`: XLuau source
- `.luau`: generated output, or plain Luau input
- `.lua`: also supported as a module resolution extension in config

## Current Config File

Create `xluau.config.json` in the project root.

Example:

```json
{
  "include": ["src/**/*.xl"],
  "exclude": [],
  "outDir": "out",
  "target": "filesystem",
  "baseDir": "src",
  "paths": {
    "@shared": "./src/shared"
  },
  "extensions": [".xl", ".luau", ".lua"],
  "indexFiles": ["init"]
}
```

## Current Targets

### `filesystem`

Emits string-style `require(...)` paths such as:

```lua
require("./src/shared/math")
```

### `roblox`

Emits instance-path requires such as:

```lua
require(script.Parent.Parent.shared.math)
```

### `custom`

Calls your configured resolver function:

```lua
require(resolveModule("shared/math"))
```

## Suggested Workflow

1. Write `.xl` files under `src/`.
2. Keep reusable modules under aliased folders like `src/shared`.
3. Run `cargo run -- check` while iterating.
4. Run `cargo run -- build` when you want emitted output.
5. Treat `out/` as generated code.

## What Is Still Planned

The spec also describes future tooling like watch mode, LSP, and editor integrations. Those are part of the language roadmap, but they are not all present in the current codebase yet.
