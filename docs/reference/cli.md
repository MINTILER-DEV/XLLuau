# CLI Reference

This page documents the CLI supported by the current repository.

## Command Overview

```bash
xluau build [path]
xluau check [path]
```

## `build`

Compile XLuau and write `.luau` output.

### Build the whole project

```bash
cargo run -- build
```

### Build one file

```bash
cargo run -- build src/main.xl
```

## `check`

Compile and validate without writing output files.

### Check the whole project

```bash
cargo run -- check
```

### Check one file

```bash
cargo run -- check src/main.xl
```

## Current Behavior

- Reads `xluau.config.json` from the current working directory
- Builds matching files
- Writes output under `outDir` for `build`
- Validates generated Luau
- Reports semantic and parsing errors

## Planned CLI Features

The spec also discusses future tooling like watch mode and richer editor integration. Those are part of the language roadmap, but not all are available in the current CLI yet.
