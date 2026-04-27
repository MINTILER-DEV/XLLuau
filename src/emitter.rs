use std::collections::HashSet;

use crate::{
    ast::*,
    compiler::{CompilerError, Result},
};

pub struct Emitter {
    temp_counter: usize,
    const_scopes: Vec<HashSet<String>>,
    errors: Vec<String>,
}

#[derive(Debug, Clone)]
struct LoweredExpr {
    setup: Vec<String>,
    expr: String,
    reuse_safe: bool,
}

#[derive(Debug, Clone)]
struct LoweredTarget {
    setup: Vec<String>,
    expr: String,
}

impl Emitter {
    pub fn new() -> Self {
        Self {
            temp_counter: 0,
            const_scopes: vec![HashSet::new()],
            errors: Vec::new(),
        }
    }

    pub fn emit_program(&mut self, program: &Program) -> Result<String> {
        let output = self.emit_block(&program.block, 0)?;
        if self.errors.is_empty() {
            Ok(output)
        } else {
            Err(CompilerError::Semantic {
                messages: self.errors.clone(),
            })
        }
    }

    fn emit_block(&mut self, block: &Block, indent: usize) -> Result<String> {
        self.const_scopes.push(HashSet::new());
        let mut lines = Vec::new();
        for stmt in block {
            let chunk = self.emit_stmt(stmt, indent)?;
            if !chunk.is_empty() {
                lines.push(chunk);
            }
        }
        self.const_scopes.pop();
        Ok(lines.join("\n"))
    }

    fn emit_stmt(&mut self, stmt: &Stmt, indent: usize) -> Result<String> {
        match stmt {
            Stmt::Local(local) => self.emit_local(local, indent),
            Stmt::Function(function) => self.emit_function_stmt(function, indent),
            Stmt::Assignment(assignment) => self.emit_assignment(assignment, indent),
            Stmt::CompoundAssignment { target, op, value } => {
                self.emit_compound_assignment(target, *op, value, indent)
            }
            Stmt::NullishAssignment { target, value } => {
                self.emit_nullish_assignment(target, value, indent)
            }
            Stmt::Call(expr) => {
                let lowered = self.emit_expr(expr, None)?;
                let mut parts = Vec::new();
                self.push_setup(&mut parts, indent, lowered.setup);
                parts.push(self.indent(indent, &lowered.expr));
                Ok(parts.join("\n"))
            }
            Stmt::Return(values) => self.emit_return(values, indent),
            Stmt::If(if_stmt) => self.emit_if_stmt(if_stmt, indent),
            Stmt::While { condition, block } => self.emit_while_stmt(condition, block, indent),
            Stmt::Repeat { block, condition } => self.emit_repeat_stmt(block, condition, indent),
            Stmt::ForNumeric(for_numeric) => self.emit_for_numeric(for_numeric, indent),
            Stmt::ForGeneric(for_generic) => self.emit_for_generic(for_generic, indent),
            Stmt::Do(block) => {
                let inner = self.emit_block(block, indent + 1)?;
                Ok(format!(
                    "{}\n{}\n{}",
                    self.indent(indent, "do"),
                    inner,
                    self.indent(indent, "end")
                ))
            }
            Stmt::Break => Ok(self.indent(indent, "break")),
            Stmt::Continue => Ok(self.indent(indent, "continue")),
            Stmt::TypeAlias { raw } => Ok(self.indent(indent, raw)),
        }
    }

    fn emit_local(&mut self, local: &LocalDecl, indent: usize) -> Result<String> {
        let mut parts = Vec::new();
        let needs_destructure = local
            .bindings
            .iter()
            .any(|binding| !matches!(binding.pattern, Pattern::Name(_)));

        if !needs_destructure && local.bindings.len() == 1 {
            let binding = &local.bindings[0];
            let name = self.pattern_name(&binding.pattern).unwrap();
            let annotation = binding
                .type_annotation
                .as_ref()
                .map(|text| format!(": {text}"))
                .unwrap_or_default();
            if let Some(value) = local.values.first() {
                let lowered = self.emit_expr(value, None)?;
                self.push_setup(&mut parts, indent, lowered.setup);
                parts.push(self.indent(
                    indent,
                    &format!("local {name}{annotation} = {}", lowered.expr),
                ));
            } else {
                parts.push(self.indent(indent, &format!("local {name}{annotation}")));
            }
        } else {
            let value_names = self.lower_local_values(&mut parts, indent, &local.values)?;
            for (index, binding) in local.bindings.iter().enumerate() {
                let source = value_names
                    .get(index)
                    .cloned()
                    .unwrap_or_else(|| "nil".to_string());
                self.emit_pattern_local(
                    &mut parts,
                    indent,
                    &binding.pattern,
                    &binding.type_annotation,
                    &source,
                    true,
                )?;
            }
        }

        for binding in &local.bindings {
            self.declare_local_names(&binding.pattern, local.is_const);
        }

        Ok(parts.join("\n"))
    }

    fn lower_local_values(
        &mut self,
        parts: &mut Vec<String>,
        indent: usize,
        values: &[Expr],
    ) -> Result<Vec<String>> {
        let mut names = Vec::new();
        for value in values {
            let lowered = self.emit_expr(value, None)?;
            self.push_setup(parts, indent, lowered.setup);
            let temp = self.next_temp("v");
            parts.push(self.indent(indent, &format!("local {temp} = {}", lowered.expr)));
            names.push(temp);
        }
        Ok(names)
    }

