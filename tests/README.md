These fixture projects exercise the currently implemented XLuau and Luau support.

Each project contains:
- `main.xl` or `main.luau`: source fixture
- `expected.luau`: expected emitted Luau output

The integration test in `tests/projects.rs` compiles each fixture and compares the
result to `expected.luau`.
