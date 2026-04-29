# XLuau

Luau, with the holes filled in.

XLuau is a Luau superset that compiles to readable Luau with zero runtime dependency.

## Docs

Start in [docs/README.md](./docs/README.md).

Recommended reading order:

1. [Getting Started](./docs/getting-started.md)
2. [Language Tour](./docs/language-tour.md)
3. [Feature Status](./docs/feature-status.md)
4. Topic guides in [`docs/guides`](./docs/guides)

## Current Compiler Status

Implemented today:

- Smarter module resolution
- Nullish coalescing
- Optional chaining
- Ternary expressions
- Pipes
- `const`
- Destructuring
- `switch`
- `match`
- `enum`
- Table comprehensions
- `do`-expressions
- `fmt` and `run` CLI commands
- `xlpkg` package install/bundle workflow
- Baseline LSP diagnostics, formatting, and symbols
- VS Code language support scaffold

Planned features are also documented, and the docs clearly label what is fully implemented today versus what is still roadmap work.