    fn emit_pattern_local(
        &mut self,
        parts: &mut Vec<String>,
        indent: usize,
        pattern: &Pattern,
        type_annotation: &Option<String>,
        source: &str,
        is_local: bool,
    ) -> Result<()> {
        match pattern {
            Pattern::Name(name) => {
                let keyword = if is_local { "local " } else { "" };
                let annotation = type_annotation
                    .as_ref()
                    .map(|text| format!(": {text}"))
                    .unwrap_or_default();
                parts.push(self.indent(indent, &format!("{keyword}{name}{annotation} = {source}")));
            }
            Pattern::Table { entries, rest } => {
                let base = self.next_temp("d");
                parts.push(self.indent(indent, &format!("local {base} = {source}")));
                for entry in entries {
                    let access = format!("{base}.{}", entry.key);
                    self.emit_pattern_binding(parts, indent, &entry.binding, &access)?;
                }
                if let Some(rest_name) = rest {
                    let temp = self.next_temp("rest");
                    parts.push(self.indent(indent, &format!("local {rest_name} = {{}}")));
                    parts.push(self.indent(indent, &format!("for {temp}, _v in {base} do")));
                    let excluded = entries
                        .iter()
                        .map(|entry| format!("{temp} ~= \"{}\"", entry.key))
                        .collect::<Vec<_>>()
                        .join(" and ");
                    parts.push(self.indent(indent + 1, &format!("if {excluded} then")));
                    parts.push(self.indent(indent + 2, &format!("{rest_name}[{temp}] = _v")));
                    parts.push(self.indent(indent + 1, "end"));
                    parts.push(self.indent(indent, "end"));
                }
            }
            Pattern::Array { items, rest } => {
                let base = self.next_temp("d");
                parts.push(self.indent(indent, &format!("local {base} = {source}")));
                for (index, item) in items.iter().enumerate() {
                    if let Some(binding) = &item.binding {
                        let access = format!("{base}[{}]", index + 1);
                        self.emit_pattern_binding(parts, indent, binding, &access)?;
                    }
                }
                if let Some(rest_name) = rest {
                    let start = items.len() + 1;
                    parts.push(self.indent(
                        indent,
                        &format!(
                            "local {rest_name} = table.move({base}, {start}, #{base}, 1, {{}})"
                        ),
                    ));
                }
            }
        }
        Ok(())
    }

    fn emit_pattern_binding(
        &mut self,
        parts: &mut Vec<String>,
        indent: usize,
        binding: &PatternBinding,
        source: &str,
    ) -> Result<()> {
        let resolved = if let Some(default_value) = &binding.default_value {
            let lowered = self.emit_expr(default_value, None)?;
            if lowered.setup.is_empty() {
                format!("if {source} ~= nil then {source} else {}", lowered.expr)
            } else {
                let temp = self.next_temp("default");
                parts.push(self.indent(indent, &format!("local {temp}")));
                parts.push(self.indent(indent, &format!("if {source} ~= nil then")));
                parts.push(self.indent(indent + 1, &format!("{temp} = {source}")));
                parts.push(self.indent(indent, "else"));
                self.push_setup(parts, indent + 1, lowered.setup);
                parts.push(self.indent(indent + 1, &format!("{temp} = {}", lowered.expr)));
                parts.push(self.indent(indent, "end"));
                temp
            }
        } else {
            source.to_string()
        };
        self.emit_pattern_local(parts, indent, &binding.target, &None, &resolved, true)
    }

    fn emit_function_stmt(&mut self, function: &FunctionDecl, indent: usize) -> Result<String> {
        let header = if function.local_name {
            format!("local function {}", function.name.root)
        } else {
            let mut name = function.name.root.clone();
            for field in &function.name.fields {
                name.push('.');
                name.push_str(field);
            }
            if let Some(method) = &function.name.method {
                name.push(':');
                name.push_str(method);
            }
            format!("function {name}")
        };

        let (params, prologue) = self.lower_params(&function.params)?;
        if function.local_name {
            self.declare_name(&function.name.root, false);
        }

        self.const_scopes.push(HashSet::new());
        for name in self.collect_param_names(&function.params) {
            self.declare_name(&name, false);
        }
        let mut body_lines = Vec::new();
        for line in prologue {
            body_lines.push(self.indent(indent + 1, &line));
        }
        let body = self.emit_block(&function.body, indent + 1)?;
        if !body.is_empty() {
            body_lines.push(body);
        }
        self.const_scopes.pop();

        let generics = function.generics.clone().unwrap_or_default();
        let return_type = function
            .return_type
            .as_ref()
            .map(|text| format!(": {text}"))
            .unwrap_or_default();
        let signature = format!("{header}{generics}({}){return_type}", params.join(", "));

        let mut parts = vec![self.indent(indent, &signature)];
        if !body_lines.is_empty() {
            parts.push(body_lines.join("\n"));
        }
        parts.push(self.indent(indent, "end"));
        Ok(parts.join("\n"))
    }

    fn emit_assignment(&mut self, assignment: &Assignment, indent: usize) -> Result<String> {
        for target in &assignment.targets {
            self.check_const_target(target);
        }

        let mut parts = Vec::new();
        let mut targets = Vec::new();
        for target in &assignment.targets {
            let lowered = self.emit_assign_target(target, false)?;
            self.push_setup(&mut parts, indent, lowered.setup);
            targets.push(lowered.expr);
        }

        let mut values = Vec::new();
        for value in &assignment.values {
            let lowered = self.emit_expr(value, None)?;
            self.push_setup(&mut parts, indent, lowered.setup);
            values.push(lowered.expr);
        }
        if values.is_empty() {
            values.push("nil".to_string());
        }
        parts.push(self.indent(
            indent,
            &format!("{} = {}", targets.join(", "), values.join(", ")),
        ));
        Ok(parts.join("\n"))
    }

    fn emit_nullish_assignment(
        &mut self,
        target: &AssignTarget,
        value: &Expr,
        indent: usize,
    ) -> Result<String> {
        self.check_const_target(target);
        let mut parts = Vec::new();
        let lowered_target = self.emit_assign_target(target, true)?;
        self.push_setup(&mut parts, indent, lowered_target.setup);
        let lowered_value = self.emit_expr(value, None)?;
        let target_expr = lowered_target.expr.clone();
        parts.push(self.indent(indent, &format!("if {target_expr} == nil then")));
        self.push_setup(&mut parts, indent + 1, lowered_value.setup);
        parts.push(self.indent(
            indent + 1,
            &format!("{target_expr} = {}", lowered_value.expr),
        ));
        parts.push(self.indent(indent, "end"));
        Ok(parts.join("\n"))
    }

