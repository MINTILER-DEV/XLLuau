# Transpilation Model

One of the best ways to understand XLuau is to understand how it lowers into Luau.

## Core Rule

Pure Luau should remain pure Luau.

XLuau-specific syntax is lowered into ordinary Luau constructs:

- locals
- `if` expressions
- `do` blocks
- loops
- `table.freeze`
- plain `require`

## Why the Output Matters

Readable output gives you:

- Easier debugging
- Easier trust in the compiler
- Easier onboarding for Luau developers
- Fewer surprises when reading generated code

## Common Lowering Patterns

### Single-Evaluation Temporaries

Used for:

- nullish coalescing
- optional chaining
- switch dispatch
- do-expressions

Example:

```lua
local _lhs0 = config.timeout
local timeout = if _lhs0 ~= nil then _lhs0 else 30
```

### Structured Expansion

Used for:

- destructuring
- `match`
- comprehensions

Example:

```lua
local _comp0 = {}
for _, x in numbers do
    table.insert(_comp0, x * 2)
end
local doubled = _comp0
```

### Namespace Table Generation

Used for enums:

```lua
type Direction = "North" | "South"
local Direction = table.freeze({
    North = "North" :: Direction,
    South = "South" :: Direction,
})
```

## Design Tradeoff

XLuau prefers explicit lowered code over clever hidden runtime helpers. That makes the generated Luau longer in some places, but it also makes it clearer and easier to reason about.
