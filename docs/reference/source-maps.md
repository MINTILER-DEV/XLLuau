# Source Maps and Debugging

Status: implemented

Readable Luau output is the first debugging strategy in XLuau. Source maps are the next layer for places where readable output alone is not enough.

## Why Source Maps Matter

Some XLuau features expand into multiple Luau statements or helper locals:

- optional chaining
- `match`
- comprehensions
- `switch`
- `do`-expressions

That is still readable, but it means the runtime code may not line up one-to-one with the original source.

## Current Support

The compiler supports:

- generated `.luau.map` files when `sourceMaps` is enabled
- optional `--@line` pragmas when `linePragmas` is enabled
- `xluau remap` for translating Luau stack traces back to XLuau source lines

## Practical Value

Source maps would be especially helpful for:

- stack traces
- stepping through generated code
- editor error mapping
- correlating temporary locals back to source expressions

## Notes

Mappings are currently line-oriented. For transformed multi-line lowerings, emitted lines map back to the originating XLuau statement line.