    fn emit_compound_assignment(
        &mut self,
        target: &AssignTarget,
        op: CompoundOp,
        value: &Expr,
        indent: usize,
    ) -> Result<String> {
        self.check_const_target(target);
        let mut parts = Vec::new();
        let lowered_target = self.emit_assign_target(target, true)?;
        self.push_setup(&mut parts, indent, lowered_target.setup);
        let target_expr = lowered_target.expr.clone();
        let lowered_value = self.emit_expr(value, None)?;
        self.push_setup(&mut parts, indent, lowered_value.setup);
        parts.push(self.indent(
            indent,
            &format!(
                "{target_expr} = {target_expr} {} {}",
                compound_token(op),
                lowered_value.expr
            ),
        ));
        Ok(parts.join("\n"))
    }

    fn emit_return(&mut self, values: &[Expr], indent: usize) -> Result<String> {
        let mut parts = Vec::new();
        let mut emitted = Vec::new();
        for value in values {
            let lowered = self.emit_expr(value, None)?;
            self.push_setup(&mut parts, indent, lowered.setup);
            emitted.push(lowered.expr);
        }
        if emitted.is_empty() {
            parts.push(self.indent(indent, "return"));
        } else {
            parts.push(self.indent(indent, &format!("return {}", emitted.join(", "))));
        }
        Ok(parts.join("\n"))
    }

    fn emit_if_stmt(&mut self, if_stmt: &IfStmt, indent: usize) -> Result<String> {
        let mut parts = Vec::new();
        for (index, (condition, block)) in if_stmt.branches.iter().enumerate() {
            let lowered_condition = self.emit_expr(condition, None)?;
            let lowered = self.capture_if_needed(lowered_condition, "cond");
            self.push_setup(&mut parts, indent, lowered.setup);
            let head = if index == 0 { "if" } else { "elseif" };
            parts.push(self.indent(indent, &format!("{head} {} then", lowered.expr)));
            let body = self.emit_block(block, indent + 1)?;
            if !body.is_empty() {
                parts.push(body);
            }
        }
        if let Some(block) = &if_stmt.else_block {
            parts.push(self.indent(indent, "else"));
            let body = self.emit_block(block, indent + 1)?;
            if !body.is_empty() {
                parts.push(body);
            }
        }
        parts.push(self.indent(indent, "end"));
        Ok(parts.join("\n"))
    }

    fn emit_while_stmt(
        &mut self,
        condition: &Expr,
        block: &Block,
        indent: usize,
    ) -> Result<String> {
        let lowered_condition = self.emit_expr(condition, None)?;
        let lowered = self.capture_if_needed(lowered_condition, "cond");
        if lowered.setup.is_empty() {
            let body = self.emit_block(block, indent + 1)?;
            let mut parts = vec![self.indent(indent, &format!("while {} do", lowered.expr))];
            if !body.is_empty() {
                parts.push(body);
            }
            parts.push(self.indent(indent, "end"));
            return Ok(parts.join("\n"));
        }

        let guard = self.next_temp("while");
        let mut parts = vec![self.indent(indent, "while true do")];
        self.push_setup(&mut parts, indent + 1, lowered.setup);
        parts.push(self.indent(indent + 1, &format!("local {guard} = {}", lowered.expr)));
        parts.push(self.indent(indent + 1, &format!("if not {guard} then")));
        parts.push(self.indent(indent + 2, "break"));
        parts.push(self.indent(indent + 1, "end"));
        let body = self.emit_block(block, indent + 1)?;
        if !body.is_empty() {
            parts.push(body);
        }
        parts.push(self.indent(indent, "end"));
        Ok(parts.join("\n"))
    }

    fn emit_repeat_stmt(
        &mut self,
        block: &Block,
        condition: &Expr,
        indent: usize,
    ) -> Result<String> {
        let mut parts = vec![self.indent(indent, "repeat")];
        let body = self.emit_block(block, indent + 1)?;
        if !body.is_empty() {
            parts.push(body);
        }
        let lowered_condition = self.emit_expr(condition, None)?;
        let lowered = self.capture_if_needed(lowered_condition, "repeat");
        self.push_setup(&mut parts, indent + 1, lowered.setup);
        parts.push(self.indent(indent, &format!("until {}", lowered.expr)));
        Ok(parts.join("\n"))
    }

    fn emit_for_numeric(&mut self, for_numeric: &ForNumeric, indent: usize) -> Result<String> {
        let start = self.emit_expr(&for_numeric.start, None)?;
        let end = self.emit_expr(&for_numeric.end, None)?;
        let step = match &for_numeric.step {
            Some(step) => Some(self.emit_expr(step, None)?),
            None => None,
        };
        let mut parts = Vec::new();
        self.push_setup(&mut parts, indent, start.setup);
        self.push_setup(&mut parts, indent, end.setup);
        if let Some(step) = &step {
            self.push_setup(&mut parts, indent, step.setup.clone());
        }
        let range = if let Some(step) = step {
            format!("{}, {}, {}", start.expr, end.expr, step.expr)
        } else {
            format!("{}, {}", start.expr, end.expr)
        };
        parts.push(self.indent(indent, &format!("for {} = {range} do", for_numeric.name)));
        self.const_scopes.push(HashSet::new());
        self.declare_name(&for_numeric.name, false);
        let body = self.emit_block(&for_numeric.block, indent + 1)?;
        self.const_scopes.pop();
        if !body.is_empty() {
            parts.push(body);
        }
        parts.push(self.indent(indent, "end"));
        Ok(parts.join("\n"))
    }

