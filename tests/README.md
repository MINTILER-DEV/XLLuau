These fixture projects exercise the currently implemented XLuau and Luau support.

Each project contains:
- `main.xl` or `main.luau`: source fixture
- `expected.luau`: expected emitted Luau output

The integration test in `tests/projects.rs` compiles each fixture and compares the
result to `expected.luau`.

The fixtures are intended to be self-contained examples as well, so they define
their own sample input data instead of assuming globals exist at runtime.
