# Source Maps and Debugging

Status: designed, not fully implemented in the current compiler

Readable Luau output is the first debugging strategy in XLuau. Source maps are the next layer for places where readable output alone is not enough.

## Why Source Maps Matter

Some XLuau features expand into multiple Luau statements or helper locals:

- optional chaining
- `match`
- comprehensions
- `switch`
- `do`-expressions

That is still readable, but it means the runtime code may not line up one-to-one with the original source.

## Planned Support

The language design describes support for:

- generated `.luau.map` files
- optional line pragmas
- better tooling alignment for diagnostics and debugging

## Practical Value

Source maps would be especially helpful for:

- stack traces
- stepping through generated code
- editor error mapping
- correlating temporary locals back to source expressions

## Current Status

The current compiler already benefits from readable emit, which keeps debugging practical today.

The richer source-map story is still part of the language and tooling roadmap rather than a finished implementation in this repository.
