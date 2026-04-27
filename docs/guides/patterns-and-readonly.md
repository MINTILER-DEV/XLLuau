# Pattern Literals and Readonly Sugar

Status: split status

- Pattern literals: designed, not yet implemented
- `readonly` and `freeze`: implemented in the current compiler

This guide covers a few planned language features that are useful to know as part of the overall XLuau design.

## Pattern Literals

Pattern literals are intended as friendlier syntax for Luau string patterns.

### Motivation

String patterns in raw quoted form are powerful, but they are not always easy to read or maintain.

### Intended Direction

The spec describes a syntax that makes pattern matching and extraction more expressive while still lowering to ordinary Luau string pattern behavior.

Use cases include:

- Repeated captures
- Pattern constants
- More readable extraction rules

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

- compile-time immutability intent
- `freeze { ... }` lowering to `table.freeze({ ... })`
- utility-type expansion such as `Readonly<typeof(DEFAULTS)>`

For compatibility with the Luau validator used in this repository, emitted type fields currently stay in ordinary `name: Type` form instead of using newer `read` field syntax.

## Why These Features Exist

These are not about inventing a new data model. They are about making common intent:

- immutable shape
- immutable value table
- pattern-driven string logic

more obvious in code.

## Practical Advice Today

Use `freeze { ... }` when you want the runtime table frozen, and use `Readonly<T>` or readonly field declarations to make the source intent clear in XLuau code.
