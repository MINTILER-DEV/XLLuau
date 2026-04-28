# Objects and Task Functions

Status: implemented in the current compiler

This guide documents the intended language design for object blocks and task functions.

## Object Blocks

Object blocks are meant to be thin sugar over Luau's metatable pattern.

### Design Goal

Do not replace Luau's object model.

Instead:

- Keep metatables
- Keep `self`
- Keep colon methods
- Remove repeated boilerplate

### Syntax

```lua
object Animal
    name: string
    sound: string

    function new(name: string, sound: string): Animal
        self.name = name
        self.sound = sound
    end

    function speak(): string
        return `{self.name} says {self.sound}!`
    end
end
```

### Mental Model

Read an `object` block as compiler sugar for the Luau metatable style you already know.

It is meant to generate:

- an instance type
- a class table
- `__index`
- constructor setup
- method declarations

The design is intentionally conservative. It is not trying to import a foreign class system into Luau.

### Lowering

The design lowers to the familiar pattern:

```lua
type Animal = {
    name: string,
    sound: string,
    speak: (self: Animal) -> string,
}

local Animal = {}
Animal.__index = Animal

function Animal.new(name: string, sound: string): Animal
    local self = setmetatable({} :: Animal, Animal)
    self.name = name
    self.sound = sound
    return self
end

function Animal:speak(): string
    return `{self.name} says {self.sound}!`
end
```

### Inheritance

The current compiler supports:

- `extends`
- `super.new(...)`
- `super.method(self, ...)`

The intent is still metatable inheritance, not a foreign class model.

### Why This Feature Is Valuable

Most Luau object code is not hard because the model is bad. It is hard because the boilerplate is repetitive:

- allocate and cast `self`
- remember `__index`
- set metatables in the right place
- mirror methods into the instance type

Object blocks are meant to solve that repetition without changing the underlying runtime model.

## Task Functions

Task functions are intended as coroutine sugar, not JavaScript-style promises.

### Syntax

```lua
task function loadPlayer(id: number): Player
    local data = yield fetchData(id)
    local inv = yield fetchInventory(id)
    return buildPlayer(data, inv)
end
```

### Meaning

- `task function` creates coroutine-oriented async work
- `yield expr` suspends inside a task function
- `spawn fn(...)` starts work

### Why This Matches Luau Well

The design is deliberately coroutine-first.

That means it does not try to hide:

- suspension points
- resumed execution
- target-specific task scheduling

Instead, it tries to make that model easier to write, especially in codebases that already think in coroutines.

### Lowering

The spec lowers it toward:

```lua
local function loadPlayer(id: number): thread
    return coroutine.create(function()
        local data = coroutine.yield(fetchData(id))
        local inv = coroutine.yield(fetchInventory(id))
        return buildPlayer(data, inv)
    end)
end
```

### Spawn Handlers

The design also includes structured success and failure handling:

```lua
spawn loadPlayer(42)
    then |player|
        setupHUD(player)
    catch |err|
        warn("Failed:", err)
end
```

The current compiler lowers this into plain coroutine control flow for the default adapter, and wraps the same logic in `task.spawn(function() ... end)` when the Roblox task adapter is selected.

### Why This Matters

The goal is to make coroutine flow readable while staying honest about the runtime model.

### Good Use Cases

- staged loading flows
- coroutine-based gameplay tasks
- Roblox task orchestration
- long sequences where each step depends on the last step's result

## Practical Advice Today

These features are available today.

A few practical notes:

- object instance types emit method slots in explicit `self` form such as `(self: Hero) -> string`
- `yield` is enforced as task-only syntax by the XLuau compiler
- the default `spawn` adapter runs through `coroutine.resume`
- Roblox projects can opt into the task adapter through `taskAdapter: "roblox"` or by targeting Roblox