    fn emit_for_generic(&mut self, for_generic: &ForGeneric, indent: usize) -> Result<String> {
        let mut parts = Vec::new();
        let mut iterables = Vec::new();
        for iterable in &for_generic.iterables {
            let lowered = self.emit_expr(iterable, None)?;
            self.push_setup(&mut parts, indent, lowered.setup);
            iterables.push(lowered.expr);
        }

        let mut loop_names = Vec::new();
        let mut prologue = Vec::new();
        for binding in &for_generic.bindings {
            if let Some(name) = self.pattern_name(&binding.pattern) {
                loop_names.push(name.to_string());
            } else {
                let temp = self.next_temp("for");
                loop_names.push(temp.clone());
                self.emit_pattern_local(
                    &mut prologue,
                    0,
                    &binding.pattern,
                    &binding.type_annotation,
                    &temp,
                    true,
                )?;
            }
        }

        parts.push(self.indent(
            indent,
            &format!(
                "for {} in {} do",
                loop_names.join(", "),
                iterables.join(", ")
            ),
        ));
        self.const_scopes.push(HashSet::new());
        for binding in &for_generic.bindings {
            self.declare_local_names(&binding.pattern, false);
        }
        for line in prologue {
            parts.push(self.indent(indent + 1, &line));
        }
        let body = self.emit_block(&for_generic.block, indent + 1)?;
        self.const_scopes.pop();
        if !body.is_empty() {
            parts.push(body);
        }
        parts.push(self.indent(indent, "end"));
        Ok(parts.join("\n"))
    }

    fn emit_assign_target(
        &mut self,
        target: &AssignTarget,
        capture: bool,
    ) -> Result<LoweredTarget> {
        match target {
            AssignTarget::Name(name) => Ok(LoweredTarget {
                setup: Vec::new(),
                expr: name.clone(),
            }),
            AssignTarget::Field { object, field } => {
                let lowered_object = self.emit_expr(object, None)?;
                let lowered_object = self.capture_if_needed(lowered_object, "obj");
                let expr = if capture && !lowered_object.setup.is_empty() {
                    let temp = self.next_temp("obj");
                    let mut setup = lowered_object.setup;
                    setup.push(format!("local {temp} = {}", lowered_object.expr));
                    return Ok(LoweredTarget {
                        setup,
                        expr: format!("{temp}.{field}"),
                    });
                } else {
                    format!("{}.{}", lowered_object.expr, field)
                };
                Ok(LoweredTarget {
                    setup: lowered_object.setup,
                    expr,
                })
            }
            AssignTarget::Index { object, index } => {
                let lowered_object = self.emit_expr(object, None)?;
                let lowered_object = self.capture_if_needed(lowered_object, "obj");
                let lowered_index = self.emit_expr(index, None)?;
                let lowered_index = self.capture_if_needed(lowered_index, "idx");
                let mut setup = lowered_object.setup;
                setup.extend(lowered_index.setup);
                let object_expr = if capture && !setup.is_empty() {
                    let temp = self.next_temp("obj");
                    setup.push(format!("local {temp} = {}", lowered_object.expr));
                    temp
                } else {
                    lowered_object.expr
                };
                let index_expr = if capture && !setup.is_empty() {
                    let temp = self.next_temp("idx");
                    setup.push(format!("local {temp} = {}", lowered_index.expr));
                    temp
                } else {
                    lowered_index.expr
                };
                Ok(LoweredTarget {
                    setup,
                    expr: format!("{object_expr}[{index_expr}]"),
                })
            }
        }
    }

    fn emit_expr(&mut self, expr: &Expr, placeholder: Option<&str>) -> Result<LoweredExpr> {
        match expr {
            Expr::Nil => Ok(self.simple_expr("nil", true)),
            Expr::Bool(value) => Ok(self.simple_expr(if *value { "true" } else { "false" }, true)),
            Expr::Number(value) | Expr::String(value) => Ok(self.simple_expr(value, true)),
            Expr::VarArg => Ok(self.simple_expr("...", true)),
            Expr::Name(name) if placeholder.is_some() && name == "_" => {
                Ok(self.simple_expr(placeholder.unwrap(), true))
            }
            Expr::Name(name) => Ok(self.simple_expr(name, true)),
            Expr::Paren(inner) => {
                let lowered = self.emit_expr(inner, placeholder)?;
                Ok(LoweredExpr {
                    setup: lowered.setup,
                    expr: format!("({})", lowered.expr),
                    reuse_safe: lowered.reuse_safe,
                })
            }
            Expr::Unary { op, expr } => {
                let lowered = self.emit_expr(expr, placeholder)?;
                let token = match op {
                    UnaryOp::Negate => "-",
                    UnaryOp::Not => "not ",
                    UnaryOp::Length => "#",
                };
                Ok(LoweredExpr {
                    setup: lowered.setup,
                    expr: format!("({token}{})", lowered.expr),
                    reuse_safe: false,
                })
            }
            Expr::TypeAssertion { expr, annotation } => {
                let lowered = self.emit_expr(expr, placeholder)?;
                Ok(LoweredExpr {
                    setup: lowered.setup,
                    expr: format!("({} :: {annotation})", lowered.expr),
                    reuse_safe: false,
                })
            }
            Expr::Binary { left, op, right } => {
                self.emit_binary_expr(left, *op, right, placeholder)
            }
            Expr::Ternary {
                condition,
                then_expr,
                else_expr,
            } => self.emit_ternary_expr(condition, then_expr, else_expr, placeholder),
            Expr::IfElse {
                branches,
                else_expr,
            } => self.emit_if_expr(branches, else_expr, placeholder),
            Expr::Table(fields) => self.emit_table_expr(fields, placeholder),
            Expr::Function(function) => self.emit_function_expr(function, placeholder),
            Expr::Chain { base, segments } => self.emit_chain_expr(base, segments, placeholder),
            Expr::Pipe { left, stages } => self.emit_pipe_expr(left, stages, placeholder),
        }
    }

