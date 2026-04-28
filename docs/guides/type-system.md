# Type System Extensions

Status: implemented for phase 5 in the current compiler

XLuau's type-system additions are meant to extend Luau's type story without changing its overall feel.

## Generic Constraints

### Syntax

```lua
local function max<T extends Comparable>(a: T, b: T): T
    return if a > b then a else b
end
```

### Lowering Strategy

The spec uses constraint erasure through intersections:

```lua
local function max<T>(a: T & Comparable, b: T & Comparable): T
    return if a > b then a else b
end
```

That fits Luau's structural type model.

In the current compiler, this lowering happens when XLuau syntax is used inside `.xl` files.

### Why Constraints Matter

Constraints let generic code say more than "this can be any type".

They let APIs communicate expectations such as:

- this value must be comparable
- this value must be serializable
- this shape must contain a specific field

## Explicit Type Arguments

### Syntax

```lua
local binding = createBinding::<number?>(nil)
```

The `::` before `<...>` avoids parser ambiguity with comparison operators.

### Emit Strategy

The design uses two different lowering paths:

1. Cast the argument when the generic parameter is represented by an input value
2. Cast the function value when inference cannot be driven by parameters

That keeps the feature aligned with Luau's existing type assertion model.

If a call target has a known function signature, XLuau instantiates that signature with the provided type arguments and either:

- casts the argument positions that carry the generic
- or casts the function value itself when there is no parameter position to drive inference

## Default Type Parameters on Functions

### Syntax

```lua
local function fetch<T, Err = string>(url: string): Result<T, Err>
    -- ...
end
```

### Why Defaults Help

Default generic parameters are especially useful when one type parameter is "the advanced one" and most callers should not need to spell it out.

The current compiler fills trailing defaults when explicit type arguments are provided with fewer entries than the function declares.

## Type Utilities

The current compiler supports a family of utility helpers such as:

- `Partial<T>`
- `Required<T>`
- `Readonly<T>`
- `Pick<T, K>`
- `Omit<T, K>`
- `Record<K, V>`
- `Exclude<T, U>`
- `ReturnType<typeof(fn)>`
- `Parameters<typeof(fn)>`

These are documented as language-level conveniences over common Luau type transforms.

They can be composed with each other, named type aliases, `typeof(...)` value types, and known function signatures such as regular functions and object constructors/method tables that XLuau emits.

### Why Utility Types Matter

They capture transforms developers already think about constantly:

- "make this partial"
- "make this readonly"
- "keep only these fields"
- "drop these keys"

## Why These Features Matter

The goal is not to build a completely separate type language. The goal is to make advanced Luau typing more ergonomic where the current syntax gets repetitive.

## Practical Advice Today

The current compiler now lowers the phase 5 type-system syntax in `.xl` files.

Practical notes:

- `readonly` field declarations emit `read field: Type` on `new-solver` targets
- `readonly` field declarations emit `field: Type,  -- @readonly (XLuau-enforced)` on `legacy` targets
- `freeze { ... }` lowers to `table.freeze(...)` and XLuau preserves readonly field information in the inferred table type
