# Feature Status

This page separates the XLuau language design from the current implementation in this repository.

## Implemented

These features are implemented and tested in the current Rust compiler:

- Module alias resolution in `require`
- Index-file resolution for modules
- Circular dependency detection
- Filesystem, Roblox, and custom module targets
- `??`
- `??=`
- `?.`
- `? :`
- `|>`
- `const`
- Table and array destructuring
- Destructured function parameters
- Destructured generic `for` bindings
- `switch` statements
- `switch` expressions
- `match` statements
- `enum`
- Table comprehensions
- `do`-expressions
- Generic constraints on functions
- Explicit type arguments with `::<...>`
- Default type parameters on functions
- Built-in type utilities:
  - `Partial`
  - `Required`
  - `Readonly`
  - `Pick`
  - `Omit`
  - `Record`
  - `Exclude`
  - `ReturnType`
  - `Parameters`
- `freeze {}` sugar
- `readonly` field lowering for `new-solver` and `legacy` targets
- Object blocks
- Object inheritance with `extends` and `super`
- Task functions
- `yield`
- `spawn`
- Roblox task adapter for `spawn`
- Exhaustiveness checks for:
  - `switch` over literal unions and enums
  - `match` over discriminated unions

## Designed but Not Yet Implemented

These are part of the language design and are documented here so people can learn the intended shape of XLuau, but they are not fully implemented in the current compiler yet:

- Signals
- Reactive state
- Pattern literals
- Source maps and line pragmas
- LSP and editor tooling described in the spec

## How to Read the Guides

The guides in `docs/guides` do two things:

- Explain how a feature is supposed to work in XLuau
- Call out whether that feature is implemented today

When a guide covers both implemented and planned features, it will say so clearly.
