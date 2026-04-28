# Signals and Reactive State

Status: implemented for phase 7 in the current compiler

This guide explains the XLuau model for signals and reactive state as implemented in the current compiler.

## Why These Features Exist

Event and state wiring in Luau often becomes repetitive:

- Arrays of handlers
- Manual connect/disconnect objects
- Fire loops
- Hand-written watch tables

The XLuau design makes those patterns first-class without introducing a runtime dependency.

## Signals

### Syntax

```lua
signal OnPlayerJoined: (player: Player)
signal OnScoreChanged: (old: number, new: number)
signal OnDied
```

Fire them:

```lua
fire OnPlayerJoined(player)
fire OnDied
```

Attach listeners:

```lua
on OnPlayerJoined |player|
    setupHUD(player)
end
```

One-shot listeners:

```lua
once OnPlayerJoined |player|
    print(player.Name)
end
```

### Mental Model

Signals are intended to be ordinary values backed by handler tables.

The design gives you a direct syntax for the things people usually build by hand:

- register a callback
- disconnect later
- fire all handlers
- auto-disconnect after the first fire

### Lowering Shape

Signals lower into ordinary Luau tables with:

- `_handlers`
- `connect`
- `once`
- `fire`

For example:

```lua
signal OnPlayerJoined: (player: Player)
```

becomes a typed signal table with a generated `_Signal_OnPlayerJoined` alias plus a local value that stores handlers and exposes `connect`, `once`, and `fire`.

## Reactive State

### Syntax

```lua
state playerCount: number = 0
state currentMap: string = "Lobby"
```

Watch it:

```lua
watch playerCount |old, new|
    updatePlayerCountUI(new)
end
```

Assignments remain normal assignments:

```lua
playerCount = playerCount + 1
```

The compiler lowers those assignments into watcher-aware updates.

### Mental Model

Reactive state is meant to keep source code looking like normal variable code while letting the compiler expand the assignment path into watcher notification logic.

Conceptually it gives you:

- one backing value
- one watcher list
- one normal-looking assignment surface

### Why This Is Useful

This is especially attractive for:

- UI counters
- selected map or mode
- values watched by several small systems
- state that changes often but should remain easy to trace

## Design Philosophy

The important part is that these are meant to compile into ordinary callback-table patterns. The language design does not require a shared runtime library to make them work.

## Practical Advice Today

Current behavior:

- `signal Name: (...)` emits a generated signal type plus a local handler table implementation
- `fire Name(...)` lowers to `Name:fire(...)`
- `on Name |...| ... end` lowers to `Name:connect(function(...) ... end)`
- `once Name |...| ... end` lowers to `Name:once(function(...) ... end)`
- `local conn = on ... end` and `local conn = once ... end` are supported
- `state name: T = value` emits a backing local plus a watcher list
- `watch name |old, new| ... end` registers a watcher with `table.insert`
- direct assignment, compound assignment, and `??=` on state locals notify watchers when the value changes
