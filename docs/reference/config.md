# `xluau.config.json` Reference

This page documents the configuration shape currently described by the project and used by the compiler.

## Example

```json
{
  "version": 1,
  "include": ["src/**/*.xl"],
  "exclude": [],
  "outDir": "out",
  "target": "filesystem",
  "luauTarget": "new-solver",
  "baseDir": "src",
  "paths": {
    "@shared": "./src/shared"
  },
  "extensions": [".xl", ".luau", ".lua"],
  "indexFiles": ["init"],
  "sourceMaps": true,
  "linePragmas": false,
  "strict": true,
  "noImplicitAny": true,
  "noUncheckedOptional": true,
  "taskAdapter": "coroutine"
}
```

## Fields

### `include`

Glob patterns for source files to compile.

Common value:

```json
["src/**/*.xl"]
```

### `exclude`

Substrings or paths you do not want included from the matched set.

### `outDir`

Output directory for generated `.luau` files.

### `target`

Supported current values:

- `"filesystem"`
- `"roblox"`
- `"custom"`

This affects module path emission.

### `customTargetFunction`

Used when `target` is `"custom"`.

Example:

```json
{
  "target": "custom",
  "customTargetFunction": "resolveModule"
}
```

### `luauTarget`

Target Luau type-solver mode described by the project design.

### `baseDir`

Root directory used for relative module layout and output structure.

### `paths`

Alias map for `require` resolution.

Example:

```json
{
  "@shared": "./src/shared",
  "@components/*": "./src/ui/components/*"
}
```

### `extensions`

Extensions tried during module resolution.

### `indexFiles`

Filenames tried when a module path resolves to a directory.

### `sourceMaps`

When `true`, writes `.luau.map` files next to emitted `.luau` output.

### `linePragmas`

When `true`, keeps emitted `--@line` comments in the generated Luau output.

### `strict`

Intended compiler strictness toggle.

### `noImplicitAny`

Intended type-checking option.

### `noUncheckedOptional`

Intended type-checking option.

### `taskAdapter`

Controls the planned lowering strategy for task functions.