    fn emit_binary_expr(
        &mut self,
        left: &Expr,
        op: BinaryOp,
        right: &Expr,
        placeholder: Option<&str>,
    ) -> Result<LoweredExpr> {
        if op == BinaryOp::Nullish {
            return self.emit_nullish_expr(left, right, placeholder);
        }

        if matches!(op, BinaryOp::And | BinaryOp::Or) {
            let lowered_left = self.emit_expr(left, placeholder)?;
            let left = self.capture_if_needed(lowered_left, "lhs");
            let right = self.emit_expr(right, placeholder)?;
            if right.setup.is_empty() {
                let mut setup = left.setup;
                let token = if op == BinaryOp::And { "and" } else { "or" };
                return Ok(LoweredExpr {
                    setup: {
                        setup.shrink_to_fit();
                        setup
                    },
                    expr: format!("({} {token} {})", left.expr, right.expr),
                    reuse_safe: false,
                });
            }

            let temp = self.next_temp(if op == BinaryOp::And { "and" } else { "or" });
            let mut setup = left.setup;
            setup.push(format!("local {temp}"));
            match op {
                BinaryOp::And => {
                    setup.push(format!("if {} then", left.expr));
                    setup.extend(self.indent_lines(1, right.setup));
                    setup.push(self.indent(1, &format!("{temp} = {}", right.expr)));
                    setup.push("else".to_string());
                    setup.push(self.indent(1, &format!("{temp} = {}", left.expr)));
                    setup.push("end".to_string());
                }
                BinaryOp::Or => {
                    setup.push(format!("if {} then", left.expr));
                    setup.push(self.indent(1, &format!("{temp} = {}", left.expr)));
                    setup.push("else".to_string());
                    setup.extend(self.indent_lines(1, right.setup));
                    setup.push(self.indent(1, &format!("{temp} = {}", right.expr)));
                    setup.push("end".to_string());
                }
                _ => unreachable!(),
            }
            return Ok(LoweredExpr {
                setup,
                expr: temp,
                reuse_safe: false,
            });
        }

        let left = self.emit_expr(left, placeholder)?;
        let right = self.emit_expr(right, placeholder)?;
        let mut setup = left.setup;
        setup.extend(right.setup);
        Ok(LoweredExpr {
            setup,
            expr: format!("({} {} {})", left.expr, binary_token(op), right.expr),
            reuse_safe: false,
        })
    }

    fn emit_nullish_expr(
        &mut self,
        left: &Expr,
        right: &Expr,
        placeholder: Option<&str>,
    ) -> Result<LoweredExpr> {
        let lowered_left = self.emit_expr(left, placeholder)?;
        let left = self.capture_if_needed(lowered_left, "lhs");
        let right = self.emit_expr(right, placeholder)?;
        if right.setup.is_empty() {
            return Ok(LoweredExpr {
                setup: left.setup,
                expr: format!(
                    "(if {} ~= nil then {} else {})",
                    left.expr, left.expr, right.expr
                ),
                reuse_safe: false,
            });
        }
        let temp = self.next_temp("nullish");
        let mut setup = left.setup;
        setup.push(format!("local {temp}"));
        setup.push(format!("if {} ~= nil then", left.expr));
        setup.push(self.indent(1, &format!("{temp} = {}", left.expr)));
        setup.push("else".to_string());
        setup.extend(self.indent_lines(1, right.setup));
        setup.push(self.indent(1, &format!("{temp} = {}", right.expr)));
        setup.push("end".to_string());
        Ok(LoweredExpr {
            setup,
            expr: temp,
            reuse_safe: false,
        })
    }

    fn emit_ternary_expr(
        &mut self,
        condition: &Expr,
        then_expr: &Expr,
        else_expr: &Expr,
        placeholder: Option<&str>,
    ) -> Result<LoweredExpr> {
        let lowered_condition = self.emit_expr(condition, placeholder)?;
        let condition = self.capture_if_needed(lowered_condition, "cond");
        let then_expr = self.emit_expr(then_expr, placeholder)?;
        let else_expr = self.emit_expr(else_expr, placeholder)?;
        if then_expr.setup.is_empty() && else_expr.setup.is_empty() {
            return Ok(LoweredExpr {
                setup: condition.setup,
                expr: format!(
                    "(if {} then {} else {})",
                    condition.expr, then_expr.expr, else_expr.expr
                ),
                reuse_safe: false,
            });
        }

        let temp = self.next_temp("ternary");
        let mut setup = condition.setup;
        setup.push(format!("local {temp}"));
        setup.push(format!("if {} then", condition.expr));
        setup.extend(self.indent_lines(1, then_expr.setup));
        setup.push(self.indent(1, &format!("{temp} = {}", then_expr.expr)));
        setup.push("else".to_string());
        setup.extend(self.indent_lines(1, else_expr.setup));
        setup.push(self.indent(1, &format!("{temp} = {}", else_expr.expr)));
        setup.push("end".to_string());
        Ok(LoweredExpr {
            setup,
            expr: temp,
            reuse_safe: false,
        })
    }

    fn emit_if_expr(
        &mut self,
        branches: &[(Expr, Expr)],
        else_expr: &Expr,
        placeholder: Option<&str>,
    ) -> Result<LoweredExpr> {
        let temp = self.next_temp("ifexpr");
        let mut setup = vec![format!("local {temp}")];
        for (index, (condition, value)) in branches.iter().enumerate() {
            let lowered_condition = self.emit_expr(condition, placeholder)?;
            let lowered_condition = self.capture_if_needed(lowered_condition, "ifcond");
            setup.extend(lowered_condition.setup);
            let lowered_value = self.emit_expr(value, placeholder)?;
            let keyword = if index == 0 { "if" } else { "elseif" };
            setup.push(format!("{keyword} {} then", lowered_condition.expr));
            setup.extend(self.indent_lines(1, lowered_value.setup));
            setup.push(self.indent(1, &format!("{temp} = {}", lowered_value.expr)));
        }
        let lowered_else = self.emit_expr(else_expr, placeholder)?;
        setup.push("else".to_string());
        setup.extend(self.indent_lines(1, lowered_else.setup));
        setup.push(self.indent(1, &format!("{temp} = {}", lowered_else.expr)));
        setup.push("end".to_string());
        Ok(LoweredExpr {
            setup,
            expr: temp,
            reuse_safe: false,
        })
    }

