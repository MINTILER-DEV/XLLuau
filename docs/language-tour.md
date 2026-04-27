# Language Tour

This page is the high-level map of XLuau.

## The Main Problems XLuau Solves

Luau is already a strong language, but there are a few places where everyday code becomes more verbose or more fragile than it should be.

XLuau targets those spots:

- Fallback logic that should distinguish `nil` from valid falsy values
- Safe nested access
- Value-returning conditionals
- Left-to-right data pipelines
- Immutable locals
- Table unpacking
- Multi-branch value dispatch
- Lightweight structural matching
- Better enum ergonomics
- Table construction from iteration
- Small scoped expressions that return values
- Friendlier module resolution

## The Three Buckets of XLuau Features

### 1. Safer Expressions

These improve code you already write all the time:

- `??`
- `??=`
- `?.`
- `? :`
- `|>`

### 2. Better Binding and Control Flow

These reduce ceremony in common code paths:

- `const`
- Destructuring
- `switch`
- `match`
- `do`-expressions

### 3. Better Data Modeling

These improve how you organize and construct data:

- `enum`
- Table comprehensions
- Smarter `require`

## The Compiler Model

XLuau does not try to hide the fact that it is a transpiler.

The pipeline is:

1. Lex XLuau syntax.
2. Parse it into an AST.
3. Run XLuau-specific analysis.
4. Lower XLuau nodes into ordinary Luau.
5. Write readable Luau output.

Pure Luau code is intended to pass through unchanged wherever possible.

## Example of the Style

XLuau:

```lua
local { timeout = 30 } = config
local item = data?.items?.[1] ?? fallback
local label = item ? "loaded" : "empty"
```

Lowered Luau:

```lua
local _d0 = config
local timeout = if _d0.timeout ~= nil then _d0.timeout else 30

local _opt1 = nil
do
    local _cur2 = data
    if _cur2 ~= nil then
        _cur2 = _cur2.items
        if _cur2 ~= nil then
            _cur2 = _cur2[1]
            _opt1 = _cur2
        end
    end
end

local _lhs3 = _opt1
local item = if _lhs3 ~= nil then _lhs3 else fallback
local label = if item then "loaded" else "empty"
```

That output is not magical. It is just the tedious Luau you would have written by hand if you wanted correctness and single evaluation.

## What to Read Next

- For concrete day-to-day syntax, go to [Expressions and Operators](./guides/expressions.md).
- For project structure and alias imports, go to [Modules and Imports](./guides/modules.md).
- For a precise map of what works today, go to [Feature Status](./feature-status.md).
