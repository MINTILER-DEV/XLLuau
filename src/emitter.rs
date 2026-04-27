use std::collections::{HashMap, HashSet};

use crate::{
    ast::*,
    compiler::{CompilerError, Result},
};

pub struct Emitter {
    temp_counter: usize,
    const_scopes: Vec<HashSet<String>>,
    type_scopes: Vec<HashMap<String, String>>,
    type_defs: HashMap<String, TypeShape>,
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

#[derive(Debug, Clone)]
struct GroupedSwitchCase {
    values: Vec<Expr>,
    block: Block,
}

#[derive(Debug, Clone)]
enum TypeShape {
    LiteralUnion(Vec<String>),
    Enum {
        values: Vec<String>,
        members: HashMap<String, String>,
    },
    DiscriminatedUnion {
        field: String,
        variants: Vec<String>,
    },
}

impl Emitter {
    pub fn new() -> Self {
        Self {
            temp_counter: 0,
            const_scopes: vec![HashSet::new()],
            type_scopes: vec![HashMap::new()],
            type_defs: HashMap::new(),
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
        self.type_scopes.push(HashMap::new());
        let mut lines = Vec::new();
        for stmt in block {
            let chunk = self.emit_stmt(stmt, indent)?;
            if !chunk.is_empty() {
                lines.push(chunk);
            }
        }
        self.const_scopes.pop();
        self.type_scopes.pop();
        Ok(lines.join("\n"))
    }

    fn emit_stmt(&mut self, stmt: &Stmt, indent: usize) -> Result<String> {
        match stmt {
            Stmt::Local(local) => self.emit_local(local, indent),
            Stmt::Function(function) => self.emit_function_stmt(function, indent),
            Stmt::Enum(decl) => {
                self.register_enum_type(decl);
                self.emit_enum_decl(decl, indent)
            }
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
            Stmt::Switch(switch_stmt) => self.emit_switch_stmt(switch_stmt, indent),
            Stmt::Match(match_stmt) => self.emit_match_stmt(match_stmt, indent),
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
            Stmt::Fallthrough => {
                self.errors.push(
                    "`fallthrough` is only valid as the last statement in a switch case"
                        .to_string(),
                );
                Ok(String::new())
            }
            Stmt::TypeAlias { raw } => {
                self.register_type_alias(raw);
                Ok(self.indent(indent, raw))
            }
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
            self.declare_pattern_types(&binding.pattern, &binding.type_annotation);
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
        self.type_scopes.push(HashMap::new());
        for name in self.collect_param_names(&function.params) {
            self.declare_name(&name, false);
        }
        self.declare_param_types(&function.params);
        let mut body_lines = Vec::new();
        for line in prologue {
            body_lines.push(self.indent(indent + 1, &line));
        }
        let body = self.emit_block(&function.body, indent + 1)?;
        if !body.is_empty() {
            body_lines.push(body);
        }
        self.const_scopes.pop();
        self.type_scopes.pop();

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

    fn emit_enum_decl(&mut self, decl: &EnumDecl, indent: usize) -> Result<String> {
        let number_backed = decl
            .base_type
            .as_deref()
            .map(|base| base.trim() == "number")
            .unwrap_or(false);

        let mut variants = Vec::new();
        let mut members = Vec::new();
        for member in &decl.members {
            let value = if let Some(value) = &member.value {
                let lowered = self.emit_expr(value, None)?;
                if !lowered.setup.is_empty() {
                    self.errors.push(format!(
                        "enum member `{}` value must be a simple expression",
                        member.name
                    ));
                }
                lowered.expr
            } else if number_backed {
                self.errors.push(format!(
                    "number-backed enum `{}` member `{}` requires an explicit value",
                    decl.name, member.name
                ));
                "0".to_string()
            } else {
                format!("\"{}\"", member.name)
            };
            variants.push(value.clone());
            members.push((member.name.clone(), value));
        }

        let type_alias = if number_backed {
            format!("type {} = number", decl.name)
        } else {
            format!("type {} = {}", decl.name, variants.join(" | "))
        };

        let mut parts = vec![self.indent(indent, &type_alias)];
        parts.push(self.indent(indent, &format!("local {} = table.freeze({{", decl.name)));
        for (name, value) in members {
            parts.push(self.indent(indent + 1, &format!("{name} = {value} :: {},", decl.name)));
        }
        parts.push(self.indent(indent, "})"));
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

    fn emit_switch_stmt(&mut self, switch_stmt: &SwitchStmt, indent: usize) -> Result<String> {
        self.check_switch_exhaustiveness(switch_stmt);
        let lowered_value = self.emit_expr(&switch_stmt.value, None)?;
        let switch_name = self.next_temp("sw");
        let mut parts = Vec::new();
        self.push_setup(&mut parts, indent, lowered_value.setup);
        parts.push(self.indent(
            indent,
            &format!("local {switch_name} = {}", lowered_value.expr),
        ));

        let groups = self.group_switch_cases(&switch_stmt.cases);
        for (index, group) in groups.iter().enumerate() {
            let (condition_setup, condition) =
                self.emit_switch_condition(&switch_name, &group.values)?;
            self.push_setup(&mut parts, indent, condition_setup);
            let keyword = if index == 0 { "if" } else { "elseif" };
            parts.push(self.indent(indent, &format!("{keyword} {condition} then")));
            let body = self.emit_block(&group.block, indent + 1)?;
            if !body.is_empty() {
                parts.push(body);
            }
        }
        if let Some(default) = &switch_stmt.default {
            if groups.is_empty() {
                return self.emit_block(default, indent);
            }
            parts.push(self.indent(indent, "else"));
            let body = self.emit_block(default, indent + 1)?;
            if !body.is_empty() {
                parts.push(body);
            }
        }
        if !groups.is_empty() {
            parts.push(self.indent(indent, "end"));
        }
        Ok(parts.join("\n"))
    }

    fn emit_match_stmt(&mut self, match_stmt: &MatchStmt, indent: usize) -> Result<String> {
        self.check_match_exhaustiveness(match_stmt);
        let lowered_value = self.emit_expr(&match_stmt.value, None)?;
        let lowered_value = self.capture_if_needed(lowered_value, "m");
        let mut parts = Vec::new();
        self.push_setup(&mut parts, indent, lowered_value.setup);
        let matched_name = self.next_temp("matched");
        parts.push(self.indent(indent, &format!("local {matched_name} = false")));

        for case in &match_stmt.cases {
            let bindings_name = self.next_temp("mbind");
            let cond_name = self.next_temp("mcond");
            parts.push(self.indent(indent, "do"));
            parts.push(self.indent(indent + 1, &format!("local {bindings_name} = {{}}")));
            let (pattern_setup, pattern_expr) = self.emit_match_pattern_condition(
                &case.pattern,
                &lowered_value.expr,
                &bindings_name,
            )?;
            self.push_setup(&mut parts, indent + 1, pattern_setup);
            parts.push(self.indent(
                indent + 1,
                &format!("local {cond_name} = (not {matched_name}) and ({pattern_expr})"),
            ));
            if let Some(guard) = &case.guard {
                parts.push(self.indent(indent + 1, &format!("if {cond_name} then")));
                self.emit_match_bindings(&mut parts, indent + 2, &bindings_name, &case.pattern);
                let lowered_guard = self.emit_expr(guard, None)?;
                self.push_setup(&mut parts, indent + 2, lowered_guard.setup);
                parts.push(
                    self.indent(indent + 2, &format!("{cond_name} = {}", lowered_guard.expr)),
                );
                parts.push(self.indent(indent + 1, "end"));
            }

            parts.push(self.indent(indent + 1, &format!("if {cond_name} then")));
            self.emit_match_bindings(&mut parts, indent + 2, &bindings_name, &case.pattern);
            parts.push(self.indent(indent + 2, &format!("{matched_name} = true")));
            let body = self.emit_block(&case.block, indent + 2)?;
            if !body.is_empty() {
                parts.push(body);
            }
            parts.push(self.indent(indent + 1, "end"));
            parts.push(self.indent(indent, "end"));
        }
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

    fn group_switch_cases(&self, cases: &[SwitchCase]) -> Vec<GroupedSwitchCase> {
        let mut groups = Vec::new();
        let mut pending_values = Vec::new();

        for case in cases {
            pending_values.push(case.value.clone());
            if case.fallthrough || case.block.is_empty() {
                continue;
            }
            groups.push(GroupedSwitchCase {
                values: std::mem::take(&mut pending_values),
                block: case.block.clone(),
            });
        }

        groups
    }

    fn emit_switch_condition(
        &mut self,
        switch_value: &str,
        values: &[Expr],
    ) -> Result<(Vec<String>, String)> {
        let mut setup = Vec::new();
        let mut conditions = Vec::new();
        for value in values {
            let lowered = self.emit_expr(value, None)?;
            setup.extend(lowered.setup);
            conditions.push(format!("{switch_value} == {}", lowered.expr));
        }
        Ok((setup, conditions.join(" or ")))
    }

    fn emit_match_pattern_condition(
        &mut self,
        pattern: &MatchPattern,
        value_expr: &str,
        bindings_name: &str,
    ) -> Result<(Vec<String>, String)> {
        match pattern {
            MatchPattern::Literal(expr) => {
                let lowered = self.emit_expr(expr, None)?;
                Ok((lowered.setup, format!("{value_expr} == {}", lowered.expr)))
            }
            MatchPattern::Bind(name) => Ok((
                vec![format!("{bindings_name}[\"{name}\"] = {value_expr}")],
                "true".to_string(),
            )),
            MatchPattern::Table(fields) => {
                let mut setup = Vec::new();
                let mut conditions = vec![format!("type({value_expr}) == \"table\"")];
                for field in fields {
                    let access = format!("{value_expr}.{}", field.key);
                    let (field_setup, field_condition) =
                        self.emit_match_pattern_condition(&field.pattern, &access, bindings_name)?;
                    setup.extend(field_setup);
                    conditions.push(field_condition);
                }
                Ok((setup, conditions.join(" and ")))
            }
        }
    }

    fn emit_match_bindings(
        &mut self,
        parts: &mut Vec<String>,
        indent: usize,
        bindings_name: &str,
        pattern: &MatchPattern,
    ) {
        for name in self.match_pattern_bindings(pattern) {
            parts.push(self.indent(
                indent,
                &format!("local {name} = {bindings_name}[\"{name}\"]"),
            ));
        }
    }

    fn emit_comprehension_clauses(
        &mut self,
        setup: &mut Vec<String>,
        indent: usize,
        table_name: &str,
        kind: &TableComprehensionKind,
        clauses: &[ComprehensionClause],
        placeholder: Option<&str>,
    ) -> Result<()> {
        if clauses.is_empty() {
            return self.emit_comprehension_insert(setup, indent, table_name, kind, placeholder);
        }

        match &clauses[0] {
            ComprehensionClause::GenericFor {
                bindings,
                iterables,
            } => {
                let mut iter_setup = Vec::new();
                let mut iterable_values = Vec::new();
                for iterable in iterables {
                    let lowered = self.emit_expr(iterable, placeholder)?;
                    iter_setup.extend(lowered.setup);
                    iterable_values.push(lowered.expr);
                }
                setup.extend(self.indent_lines(indent, iter_setup));

                let mut loop_names = Vec::new();
                let mut prologue = Vec::new();
                for binding in bindings {
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

                setup.push(self.indent(
                    indent,
                    &format!(
                        "for {} in {} do",
                        loop_names.join(", "),
                        iterable_values.join(", ")
                    ),
                ));
                setup.extend(self.indent_lines(indent + 1, prologue));
                self.emit_comprehension_clauses(
                    setup,
                    indent + 1,
                    table_name,
                    kind,
                    &clauses[1..],
                    placeholder,
                )?;
                setup.push(self.indent(indent, "end"));
            }
            ComprehensionClause::NumericFor {
                name,
                start,
                end,
                step,
            } => {
                let lowered_start = self.emit_expr(start, placeholder)?;
                let lowered_end = self.emit_expr(end, placeholder)?;
                let lowered_step = if let Some(step) = step {
                    Some(self.emit_expr(step, placeholder)?)
                } else {
                    None
                };
                setup.extend(self.indent_lines(indent, lowered_start.setup));
                setup.extend(self.indent_lines(indent, lowered_end.setup));
                if let Some(step) = &lowered_step {
                    setup.extend(self.indent_lines(indent, step.setup.clone()));
                }
                let range = if let Some(step) = lowered_step {
                    format!(
                        "{}, {}, {}",
                        lowered_start.expr, lowered_end.expr, step.expr
                    )
                } else {
                    format!("{}, {}", lowered_start.expr, lowered_end.expr)
                };
                setup.push(self.indent(indent, &format!("for {name} = {range} do")));
                self.emit_comprehension_clauses(
                    setup,
                    indent + 1,
                    table_name,
                    kind,
                    &clauses[1..],
                    placeholder,
                )?;
                setup.push(self.indent(indent, "end"));
            }
            ComprehensionClause::Filter(condition) => {
                let lowered = self.emit_expr(condition, placeholder)?;
                setup.extend(self.indent_lines(indent, lowered.setup));
                setup.push(self.indent(indent, &format!("if {} then", lowered.expr)));
                self.emit_comprehension_insert(setup, indent + 1, table_name, kind, placeholder)?;
                setup.push(self.indent(indent, "end"));
            }
        }
        Ok(())
    }

    fn emit_comprehension_insert(
        &mut self,
        setup: &mut Vec<String>,
        indent: usize,
        table_name: &str,
        kind: &TableComprehensionKind,
        placeholder: Option<&str>,
    ) -> Result<()> {
        match kind {
            TableComprehensionKind::Array { value } => {
                let lowered = self.emit_expr(value, placeholder)?;
                setup.extend(self.indent_lines(indent, lowered.setup));
                setup.push(self.indent(
                    indent,
                    &format!("table.insert({table_name}, {})", lowered.expr),
                ));
            }
            TableComprehensionKind::Map { key, value } => {
                let lowered_key = self.emit_expr(key, placeholder)?;
                let lowered_value = self.emit_expr(value, placeholder)?;
                setup.extend(self.indent_lines(indent, lowered_key.setup));
                setup.extend(self.indent_lines(indent, lowered_value.setup));
                setup.push(self.indent(
                    indent,
                    &format!(
                        "{table_name}[{}] = {}",
                        lowered_key.expr, lowered_value.expr
                    ),
                ));
            }
        }
        Ok(())
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
            Expr::DoExpr { block, result } => self.emit_do_expr(block, result, placeholder),
            Expr::SwitchExpr {
                value,
                cases,
                default,
            } => self.emit_switch_expr(value, cases, default, placeholder),
            Expr::Table(fields) => self.emit_table_expr(fields, placeholder),
            Expr::Function(function) => self.emit_function_expr(function, placeholder),
            Expr::Chain { base, segments } => self.emit_chain_expr(base, segments, placeholder),
            Expr::Pipe { left, stages } => self.emit_pipe_expr(left, stages, placeholder),
            Expr::Comprehension(comprehension) => {
                self.emit_comprehension_expr(comprehension, placeholder)
            }
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

    fn emit_do_expr(
        &mut self,
        block: &Block,
        result: &Expr,
        placeholder: Option<&str>,
    ) -> Result<LoweredExpr> {
        let result_name = self.next_temp("de");
        let mut setup = vec![format!("local {result_name}")];
        setup.push("do".to_string());
        let body = self.emit_block(block, 1)?;
        if !body.is_empty() {
            setup.extend(body.lines().map(ToString::to_string));
        }
        let lowered_result = self.emit_expr(result, placeholder)?;
        setup.extend(self.indent_lines(1, lowered_result.setup));
        setup.push(self.indent(1, &format!("{result_name} = {}", lowered_result.expr)));
        setup.push("end".to_string());
        Ok(LoweredExpr {
            setup,
            expr: result_name,
            reuse_safe: false,
        })
    }

    fn emit_switch_expr(
        &mut self,
        value: &Expr,
        cases: &[SwitchExprCase],
        default: &Expr,
        placeholder: Option<&str>,
    ) -> Result<LoweredExpr> {
        let lowered_value = self.emit_expr(value, placeholder)?;
        let result_name = self.next_temp("swexpr");
        let switch_name = self.next_temp("sw");
        let mut setup = lowered_value.setup;
        setup.push(format!("local {switch_name} = {}", lowered_value.expr));
        setup.push(format!("local {result_name}"));
        for (index, case) in cases.iter().enumerate() {
            let lowered_case = self.emit_expr(&case.value, placeholder)?;
            setup.extend(lowered_case.setup);
            let lowered_result = self.emit_expr(&case.result, placeholder)?;
            let keyword = if index == 0 { "if" } else { "elseif" };
            setup.push(format!(
                "{keyword} {} == {} then",
                switch_name, lowered_case.expr
            ));
            setup.extend(self.indent_lines(1, lowered_result.setup));
            setup.push(self.indent(1, &format!("{result_name} = {}", lowered_result.expr)));
        }
        let lowered_default = self.emit_expr(default, placeholder)?;
        setup.push("else".to_string());
        setup.extend(self.indent_lines(1, lowered_default.setup));
        setup.push(self.indent(1, &format!("{result_name} = {}", lowered_default.expr)));
        setup.push("end".to_string());
        Ok(LoweredExpr {
            setup,
            expr: result_name,
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

    fn emit_comprehension_expr(
        &mut self,
        comprehension: &TableComprehension,
        placeholder: Option<&str>,
    ) -> Result<LoweredExpr> {
        let table_name = self.next_temp("comp");
        let mut setup = vec![format!("local {table_name} = {{}}")];
        self.emit_comprehension_clauses(
            &mut setup,
            0,
            &table_name,
            &comprehension.kind,
            &comprehension.clauses,
            placeholder,
        )?;
        Ok(LoweredExpr {
            setup,
            expr: table_name,
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

    fn register_enum_type(&mut self, decl: &EnumDecl) {
        let mut values = Vec::new();
        let mut members = HashMap::new();
        let number_backed = decl
            .base_type
            .as_deref()
            .map(|base| base.trim() == "number")
            .unwrap_or(false);

        for member in &decl.members {
            let value = if let Some(value) = &member.value {
                self.literal_key(value)
                    .unwrap_or_else(|| self.expr_key(value).unwrap_or_default())
            } else if number_backed {
                "0".to_string()
            } else {
                format!("\"{}\"", member.name)
            };
            values.push(value.clone());
            members.insert(member.name.clone(), value);
        }

        self.type_defs
            .insert(decl.name.clone(), TypeShape::Enum { values, members });
    }

    fn register_type_alias(&mut self, raw: &str) {
        let Some((name, shape)) = self.parse_type_alias_shape(raw) else {
            return;
        };
        self.type_defs.insert(name, shape);
    }

    fn parse_type_alias_shape(&self, raw: &str) -> Option<(String, TypeShape)> {
        let trimmed = raw.trim();
        let trimmed = trimmed.strip_prefix("export ").unwrap_or(trimmed);
        let rest = trimmed.strip_prefix("type ")?;
        let eq_index = rest.find('=')?;
        let name = rest[..eq_index]
            .trim()
            .split('<')
            .next()?
            .trim()
            .to_string();
        let rhs = rest[eq_index + 1..].trim();

        if let Some(values) = self.parse_literal_union(rhs) {
            return Some((name, TypeShape::LiteralUnion(values)));
        }
        if let Some((field, variants)) = self.parse_discriminated_union(rhs) {
            return Some((name, TypeShape::DiscriminatedUnion { field, variants }));
        }
        None
    }

    fn parse_literal_union(&self, rhs: &str) -> Option<Vec<String>> {
        let parts = self.split_top_level(rhs, '|');
        if parts.len() < 2 {
            return None;
        }
        let mut values = Vec::new();
        for part in parts {
            let piece = part.trim();
            if self.is_literal_text(piece) {
                values.push(piece.to_string());
            } else {
                return None;
            }
        }
        Some(values)
    }

    fn parse_discriminated_union(&self, rhs: &str) -> Option<(String, Vec<String>)> {
        let parts = self.split_top_level(rhs, '|');
        if parts.len() < 2 {
            return None;
        }

        let mut parsed_variants = Vec::new();
        for part in parts {
            parsed_variants.push(self.parse_type_table_literal_fields(part.trim())?);
        }

        let candidate_fields = parsed_variants.first()?.keys().cloned().collect::<Vec<_>>();
        for field in candidate_fields {
            let mut variants = Vec::new();
            let mut valid = true;
            for variant in &parsed_variants {
                let Some(value) = variant.get(&field) else {
                    valid = false;
                    break;
                };
                variants.push(value.clone());
            }
            if valid {
                return Some((field, variants));
            }
        }
        None
    }

    fn parse_type_table_literal_fields(&self, text: &str) -> Option<HashMap<String, String>> {
        let trimmed = text.trim();
        if !trimmed.starts_with('{') || !trimmed.ends_with('}') {
            return None;
        }
        let inner = &trimmed[1..trimmed.len() - 1];
        let mut fields = HashMap::new();
        for part in self.split_top_level(inner, ',') {
            let piece = part.trim();
            if piece.is_empty() {
                continue;
            }
            let colon_index = piece.find(':')?;
            let key = piece[..colon_index].trim().to_string();
            let value = piece[colon_index + 1..].trim();
            if self.is_literal_text(value) {
                fields.insert(key, value.to_string());
            }
        }
        if fields.is_empty() {
            None
        } else {
            Some(fields)
        }
    }

    fn split_top_level<'a>(&self, text: &'a str, delimiter: char) -> Vec<&'a str> {
        let mut parts = Vec::new();
        let mut start = 0usize;
        let mut paren = 0usize;
        let mut brace = 0usize;
        let mut bracket = 0usize;
        let mut angle = 0usize;
        let mut in_string: Option<char> = None;
        let mut escaped = false;

        for (index, ch) in text.char_indices() {
            if let Some(quote) = in_string {
                if escaped {
                    escaped = false;
                    continue;
                }
                if ch == '\\' {
                    escaped = true;
                } else if ch == quote {
                    in_string = None;
                }
                continue;
            }

            match ch {
                '"' | '\'' | '`' => in_string = Some(ch),
                '(' => paren += 1,
                ')' => paren = paren.saturating_sub(1),
                '{' => brace += 1,
                '}' => brace = brace.saturating_sub(1),
                '[' => bracket += 1,
                ']' => bracket = bracket.saturating_sub(1),
                '<' => angle += 1,
                '>' => angle = angle.saturating_sub(1),
                _ if ch == delimiter && paren == 0 && brace == 0 && bracket == 0 && angle == 0 => {
                    parts.push(&text[start..index]);
                    start = index + ch.len_utf8();
                }
                _ => {}
            }
        }

        parts.push(&text[start..]);
        parts
    }

    fn is_literal_text(&self, text: &str) -> bool {
        let trimmed = text.trim();
        (trimmed.starts_with('"') && trimmed.ends_with('"'))
            || (trimmed.starts_with('\'') && trimmed.ends_with('\''))
            || trimmed == "true"
            || trimmed == "false"
            || trimmed == "nil"
            || trimmed
                .chars()
                .all(|ch| ch.is_ascii_digit() || matches!(ch, '.' | '-' | '_'))
    }

    fn declare_param_types(&mut self, params: &[Param]) {
        for param in params {
            if let Param::Binding(binding) = param {
                self.declare_pattern_types(&binding.pattern, &binding.type_annotation);
            }
        }
    }

    fn declare_pattern_types(&mut self, pattern: &Pattern, annotation: &Option<String>) {
        let Some(type_name) = annotation
            .as_ref()
            .and_then(|annotation| self.simple_type_name(annotation))
        else {
            return;
        };

        for name in self.pattern_names(pattern) {
            if let Some(scope) = self.type_scopes.last_mut() {
                scope.insert(name, type_name.clone());
            }
        }
    }

    fn simple_type_name(&self, annotation: &str) -> Option<String> {
        let trimmed = annotation.trim();
        if trimmed.is_empty() {
            return None;
        }
        let name = trimmed
            .split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'))
            .next()?;
        if name.is_empty() {
            None
        } else {
            Some(name.to_string())
        }
    }

    fn infer_expr_type(&self, expr: &Expr) -> Option<&TypeShape> {
        let owned_name = match expr {
            Expr::Name(name) => name.clone(),
            Expr::TypeAssertion { annotation, .. } => self.simple_type_name(annotation)?,
            _ => return None,
        };

        if let Some(type_name) = self.lookup_type_name(&owned_name) {
            return self.type_defs.get(type_name);
        }
        self.type_defs.get(&owned_name)
    }

    fn lookup_type_name(&self, name: &str) -> Option<&str> {
        for scope in self.type_scopes.iter().rev() {
            if let Some(type_name) = scope.get(name) {
                return Some(type_name);
            }
        }
        None
    }

    fn check_switch_exhaustiveness(&mut self, switch_stmt: &SwitchStmt) {
        if switch_stmt.default.is_some() {
            return;
        }
        let Some(type_shape) = self.infer_expr_type(&switch_stmt.value).cloned() else {
            return;
        };

        let expected = match type_shape {
            TypeShape::LiteralUnion(values) => values,
            TypeShape::Enum { values, .. } => values,
            TypeShape::DiscriminatedUnion { .. } => return,
        };

        let mut seen = HashSet::new();
        for case in &switch_stmt.cases {
            if let Some(value) = self.expr_key(&case.value) {
                seen.insert(value);
            }
        }

        let missing = expected
            .into_iter()
            .filter(|value| !seen.contains(value))
            .collect::<Vec<_>>();
        if !missing.is_empty() {
            self.errors.push(format!(
                "non-exhaustive switch is missing: {}",
                missing.join(", ")
            ));
        }
    }

    fn check_match_exhaustiveness(&mut self, match_stmt: &MatchStmt) {
        let Some(TypeShape::DiscriminatedUnion { field, variants }) =
            self.infer_expr_type(&match_stmt.value).cloned()
        else {
            return;
        };

        let mut seen = HashSet::new();
        for case in &match_stmt.cases {
            if case.guard.is_some() {
                continue;
            }
            if let Some(value) = self.match_case_variant(&case.pattern, &field) {
                seen.insert(value);
            }
        }

        let missing = variants
            .into_iter()
            .filter(|value| !seen.contains(value))
            .collect::<Vec<_>>();
        if !missing.is_empty() {
            self.errors.push(format!(
                "non-exhaustive match is missing variants for `{}`: {}",
                field,
                missing.join(", ")
            ));
        }
    }

    fn match_case_variant(&self, pattern: &MatchPattern, field: &str) -> Option<String> {
        let MatchPattern::Table(fields) = pattern else {
            return None;
        };
        let entry = fields.iter().find(|entry| entry.key == field)?;
        match &entry.pattern {
            MatchPattern::Literal(expr) => self.expr_key(expr),
            _ => None,
        }
    }

    fn expr_key(&self, expr: &Expr) -> Option<String> {
        if let Some(literal) = self.literal_key(expr) {
            return Some(literal);
        }

        if let Expr::Chain { base, segments } = expr
            && let Expr::Name(root) = &**base
            && segments.len() == 1
            && let ChainSegment::Field { name, safe: false } = &segments[0]
            && let Some(TypeShape::Enum { members, .. }) = self.type_defs.get(root)
        {
            return members.get(name).cloned();
        }

        None
    }

    fn literal_key(&self, expr: &Expr) -> Option<String> {
        match expr {
            Expr::String(value) | Expr::Number(value) => Some(value.clone()),
            Expr::Bool(value) => Some(if *value {
                "true".to_string()
            } else {
                "false".to_string()
            }),
            Expr::Nil => Some("nil".to_string()),
            Expr::Paren(inner) => self.literal_key(inner),
            _ => None,
        }
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
            Expr::DoExpr { result, .. } => self.contains_placeholder(result),
            Expr::SwitchExpr {
                value,
                cases,
                default,
            } => {
                self.contains_placeholder(value)
                    || cases.iter().any(|case| {
                        self.contains_placeholder(&case.value)
                            || self.contains_placeholder(&case.result)
                    })
                    || self.contains_placeholder(default)
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
            Expr::Comprehension(comprehension) => match &comprehension.kind {
                TableComprehensionKind::Array { value } => {
                    self.contains_placeholder(value)
                        || comprehension.clauses.iter().any(|clause| match clause {
                            ComprehensionClause::GenericFor { iterables, .. } => iterables
                                .iter()
                                .any(|iterable| self.contains_placeholder(iterable)),
                            ComprehensionClause::NumericFor {
                                start, end, step, ..
                            } => {
                                self.contains_placeholder(start)
                                    || self.contains_placeholder(end)
                                    || step
                                        .as_ref()
                                        .map(|step| self.contains_placeholder(step))
                                        .unwrap_or(false)
                            }
                            ComprehensionClause::Filter(expr) => self.contains_placeholder(expr),
                        })
                }
                TableComprehensionKind::Map { key, value } => {
                    self.contains_placeholder(key)
                        || self.contains_placeholder(value)
                        || comprehension.clauses.iter().any(|clause| match clause {
                            ComprehensionClause::GenericFor { iterables, .. } => iterables
                                .iter()
                                .any(|iterable| self.contains_placeholder(iterable)),
                            ComprehensionClause::NumericFor {
                                start, end, step, ..
                            } => {
                                self.contains_placeholder(start)
                                    || self.contains_placeholder(end)
                                    || step
                                        .as_ref()
                                        .map(|step| self.contains_placeholder(step))
                                        .unwrap_or(false)
                            }
                            ComprehensionClause::Filter(expr) => self.contains_placeholder(expr),
                        })
                }
            },
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

    fn match_pattern_bindings(&self, pattern: &MatchPattern) -> Vec<String> {
        let mut names = Vec::new();
        match pattern {
            MatchPattern::Literal(_) => {}
            MatchPattern::Bind(name) => names.push(name.clone()),
            MatchPattern::Table(fields) => {
                for field in fields {
                    names.extend(self.match_pattern_bindings(&field.pattern));
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
