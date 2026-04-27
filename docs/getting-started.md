# Getting Started

XLuau is designed for Luau developers who want a little more syntax in the places where Luau is currently awkward or error-prone.

The core idea is simple:

- You write `.xl` files.
- The compiler lowers XLuau-only syntax into ordinary Luau.
- The output stays readable, debuggable, and close to the source.

## What XLuau Is

XLuau is:

- A language layer on top of Luau.
- A source-to-source compiler.
- Focused on solving real Luau pain points.
- Intentionally close to Luau style.

XLuau is not:

- A runtime framework.
- A replacement VM.
- A new object model.
- A JavaScript-like language with Luau syntax pasted on top.

## What Stays the Same

If Luau already has a good way to express something, XLuau keeps it.

That means:

- Functions still use `function ... end`.
- Blocks still use `if`, `for`, `while`, `do`, and `end`.
- Modules still use `require` and `return`.
- Tables remain the main data structure.
- Method calls still use `:`.

## Your First XLuau File

```lua
local config = {}

local timeout = config.timeout ?? 30
local role = isAdmin ? "admin" : "user"

print(timeout, role)
```

This compiles to readable Luau:

```lua
local config = {}
local _lhs0 = config.timeout
local timeout = if _lhs0 ~= nil then _lhs0 else 30
local role = if isAdmin then "admin" else "user"
print(timeout, role)
```

The output is still normal Luau code. That is the point.

## Current Implementation Status

Today, the compiler in this repository fully implements:

- Smarter module resolution
- Nullish coalescing
- Optional chaining
- Ternary expressions
- Pipe expressions
- `const`
- Destructuring
- `switch`
- `match`
- `enum`
- Table comprehensions
- `do`-expressions

The language design also includes more features that are documented here, but not all of them are implemented yet. Use [Feature Status](./feature-status.md) to check what is available right now.

## Minimal Workflow

From the repo root:

```bash
cargo run -- build
```

Compile a single file:

```bash
cargo run -- build src/main.xl
```

Check without writing output:

```bash
cargo run -- check
```

## Project Shape

A typical project looks like this:

```text
src/
  main.xl
  shared/
    init.xl
    math.xl
xluau.config.json
```

And a minimal config:

```json
{
  "include": ["src/**/*.xl"],
  "outDir": "out",
  "baseDir": "src",
  "target": "filesystem"
}
```

## How to Learn XLuau

Use this progression:

1. Learn the everyday syntax in [Expressions and Operators](./guides/expressions.md).
2. Learn [Bindings, Const, and Destructuring](./guides/bindings.md).
3. Learn [Modules and Imports](./guides/modules.md).
4. Learn [Control Flow and Data](./guides/control-flow.md).
5. Keep [Tooling and Project Setup](./tooling.md) nearby while building.

## Philosophy in One Sentence

If Luau already has syntax for it, keep Luau. If Luau has a hole, fill the hole with the smallest thing that still feels like Luau.
