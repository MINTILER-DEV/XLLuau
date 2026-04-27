# Bindings, Const, and Destructuring

Status: implemented

These features improve how values enter scope.

## `const`

`const` is a compile-time immutable local.

```lua
const PI = 3.14159
const DEFAULT_TIMEOUT: number = 30
```

The emitted Luau is still just `local`:

```lua
local PI = 3.14159
```

The protection is static, not runtime.

### Why It Matters

Luau has no built-in local immutability marker. `const` gives you a way to communicate intent and catch accidental reassignment early.

### Compile-Time Error

```lua
const PI = 3.14159
PI = 2
```

XLuau reports that as an error.

## Table Destructuring

Extract fields from a table directly in a `local`.

```lua
local { x, y, z } = point
```

Lowering:

```lua
local x = point.x
local y = point.y
local z = point.z
```

### Renaming

```lua
local { x: posX, y: posY } = point
```

### Defaults

```lua
local { role = "user" } = config
```

### Nested Destructuring

```lua
local { position: { x, y } } = entity
```

### Rest

```lua
local { name, ...rest } = options
```

This copies the remaining key/value pairs into a new table.

## Array Destructuring

Index into list-like data by shape:

```lua
local [first, second] = arr
local [head, ...tail] = list
```

### Skip Elements

Use `_` to ignore positions:

```lua
local [first, _, third] = arr
```

## Destructuring in Parameters

You can destructure directly in function parameters:

```lua
function update({ x, y, speed }: EntityConfig)
    move(x, y)
    setSpeed(speed)
end
```

Lowering:

```lua
function update(_param0: EntityConfig)
    local x = _param0.x
    local y = _param0.y
    local speed = _param0.speed
    move(x, y)
    setSpeed(speed)
end
```

## Destructuring in `for` Loops

Generic `for` loops can unpack tables too:

```lua
for { name, score } in players do
    print(name, score)
end
```

## Destructuring `require`

This is a natural fit:

```lua
local { clamp, lerp } = require "@shared/math"
```

That is not a special import feature. It is ordinary destructuring applied to a table result.

## Best Practices

- Use destructuring for small, local unpacking.
- Avoid very deep nested patterns when a named temporary would read better.
- Use defaults when the data is naturally partial.
- Prefer `const` for true one-time bindings and fixed configuration values.
