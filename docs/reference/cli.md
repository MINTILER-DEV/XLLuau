# CLI Reference

This page documents the CLI supported by the current repository.

## Command Overview

```bash
xluau build [path] [--watch]
xluau check [path] [--watch]
xluau fmt [path] [--check]
xluau run <path> [--runtime <cmd>] [args...]
xluau remap <stacktrace>
```

## `build`

Compile XLuau and write `.luau` output.

### Build the whole project

```bash
cargo run -- build
```

### Build a project from a directory path

```bash
cargo run -- build tests/module_projects/custom_alias
```

### Build a project from its config file

```bash
cargo run -- build tests/module_projects/custom_alias/xluau.config.json
```

### Build one source file

```bash
cargo run -- build src/main.xl
```

### Watch for changes

```bash
cargo run -- build --watch
```

## `check`

Compile and validate without writing output files.

### Check the whole project

```bash
cargo run -- check
```

### Check a project from a directory path

```bash
cargo run -- check tests/module_projects/custom_alias
```

### Check a project from its config file

```bash
cargo run -- check tests/module_projects/custom_alias/xluau.config.json
```

### Check one source file

```bash
cargo run -- check src/main.xl
```

### Watch while checking

```bash
cargo run -- check --watch
```

## `remap`

Translate Luau stack traces back to XLuau source locations by consulting adjacent `.luau.map` files.

```bash
cargo run -- remap stacktrace.txt
```

## `fmt`

Format XLuau and Luau source files in place.

### Format a whole project tree

```bash
cargo run -- fmt
```

### Format a directory

```bash
cargo run -- fmt tests/projects
```

### Format one file

```bash
cargo run -- fmt src/main.xl
```

### Check formatting without rewriting files

```bash
cargo run -- fmt --check
```

## `run`

Compile an entry file and execute the emitted Luau with a runtime command.

### Run a source file directly

```bash
cargo run -- run src/main.xl
```

### Run a project directory

```bash
cargo run -- run tests/module_projects/custom_alias
```

### Override the runtime executable

```bash
cargo run -- run src/main.xl --runtime luau
```

### Pass arguments through to the runtime

```bash
cargo run -- run src/main.xl -- --flag value
```

## Current Behavior

- If `path` is omitted, uses the current working directory as the project root
- If `path` is a directory, treats that directory as the project root
- If `path` is `xluau.config.json`, treats that file's parent directory as the project root
- If `path` is a source file, searches upward from that file for the nearest `xluau.config.json`
- Falls back to the current working directory if a single-file build does not live inside a discovered project
- Builds matching files for project builds
- Writes output under `outDir` for `build`
- Writes `.luau.map` files when `sourceMaps` is enabled
- Validates generated Luau
- Reports semantic and parsing errors
- Polls for file changes when `--watch` is enabled
- `fmt` formats `.xl`, `.luau`, and `.lua` files
- `fmt --check` reports files that need formatting and exits non-zero
- `run` resolves entry files from a source file, a project directory, or a config path
- `run` looks for `src/main.xl`, `src/main.luau`, `main.xl`, then `main.luau` when given a project path
- `run` uses `--runtime`, then `XLUAU_RUNTIME`, then `luau`
