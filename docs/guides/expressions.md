# Expressions and Operators

Status: implemented

This guide covers the expression features you are most likely to use every day.

## Nullish Coalescing: `??`

Use `??` when you want a fallback only for `nil`.

```lua
local timeout = config.timeout ?? 30
local debug = flags.debug ?? false
```

Why not `or`?

Because `or` treats `false` as missing:

```lua
local debug = flags.debug or false
```

That is wrong when `flags.debug` is explicitly `false`.

### Lowering

```lua
local _lhs0 = config.timeout
local timeout = if _lhs0 ~= nil then _lhs0 else 30
```

### When to Use It

- Configuration defaults
- Optional return values
- Table field fallbacks
- Any place where `false` and `0` are valid values

## Nullish Assignment: `??=`

Assign only when the current value is `nil`:

```lua
stats.total ??= 0
```

Lowering:

```lua
if stats.total == nil then
    stats.total = 0
end
```

Use this for lazy initialization.

## Optional Chaining: `?.`

Optional chaining safely walks nullable paths.

```lua
local name = player?.Character?.Humanoid?.Name
local first = data?.items?.[1]
local hp = entity?.GetHealth()
```

### What It Solves

Without it, you either write a lot of nested guards or rely on `and` chains that collapse on `false`.

### Lowering Pattern

XLuau uses temporaries and explicit guards so each step is evaluated once:

```lua
local _opt0 = nil
do
    local _cur1 = player
    if _cur1 ~= nil then
        _cur1 = _cur1.Character
        if _cur1 ~= nil then
            _cur1 = _cur1.Humanoid
            if _cur1 ~= nil then
                _opt0 = _cur1.Name
            end
        end
    end
end
```

## Ternary: `? :`

Use this when you want a value-level conditional:

```lua
local role = isAdmin ? "admin" : "user"
```

Lowering:

```lua
local role = if isAdmin then "admin" else "user"
```

### Use It Sparingly

A short, direct conditional is great.

Deeply nested ternaries quickly become harder to read than a normal `if`.

## Pipe Operator: `|>`

Pipes let you read transformations from left to right.

```lua
local result = rawData |> parse |> normalize |> validate
```

### Placeholder Form

The piped value normally becomes the first argument.

When you want it somewhere else, use `_`:

```lua
local evens = numbers |> filter(_, isEven)
local doubled = numbers |> map(_, double)
```

### Method Pipe

Call methods directly on the piped value:

```lua
local words = text |> :lower() |> :split(" ")
```

### Lowering Strategy

Short pipelines may inline.

Longer ones become readable temporaries:

```lua
local _pipe0 = parse(rawData)
local _pipe1 = normalize(_pipe0)
local result = validate(_pipe1)
```

### When Pipes Shine

- Data cleanup pipelines
- List transformation chains
- Readability improvements over inside-out nested calls

## Composing Features

These expression features work well together:

```lua
local selected = response?.items?.[1] ?? fallback
local label = selected ? "loaded" : "empty"
local result = numbers |> filter(_, isValid) |> map(_, normalize)
```

That combination is a big part of the day-to-day XLuau experience.
