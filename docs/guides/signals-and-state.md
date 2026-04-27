# Signals and Reactive State

Status: designed, not fully implemented in the current compiler

This guide explains the intended XLuau model for signals and reactive state.

## Why These Features Exist

Event and state wiring in Luau often becomes repetitive:

- Arrays of handlers
- Manual connect/disconnect objects
- Fire loops
- Hand-written watch tables

The XLuau design makes those patterns first-class without introducing a runtime dependency.

## Signals

### Intended Syntax

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

### Intended Lowering Shape

The spec lowers signals into a structure with pieces like:

- `_handlers`
- `connect`
- `once`
- `fire`

That matters because the feature is still understandable in normal Luau terms.

## Reactive State

### Intended Syntax

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

The compiler would lower those assignments into watcher-aware updates.

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

These are documented here so learners can understand the intended direction of XLuau, but they are not fully implemented in the current compiler yet.
