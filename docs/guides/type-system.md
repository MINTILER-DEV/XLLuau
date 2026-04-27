# Type System Extensions

Status: designed, not fully implemented in the current compiler

XLuau's type-system additions are meant to extend Luau's type story without changing its overall feel.

## Generic Constraints

### Intended Syntax

```lua
local function max<T extends Comparable>(a: T, b: T): T
    return if a > b then a else b
end
```

### Intended Lowering Strategy

The spec uses constraint erasure through intersections:

```lua
local function max<T>(a: T & Comparable, b: T & Comparable): T
    return if a > b then a else b
end
```

That fits Luau's structural type model.

### Why Constraints Matter

Constraints let generic code say more than "this can be any type".

They let APIs communicate expectations such as:

- this value must be comparable
- this value must be serializable
- this shape must contain a specific field

## Explicit Type Arguments

### Intended Syntax

```lua
local binding = createBinding::<number?>(nil)
```

The `::` before `<...>` avoids parser ambiguity with comparison operators.

### Intended Emit Strategy

The design uses two different lowering paths:

1. Cast the argument when the generic parameter is represented by an input value
2. Cast the function value when inference cannot be driven by parameters

That keeps the feature aligned with Luau's existing type assertion model.

## Default Type Parameters on Functions

### Intended Syntax

```lua
local function fetch<T, Err = string>(url: string): Result<T, Err>
    -- ...
end
```

### Why Defaults Help

Default generic parameters are especially useful when one type parameter is "the advanced one" and most callers should not need to spell it out.

## Type Utilities

The spec also reserves a family of utility helpers such as:

- `Partial<T>`
- `Required<T>`
- `Readonly<T>`
- `Pick<T, K>`
- `Omit<T, K>`

These are documented as language-level conveniences over common Luau type transforms.

### Why Utility Types Matter

They capture transforms developers already think about constantly:

- "make this partial"
- "make this readonly"
- "keep only these fields"
- "drop these keys"

## Why These Features Matter

The goal is not to build a completely separate type language. The goal is to make advanced Luau typing more ergonomic where the current syntax gets repetitive.

## Practical Advice Today

The current compiler already passes through a wide range of ordinary Luau type syntax, but these XLuau-specific type extensions are still part of the planned language surface rather than the fully-implemented compiler surface.