    fn emit_table_expr(
        &mut self,
        fields: &[TableField],
        placeholder: Option<&str>,
    ) -> Result<LoweredExpr> {
        let mut setup = Vec::new();
        let mut rendered = Vec::new();
        for field in fields {
            match field {
                TableField::Named(name, value) => {
                    let lowered = self.emit_expr(value, placeholder)?;
                    setup.extend(lowered.setup);
                    rendered.push(format!("{name} = {}", lowered.expr));
                }
                TableField::Indexed(key, value) => {
                    let key = self.emit_expr(key, placeholder)?;
                    let value = self.emit_expr(value, placeholder)?;
                    setup.extend(key.setup);
                    setup.extend(value.setup);
                    rendered.push(format!("[{}] = {}", key.expr, value.expr));
                }
                TableField::Value(value) => {
                    let lowered = self.emit_expr(value, placeholder)?;
                    setup.extend(lowered.setup);
                    rendered.push(lowered.expr);
                }
            }
        }
        Ok(LoweredExpr {
            setup,
            expr: format!("{{{}}}", rendered.join(", ")),
            reuse_safe: false,
        })
    }

    fn emit_function_expr(
        &mut self,
        function: &FunctionExpr,
        _placeholder: Option<&str>,
    ) -> Result<LoweredExpr> {
        let (params, prologue) = self.lower_params(&function.params)?;
        self.const_scopes.push(HashSet::new());
        for name in self.collect_param_names(&function.params) {
            self.declare_name(&name, false);
        }
        let body = self.emit_block(&function.body, 1)?;
        self.const_scopes.pop();
        let generics = function.generics.clone().unwrap_or_default();
        let return_type = function
            .return_type
            .as_ref()
            .map(|text| format!(": {text}"))
            .unwrap_or_default();
        let mut lines = vec![format!(
            "function{generics}({}){return_type}",
            params.join(", ")
        )];
        for line in prologue {
            lines.push(self.indent(1, &line));
        }
        if !body.is_empty() {
            lines.push(body);
        }
        lines.push("end".to_string());
        Ok(LoweredExpr {
            setup: Vec::new(),
            expr: lines.join("\n"),
            reuse_safe: false,
        })
    }

    fn emit_chain_expr(
        &mut self,
        base: &Expr,
        segments: &[ChainSegment],
        placeholder: Option<&str>,
    ) -> Result<LoweredExpr> {
        let has_safe = segments.iter().any(|segment| match segment {
            ChainSegment::Field { safe, .. }
            | ChainSegment::Index { safe, .. }
            | ChainSegment::MethodCall { safe, .. } => *safe,
            ChainSegment::Call { .. } => false,
        });
        if has_safe {
            return self.emit_optional_chain_expr(base, segments, placeholder);
        }

        let mut lowered = self.emit_expr(base, placeholder)?;
        let mut expr = lowered.expr.clone();
        for segment in segments {
            match segment {
                ChainSegment::Field { name, .. } => {
                    expr = format!("{expr}.{name}");
                }
                ChainSegment::Index { expr: index, .. } => {
                    let index = self.emit_expr(index, placeholder)?;
                    lowered.setup.extend(index.setup);
                    expr = format!("{expr}[{}]", index.expr);
                }
                ChainSegment::Call { args } => {
                    let args = self.emit_args(args, placeholder)?;
                    lowered.setup.extend(args.0);
                    expr = format!("{expr}({})", args.1.join(", "));
                }
                ChainSegment::MethodCall { name, args, .. } => {
                    let args = self.emit_args(args, placeholder)?;
                    lowered.setup.extend(args.0);
                    expr = format!("{expr}:{name}({})", args.1.join(", "));
                }
            }
        }
        Ok(LoweredExpr {
            setup: lowered.setup,
            expr,
            reuse_safe: false,
        })
    }

    fn emit_optional_chain_expr(
        &mut self,
        base: &Expr,
        segments: &[ChainSegment],
        placeholder: Option<&str>,
    ) -> Result<LoweredExpr> {
        let lowered_base = self.emit_expr(base, placeholder)?;
        let base = self.capture_if_needed(lowered_base, "opt");
        let result = self.next_temp("opt");
        let current = self.next_temp("cur");
        let mut setup = base.setup;
        setup.push(format!("local {result} = nil"));
        setup.push("do".to_string());
        setup.push(self.indent(1, &format!("local {current} = {}", base.expr)));
        let safe_positions = segments
            .iter()
            .enumerate()
            .filter_map(|(index, segment)| match segment {
                ChainSegment::Field { safe, .. }
                | ChainSegment::Index { safe, .. }
                | ChainSegment::MethodCall { safe, .. }
                    if *safe =>
                {
                    Some(index)
                }
                _ => None,
            })
            .collect::<Vec<_>>();

        let mut nesting = 1usize;
        for (index, segment) in segments.iter().enumerate() {
            let is_safe = safe_positions.contains(&index);
            if is_safe {
                setup.push(self.indent(nesting, &format!("if {current} ~= nil then")));
                nesting += 1;
            }
            match segment {
                ChainSegment::Field { name, .. } => {
                    setup.push(self.indent(nesting, &format!("{current} = {current}.{name}")));
                }
                ChainSegment::Index { expr, .. } => {
                    let lowered = self.emit_expr(expr, placeholder)?;
                    setup.extend(self.indent_lines(nesting, lowered.setup));
                    setup.push(
                        self.indent(nesting, &format!("{current} = {current}[{}]", lowered.expr)),
                    );
                }
                ChainSegment::Call { args } => {
                    let (arg_setup, arg_values) = self.emit_args(args, placeholder)?;
                    setup.extend(self.indent_lines(nesting, arg_setup));
                    setup.push(self.indent(
                        nesting,
                        &format!("{current} = {current}({})", arg_values.join(", ")),
                    ));
                }
                ChainSegment::MethodCall { name, args, .. } => {
                    let (arg_setup, arg_values) = self.emit_args(args, placeholder)?;
                    setup.extend(self.indent_lines(nesting, arg_setup));
                    setup.push(self.indent(
                        nesting,
                        &format!("{current} = {current}:{name}({})", arg_values.join(", ")),
                    ));
                }
            }
        }
        setup.push(self.indent(nesting, &format!("{result} = {current}")));
        while nesting > 1 {
            nesting -= 1;
            setup.push(self.indent(nesting, "end"));
        }
        setup.push("end".to_string());
        Ok(LoweredExpr {
            setup,
            expr: result,
            reuse_safe: false,
        })
    }

