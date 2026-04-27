# Modules and Imports

Status: implemented

XLuau keeps Luau's module model. You still use `require`, modules still `return` values, and there is no new `import` or `export` syntax.

The goal is not to replace Luau modules. The goal is to make the existing model less painful.

## Why This Feature Exists

Plain Luau modules are fine until a codebase gets deep:

```lua
local MathUtils = require(script.Parent.Parent.Parent.shared.MathUtils)
```

That is still valid in XLuau, but XLuau adds project-level aliasing and index-file resolution so the common cases stay short and stable.

## Path Aliases

Define aliases in `xluau.config.json`:

```json
{
  "paths": {
    "@shared": "./src/shared",
    "@server": "./src/server",
    "@components/*": "./src/ui/components/*"
  }
}
```

Then use them in source:

```lua
local MathUtils = require "@shared/math"
local Button = require "@components/Button"
```

## What Gets Rewritten

Alias strings are rewritten.

Examples:

```lua
local math = require "@shared/math"
local utils = require("@shared/utils")
```

Depending on target, that becomes:

### Filesystem target

```lua
local math = require("./src/shared/math")
local utils = require("./src/shared/utils")
```

### Roblox target

```lua
local math = require(script.Parent.Parent.shared.math)
```

### Custom target

```lua
local math = require(resolveModule("shared/math"))
```

## What Does Not Get Rewritten

Normal non-alias strings stay as they are:

```lua
local sibling = require "./sibling"
local parent = require("../parent")
```

That is important. XLuau only adds alias logic. It does not take ownership of every possible `require` shape.

## Index File Resolution

If an alias resolves to a directory, XLuau can try index filenames such as `init.xl`.

Example:

```lua
local utils = require "@shared/utils"
```

If `@shared` maps to `./src/shared`, the compiler can resolve:

```text
src/shared/utils/init.xl
```

That makes barrel-style modules work cleanly without introducing a second module system.

## Circular Dependency Detection

Luau allows cycles and resolves them at runtime with partial tables. XLuau treats those as a compile-time problem.

Example cycle:

```text
src/a.xl -> src/b.xl -> src/c.xl -> src/a.xl
```

The compiler fails early instead of letting a partially-initialized module leak into runtime.

## Best Practices

- Use aliases for stable top-level areas like `@shared`, `@server`, and `@ui`.
- Keep alias names semantic, not structural.
- Use index modules when a directory is meant to be consumed as one public entrypoint.
- Avoid cyclic dependencies even when the runtime could technically tolerate them.

## Destructuring the Result

Because `require` still returns an ordinary table, it works naturally with XLuau destructuring:

```lua
local { clamp, lerp } = require "@shared/math"
```

That lowers to the same destructuring logic used anywhere else in the language.
