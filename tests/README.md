These fixture projects exercise the currently implemented XLuau and Luau support.

Each project contains:
- `main.xl` or `main.luau`: source fixture
- `expected.luau`: expected emitted Luau output

The integration test in `tests/projects.rs` compiles each fixture and compares the
result to `expected.luau`.

The fixtures are intended to be self-contained examples as well, so they define
their own sample input data instead of assuming globals exist at runtime.

Phase 4 adds fixture projects for:
- `switch` + `enum` + `do` expressions
- `match` + table comprehensions
- a combined sample that exercises features from phases 1 through 4 together

Phase 5 adds fixture projects for:
- type-system features such as generic constraints, explicit type arguments, utility types, and `freeze`
- a combined sample that exercises features from phases 1 through 5 together

Phase 6 adds fixture projects for:
- object blocks with inheritance plus task/spawn syntax
- a combined sample that exercises features from phases 1 through 6 together

Phase 3 module-system fixtures live under `tests/module_projects/`.

Those fixtures include their own `xluau.config.json` files plus small source trees
so alias resolution, index-file lookup, target adapters, and cycle detection can
be exercised as project-level behavior rather than single-file lowering only.

Package-manager fixtures live under `tests/package_projects/`.

Those fixtures are copied into a temporary project root during the test run so the
test can inject absolute `file:` package paths, install packages, generate
`packages.luau`, and then verify the final emitted Luau against `expected.luau`.