    fn emit_pipe_expr(
        &mut self,
        left: &Expr,
        stages: &[PipeStage],
        placeholder: Option<&str>,
    ) -> Result<LoweredExpr> {
        let mut current = self.emit_expr(left, placeholder)?;
        if stages.len() <= 3 {
            for stage in stages {
                current = self.apply_pipe_stage(current, stage, placeholder)?;
            }
            return Ok(current);
        }

        let current = self.capture_if_needed(current, "p");
        let mut setup = current.setup;
        let mut prev_name = self.next_temp("p");
        setup.push(format!("local {prev_name} = {}", current.expr));

        for stage in stages {
            let applied = self.apply_pipe_stage(
                LoweredExpr {
                    setup: Vec::new(),
                    expr: prev_name.clone(),
                    reuse_safe: true,
                },
                stage,
                placeholder,
            )?;
            setup.extend(applied.setup);
            let next_name = self.next_temp("p");
            setup.push(format!("local {next_name} = {}", applied.expr));
            prev_name = next_name;
        }

        Ok(LoweredExpr {
            setup,
            expr: prev_name,
            reuse_safe: false,
        })
    }

    fn apply_pipe_stage(
        &mut self,
        input: LoweredExpr,
        stage: &PipeStage,
        placeholder: Option<&str>,
    ) -> Result<LoweredExpr> {
        let input = self.capture_if_needed(input, "pipe");
        match stage {
            PipeStage::Method { name, args } => {
                let (setup, values) = self.emit_args(args, placeholder)?;
                let mut combined = input.setup;
                combined.extend(setup);
                Ok(LoweredExpr {
                    setup: combined,
                    expr: format!("{}:{name}({})", input.expr, values.join(", ")),
                    reuse_safe: false,
                })
            }
            PipeStage::Expr { callee } => {
                let callee = self.emit_expr(callee, placeholder)?;
                let mut setup = input.setup;
                setup.extend(callee.setup);
                Ok(LoweredExpr {
                    setup,
                    expr: format!("{}({})", callee.expr, input.expr),
                    reuse_safe: false,
                })
            }
            PipeStage::Call { callee, args } => {
                let callee = self.emit_expr(callee, placeholder)?;
                let mut setup = input.setup;
                setup.extend(callee.setup);
                let mut values = Vec::new();
                let contains_placeholder = args.iter().any(|arg| self.contains_placeholder(arg));
                if !contains_placeholder {
                    values.push(input.expr.clone());
                }
                for arg in args {
                    let lowered = self.emit_expr(arg, Some(&input.expr))?;
                    setup.extend(lowered.setup);
                    values.push(lowered.expr);
                }
                Ok(LoweredExpr {
                    setup,
                    expr: format!("{}({})", callee.expr, values.join(", ")),
                    reuse_safe: false,
                })
            }
        }
    }

    fn emit_args(
        &mut self,
        args: &[Expr],
        placeholder: Option<&str>,
    ) -> Result<(Vec<String>, Vec<String>)> {
        let mut setup = Vec::new();
        let mut values = Vec::new();
        for arg in args {
            let lowered = self.emit_expr(arg, placeholder)?;
            setup.extend(lowered.setup);
            values.push(lowered.expr);
        }
        Ok((setup, values))
    }

    fn lower_params(&mut self, params: &[Param]) -> Result<(Vec<String>, Vec<String>)> {
        let mut rendered = Vec::new();
        let mut prologue = Vec::new();
        for (index, param) in params.iter().enumerate() {
            match param {
                Param::VarArg(type_annotation) => {
                    let annotation = type_annotation
                        .as_ref()
                        .map(|text| format!(": {text}"))
                        .unwrap_or_default();
                    rendered.push(format!("...{annotation}"));
                }
                Param::Binding(binding) => {
                    if let Some(name) = self.pattern_name(&binding.pattern) {
                        let annotation = binding
                            .type_annotation
                            .as_ref()
                            .map(|text| format!(": {text}"))
                            .unwrap_or_default();
                        rendered.push(format!("{name}{annotation}"));
                    } else {
                        let temp = format!("_param{index}");
                        let annotation = binding
                            .type_annotation
                            .as_ref()
                            .map(|text| format!(": {text}"))
                            .unwrap_or_default();
                        rendered.push(format!("{temp}{annotation}"));
                        self.emit_pattern_local(
                            &mut prologue,
                            0,
                            &binding.pattern,
                            &None,
                            &temp,
                            true,
                        )?;
                    }
                }
            }
        }
        Ok((rendered, prologue))
    }

    fn collect_param_names(&self, params: &[Param]) -> Vec<String> {
        let mut names = Vec::new();
        for param in params {
            if let Param::Binding(binding) = param {
                names.extend(self.pattern_names(&binding.pattern));
            }
        }
        names
    }

