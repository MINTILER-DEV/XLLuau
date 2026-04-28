# Pattern Literals and Readonly Sugar

Status: implemented

- Pattern literals: implemented in the current compiler
- `readonly` and `freeze`: implemented in the current compiler

This guide covers a few planned language features that are useful to know as part of the overall XLuau design.

## Pattern Literals

Pattern literals are intended as friendlier syntax for Luau string patterns.

### Motivation

String patterns in raw quoted form are powerful, but they are not always easy to read or maintain.

### Current Behavior

The compiler supports `pattern\`...\`` and lowers it to an ordinary Luau string pattern at compile time.

Use cases include:

- Repeated captures
- Pattern constants
- More readable extraction rules

Example:

```lua
const DATE_PATTERN = pattern`{%d+}-{%d+}-{%d+}`
local year, month, day = date:match(pattern`{year:%d+}-{month:%d+}-{day:%d+}`)
```

This lowers to ordinary string literals such as `"(%d+)-(%d+)-(%d+)"`.

### Why This Fits XLuau

Pattern literals are a strong example of the overall language philosophy:

- keep Luau's underlying capability
- improve the source ergonomics
- lower to behavior Luau developers already understand

## Readonly and Freeze Sugar

Luau already has the runtime primitives. XLuau's goal here is to reduce ceremony and pair the runtime operation with the matching type information.

### Intended Syntax

```lua
readonly config: Config = {
    retries = 3,
    debug = false,
}
```

Or explicit freeze helpers that lower into `table.freeze(...)` plus matching type information:

```lua
const DEFAULTS = freeze {
    retries = 3,
    debug = false,
}
```

### Current Behavior

The current compiler supports:

- target-specific readonly field emit
- `freeze { ... }` lowering to `table.freeze({ ... })`
- utility-type expansion such as `Readonly<typeof(DEFAULTS)>`

On `new-solver` targets, readonly fields emit with Luau's `read` property syntax.

On `legacy` targets, readonly fields emit as normal fields plus an `-- @readonly (XLuau-enforced)` annotation comment so the emitted code stays compatible while XLuau preserves the source-level immutability intent.

## Why These Features Exist

These are not about inventing a new data model. They are about making common intent:

- immutable shape
- immutable value table
- pattern-driven string logic

more obvious in code.

## Practical Advice Today

Use `freeze { ... }` when you want the runtime table frozen, and use `Readonly<T>` or readonly field declarations when you want that immutability reflected in the inferred type as well.
