# Control Flow and Data

Status: implemented for the features documented on this page

This guide covers the phase 4 features:

- `switch`
- `match`
- `enum`
- Table comprehensions
- `do`-expressions

## `switch`

`switch` is for multi-branch equality dispatch on one value.

```lua
switch state
    case "idle"
        handleIdle()
    case "walk"
        handleWalk()
    default
        handleUnknown()
end
```

### Why Use It

Use `switch` when:

- Every branch compares the same subject
- You want the branch list to read vertically
- You want exhaustiveness checking over enums or literal unions

### Switch Expression

`switch` can also return a value:

```lua
local label = switch count
    case 0 then "none"
    case 1 then "one"
    default then "many"
end
```

### Fallthrough

`fallthrough` lets several case values share a body:

```lua
switch code
    case 400
    case 401
        fallthrough
    case 403
        handleAuthError(code)
end
```

That lowers into an `or` chain against the same captured subject.

## `match`

`match` is for shape-based branching, especially table variants.

```lua
match result
    { kind = "ok", value = v }
        print(v)
    { kind = "err", error = e }
        print(e)
end
```

### Pattern Bindings

Names inside a pattern bind matched values:

```lua
{ kind = "ok", value = v }
```

This means:

- `kind` must equal `"ok"`
- `value` gets bound to local `v`

### Guards

Add an `if` guard when shape alone is not enough:

```lua
match point
    { x = x, y = 0 } if x > 0
        print("positive x-axis", x)
    { x = x, y = y }
        print(x, y)
end
```

### What `match` Is Best For

- Tagged unions
- API result tables
- Event payloads
- Lightweight structural dispatch

## `enum`

`enum` gives you both:

- A namespaced value table
- A matching type identity

### String-Backed Enum

```lua
enum Direction
    North
    South
    East
    West
end
```

Lowering:

```lua
type Direction = "North" | "South" | "East" | "West"
local Direction = table.freeze({
    North = "North" :: Direction,
    South = "South" :: Direction,
    East = "East" :: Direction,
    West = "West" :: Direction,
})
```

### Explicit Values

```lua
enum HttpMethod
    Get = "GET"
    Post = "POST"
end
```

### Number-Backed Enum

```lua
enum Flags: number
    None = 0
    Read = 1
    Write = 2
end
```

### When to Use Enums

- Known closed sets
- Switch subjects
- Namespaced constants with type meaning

## Table Comprehensions

Table comprehensions build tables from iteration without writing the accumulator manually.

### Array Comprehension

```lua
local doubled = { x * 2 for _, x in numbers }
```

Lowering:

```lua
local doubled = {}
for _, x in numbers do
    table.insert(doubled, x * 2)
end
```

### With Filter

```lua
local evens = { x for _, x in numbers if x % 2 == 0 }
```

### Dictionary Comprehension

```lua
local squared = { [x] = x ^ 2 for _, x in numbers }
```

### Nested Comprehension

```lua
local flat = { value for _, row in matrix for _, value in row }
```

### When to Use Them

- Mapping lists
- Filtering lists
- Building lookup tables
- Flattening nested iteration

### When Not to Use Them

If the logic inside the comprehension becomes long or branch-heavy, a normal loop is usually clearer.

## `do`-Expressions

`do` already exists in Luau as a scope block. XLuau lets a `do` block return a value when used in expression position.

```lua
local distance = do
    local dx = b.x - a.x
    local dy = b.y - a.y
    math.sqrt(dx ^ 2 + dy ^ 2)
end
```

Lowering:

```lua
local distance
do
    local dx = b.x - a.x
    local dy = b.y - a.y
    distance = math.sqrt(dx ^ 2 + dy ^ 2)
end
```

### Good Uses

- Multi-step value derivation
- Small scoped temporaries
- Returning a computed value without hoisting locals into the outer scope

## Exhaustiveness Checking

The current compiler performs static checks for:

- `switch` over literal unions
- `switch` over enums
- `match` over discriminated unions

Example:

```lua
type Direction = "North" | "South"
local dir: Direction = "North"

switch dir
    case "North"
        print("north")
end
```

This is reported as non-exhaustive because `"South"` is not handled.

That matters because these features are not just syntax sugar. They are also places where the compiler can help you keep branching logic honest.