    fn contains_placeholder(&self, expr: &Expr) -> bool {
        match expr {
            Expr::Name(name) => name == "_",
            Expr::Paren(inner) => self.contains_placeholder(inner),
            Expr::Unary { expr, .. } => self.contains_placeholder(expr),
            Expr::TypeAssertion { expr, .. } => self.contains_placeholder(expr),
            Expr::Binary { left, right, .. } => {
                self.contains_placeholder(left) || self.contains_placeholder(right)
            }
            Expr::Ternary {
                condition,
                then_expr,
                else_expr,
            } => {
                self.contains_placeholder(condition)
                    || self.contains_placeholder(then_expr)
                    || self.contains_placeholder(else_expr)
            }
            Expr::Table(fields) => fields.iter().any(|field| match field {
                TableField::Named(_, value) | TableField::Value(value) => {
                    self.contains_placeholder(value)
                }
                TableField::Indexed(key, value) => {
                    self.contains_placeholder(key) || self.contains_placeholder(value)
                }
            }),
            Expr::IfElse {
                branches,
                else_expr,
            } => {
                branches.iter().any(|(condition, value)| {
                    self.contains_placeholder(condition) || self.contains_placeholder(value)
                }) || self.contains_placeholder(else_expr)
            }
            Expr::Chain { base, segments } => {
                self.contains_placeholder(base)
                    || segments.iter().any(|segment| match segment {
                        ChainSegment::Field { .. } => false,
                        ChainSegment::Index { expr, .. } => self.contains_placeholder(expr),
                        ChainSegment::Call { args } | ChainSegment::MethodCall { args, .. } => {
                            args.iter().any(|arg| self.contains_placeholder(arg))
                        }
                    })
            }
            Expr::Pipe { left, stages } => {
                self.contains_placeholder(left)
                    || stages.iter().any(|stage| match stage {
                        PipeStage::Method { args, .. } | PipeStage::Call { args, .. } => {
                            args.iter().any(|arg| self.contains_placeholder(arg))
                        }
                        PipeStage::Expr { callee } => self.contains_placeholder(callee),
                    })
            }
            Expr::Function(_)
            | Expr::Nil
            | Expr::Bool(_)
            | Expr::Number(_)
            | Expr::String(_)
            | Expr::VarArg => false,
        }
    }

    fn capture_if_needed(&mut self, lowered: LoweredExpr, prefix: &str) -> LoweredExpr {
        if lowered.reuse_safe {
            lowered
        } else {
            let temp = self.next_temp(prefix);
            let mut setup = lowered.setup;
            setup.push(format!("local {temp} = {}", lowered.expr));
            LoweredExpr {
                setup,
                expr: temp,
                reuse_safe: true,
            }
        }
    }

    fn pattern_name<'a>(&self, pattern: &'a Pattern) -> Option<&'a str> {
        match pattern {
            Pattern::Name(name) => Some(name),
            _ => None,
        }
    }

    fn pattern_names(&self, pattern: &Pattern) -> Vec<String> {
        let mut names = Vec::new();
        match pattern {
            Pattern::Name(name) => names.push(name.clone()),
            Pattern::Table { entries, rest } => {
                for entry in entries {
                    names.extend(self.pattern_names(&entry.binding.target));
                }
                if let Some(rest) = rest {
                    names.push(rest.clone());
                }
            }
            Pattern::Array { items, rest } => {
                for item in items {
                    if let Some(binding) = &item.binding {
                        names.extend(self.pattern_names(&binding.target));
                    }
                }
                if let Some(rest) = rest {
                    names.push(rest.clone());
                }
            }
        }
        names
    }

    fn declare_local_names(&mut self, pattern: &Pattern, is_const: bool) {
        for name in self.pattern_names(pattern) {
            self.declare_name(&name, is_const);
        }
    }

    fn declare_name(&mut self, name: &str, is_const: bool) {
        if is_const {
            if let Some(scope) = self.const_scopes.last_mut() {
                scope.insert(name.to_string());
            }
        }
    }

    fn check_const_target(&mut self, target: &AssignTarget) {
        if let AssignTarget::Name(name) = target {
            if self
                .const_scopes
                .iter()
                .rev()
                .any(|scope| scope.contains(name))
            {
                self.errors.push(format!("cannot assign to const `{name}`"));
            }
        }
    }

    fn simple_expr(&self, expr: impl Into<String>, reuse_safe: bool) -> LoweredExpr {
        LoweredExpr {
            setup: Vec::new(),
            expr: expr.into(),
            reuse_safe,
        }
    }

    fn next_temp(&mut self, prefix: &str) -> String {
        let value = format!("_{}{}", prefix, self.temp_counter);
        self.temp_counter += 1;
        value
    }

    fn push_setup(&self, output: &mut Vec<String>, indent: usize, setup: Vec<String>) {
        for line in setup {
            self.push_multiline(output, indent, &line);
        }
    }

    fn push_multiline(&self, output: &mut Vec<String>, indent: usize, text: &str) {
        for line in text.lines() {
            if line.is_empty() {
                output.push(String::new());
            } else {
                output.push(self.indent(indent, line));
            }
        }
    }

    fn indent_lines(&self, indent: usize, lines: Vec<String>) -> Vec<String> {
        let mut result = Vec::new();
        for line in lines {
            for inner in line.lines() {
                result.push(self.indent(indent, inner));
            }
        }
        result
    }

    fn indent(&self, indent: usize, text: &str) -> String {
        format!("{}{}", "    ".repeat(indent), text)
    }
}

fn binary_token(op: BinaryOp) -> &'static str {
    match op {
        BinaryOp::Or => "or",
        BinaryOp::And => "and",
        BinaryOp::Less => "<",
        BinaryOp::LessEqual => "<=",
        BinaryOp::Greater => ">",
        BinaryOp::GreaterEqual => ">=",
        BinaryOp::Equal => "==",
        BinaryOp::NotEqual => "~=",
        BinaryOp::Concat => "..",
        BinaryOp::Add => "+",
        BinaryOp::Subtract => "-",
        BinaryOp::Multiply => "*",
        BinaryOp::Divide => "/",
        BinaryOp::FloorDivide => "//",
        BinaryOp::Modulo => "%",
        BinaryOp::Power => "^",
        BinaryOp::Nullish => "??",
    }
}

fn compound_token(op: CompoundOp) -> &'static str {
    match op {
        CompoundOp::Add => "+",
        CompoundOp::Subtract => "-",
        CompoundOp::Multiply => "*",
        CompoundOp::Divide => "/",
        CompoundOp::FloorDivide => "//",
        CompoundOp::Modulo => "%",
        CompoundOp::Power => "^",
        CompoundOp::Concat => "..",
    }
}
