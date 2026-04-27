use crate::{
    ast::*,
    compiler::{CompilerError, Result},
    lexer::{Keyword, Span, Symbol, Token, TokenKind},
};

pub struct Parser<'src> {
    source: &'src str,
    tokens: Vec<Token>,
    index: usize,
}

impl<'src> Parser<'src> {
    pub fn new(source: &'src str, tokens: Vec<Token>) -> Self {
        Self {
            source,
            tokens,
            index: 0,
        }
    }

    pub fn parse_program(&mut self) -> Result<Program> {
        Ok(Program {
            block: self.parse_block(&[])?,
        })
    }

    fn parse_block(&mut self, terminators: &[Keyword]) -> Result<Block> {
        let mut statements = Vec::new();
        while !self.is_eof() && !self.check_any_keyword(terminators) {
            if self.match_symbol(Symbol::Semi) {
                continue;
            }
            statements.push(self.parse_stmt()?);
        }
        Ok(statements)
    }

    fn parse_stmt(&mut self) -> Result<Stmt> {
        if self.match_keyword(Keyword::Local) {
            if self.match_keyword(Keyword::Function) {
                return self.parse_function_stmt(true);
            }
            return self.parse_local_decl(false);
        }
        if self.match_keyword(Keyword::Const) {
            return self.parse_local_decl(true);
        }
        if self.match_keyword(Keyword::Function) {
            return self.parse_function_stmt(false);
        }
        if self.match_keyword(Keyword::If) {
            return self.parse_if_stmt();
        }
        if self.match_keyword(Keyword::While) {
            return self.parse_while_stmt();
        }
        if self.match_keyword(Keyword::Repeat) {
            return self.parse_repeat_stmt();
        }
        if self.match_keyword(Keyword::For) {
            return self.parse_for_stmt();
        }
        if self.match_keyword(Keyword::Do) {
            let block = self.parse_block(&[Keyword::End])?;
            self.expect_keyword(Keyword::End)?;
            return Ok(Stmt::Do(block));
        }
        if self.match_keyword(Keyword::Return) {
            let values = if self.check_block_end() {
                Vec::new()
            } else {
                self.parse_expr_list()?
            };
            return Ok(Stmt::Return(values));
        }
        if self.match_keyword(Keyword::Break) {
            return Ok(Stmt::Break);
        }
        if self.match_keyword(Keyword::Continue) {
            return Ok(Stmt::Continue);
        }
        if self.check_keyword(Keyword::Type)
            || (self.check_keyword(Keyword::Export) && self.check_keyword_at(1, Keyword::Type))
        {
            return self.parse_type_alias_stmt();
        }

        self.parse_assignment_or_call_stmt()
    }

    fn parse_type_alias_stmt(&mut self) -> Result<Stmt> {
        let start = self.current().span.start;
        let start_line = self.current().span.line;
        let mut depth = 0usize;
        let mut seen_assign = false;

        while !self.is_eof() {
            let token = self.current().clone();
            match token.kind {
                TokenKind::Symbol(Symbol::LParen)
                | TokenKind::Symbol(Symbol::LBrace)
                | TokenKind::Symbol(Symbol::LBracket)
                | TokenKind::Symbol(Symbol::Less) => depth += 1,
                TokenKind::Symbol(Symbol::RParen)
                | TokenKind::Symbol(Symbol::RBrace)
                | TokenKind::Symbol(Symbol::RBracket)
                | TokenKind::Symbol(Symbol::Greater) => depth = depth.saturating_sub(1),
                TokenKind::Symbol(Symbol::Assign) => seen_assign = true,
                _ => {}
            }

            self.bump();

            if seen_assign && depth == 0 {
                let next = self.current();
                if next.span.line > start_line && self.looks_like_statement_start() {
                    break;
                }
            }
        }

        let end = self.previous().span.end;
        Ok(Stmt::TypeAlias {
            raw: self.source[start..end].trim().to_string(),
        })
    }

    fn parse_local_decl(&mut self, is_const: bool) -> Result<Stmt> {
        let mut bindings = Vec::new();
        loop {
            let pattern = self.parse_pattern()?;
            let type_annotation = if self.match_symbol(Symbol::Colon) {
                Some(self.collect_type_annotation(&[
                    StopToken::Symbol(Symbol::Comma),
                    StopToken::Symbol(Symbol::Assign),
                    StopToken::Keyword(Keyword::In),
                    StopToken::Symbol(Symbol::RParen),
                ])?)
            } else {
                None
            };
            bindings.push(Binding {
                pattern,
                type_annotation,
            });
            if !self.match_symbol(Symbol::Comma) {
                break;
            }
        }

        let values = if self.match_symbol(Symbol::Assign) {
            self.parse_expr_list()?
        } else {
            Vec::new()
        };

        Ok(Stmt::Local(LocalDecl {
            is_const,
            bindings,
            values,
        }))
    }

    fn parse_function_stmt(&mut self, local_name: bool) -> Result<Stmt> {
        let name = if local_name {
            let root = self.expect_identifier()?;
            FunctionName {
                root,
                fields: Vec::new(),
                method: None,
            }
        } else {
            self.parse_function_name()?
        };

        let generics = if self.check_symbol(Symbol::Less) {
            Some(self.collect_balanced(Symbol::Less, Symbol::Greater)?)
        } else {
            None
        };
        let (params, return_type, body) = self.parse_function_body()?;

        Ok(Stmt::Function(FunctionDecl {
            local_name,
            name,
            generics,
            params,
            return_type,
            body,
        }))
    }

    fn parse_function_name(&mut self) -> Result<FunctionName> {
        let root = self.expect_identifier()?;
        let mut fields = Vec::new();
        let mut method = None;
        while self.match_symbol(Symbol::Dot) {
            fields.push(self.expect_identifier()?);
        }
        if self.match_symbol(Symbol::Colon) {
            method = Some(self.expect_identifier()?);
        }
        Ok(FunctionName {
            root,
            fields,
            method,
        })
    }

    fn parse_function_body(&mut self) -> Result<(Vec<Param>, Option<String>, Block)> {
        self.expect_symbol(Symbol::LParen)?;
        let params = self.parse_params()?;
        self.expect_symbol(Symbol::RParen)?;
        let return_type = if self.match_symbol(Symbol::Colon) {
            Some(self.collect_type_annotation(&[])?)
        } else {
            None
        };
        let body = self.parse_block(&[Keyword::End])?;
        self.expect_keyword(Keyword::End)?;
        Ok((params, return_type, body))
    }

    fn parse_params(&mut self) -> Result<Vec<Param>> {
        let mut params = Vec::new();
        if self.check_symbol(Symbol::RParen) {
            return Ok(params);
        }

        loop {
            if self.match_symbol(Symbol::Ellipsis) {
                let type_annotation = if self.match_symbol(Symbol::Colon) {
                    Some(self.collect_type_annotation(&[
                        StopToken::Symbol(Symbol::Comma),
                        StopToken::Symbol(Symbol::RParen),
                    ])?)
                } else {
                    None
                };
                params.push(Param::VarArg(type_annotation));
            } else {
                let pattern = self.parse_pattern()?;
                let type_annotation = if self.match_symbol(Symbol::Colon) {
                    Some(self.collect_type_annotation(&[
                        StopToken::Symbol(Symbol::Comma),
                        StopToken::Symbol(Symbol::RParen),
                    ])?)
                } else {
                    None
                };
                params.push(Param::Binding(Binding {
                    pattern,
                    type_annotation,
                }));
            }

            if !self.match_symbol(Symbol::Comma) {
                break;
            }
        }
        Ok(params)
    }

    fn parse_if_stmt(&mut self) -> Result<Stmt> {
        let condition = self.parse_expr()?;
        self.expect_keyword(Keyword::Then)?;
        let mut branches = vec![(
            condition,
            self.parse_block(&[Keyword::ElseIf, Keyword::Else, Keyword::End])?,
        )];
        while self.match_keyword(Keyword::ElseIf) {
            let condition = self.parse_expr()?;
            self.expect_keyword(Keyword::Then)?;
            let block = self.parse_block(&[Keyword::ElseIf, Keyword::Else, Keyword::End])?;
            branches.push((condition, block));
        }
        let else_block = if self.match_keyword(Keyword::Else) {
            Some(self.parse_block(&[Keyword::End])?)
        } else {
            None
        };
        self.expect_keyword(Keyword::End)?;
        Ok(Stmt::If(IfStmt {
            branches,
            else_block,
        }))
    }

    fn parse_while_stmt(&mut self) -> Result<Stmt> {
        let condition = self.parse_expr()?;
        self.expect_keyword(Keyword::Do)?;
        let block = self.parse_block(&[Keyword::End])?;
        self.expect_keyword(Keyword::End)?;
        Ok(Stmt::While { condition, block })
    }

    fn parse_repeat_stmt(&mut self) -> Result<Stmt> {
        let block = self.parse_block(&[Keyword::Until])?;
        self.expect_keyword(Keyword::Until)?;
        let condition = self.parse_expr()?;
        Ok(Stmt::Repeat { block, condition })
    }

    fn parse_for_stmt(&mut self) -> Result<Stmt> {
        if self.check(TokenKind::Identifier) && self.check_symbol_at(1, Symbol::Assign) {
            let name = self.expect_identifier()?;
            self.expect_symbol(Symbol::Assign)?;
            let start = self.parse_expr()?;
            self.expect_symbol(Symbol::Comma)?;
            let end = self.parse_expr()?;
            let step = if self.match_symbol(Symbol::Comma) {
                Some(self.parse_expr()?)
            } else {
                None
            };
            self.expect_keyword(Keyword::Do)?;
            let block = self.parse_block(&[Keyword::End])?;
            self.expect_keyword(Keyword::End)?;
            return Ok(Stmt::ForNumeric(ForNumeric {
                name,
                start,
                end,
                step,
                block,
            }));
        }

        let mut bindings = Vec::new();
        loop {
            let pattern = self.parse_pattern()?;
            let type_annotation = if self.match_symbol(Symbol::Colon) {
                Some(self.collect_type_annotation(&[
                    StopToken::Symbol(Symbol::Comma),
                    StopToken::Keyword(Keyword::In),
                ])?)
            } else {
                None
            };
            bindings.push(Binding {
                pattern,
                type_annotation,
            });
            if !self.match_symbol(Symbol::Comma) {
                break;
            }
        }
        self.expect_keyword(Keyword::In)?;
        let iterables = self.parse_expr_list()?;
        self.expect_keyword(Keyword::Do)?;
        let block = self.parse_block(&[Keyword::End])?;
        self.expect_keyword(Keyword::End)?;
        Ok(Stmt::ForGeneric(ForGeneric {
            bindings,
            iterables,
            block,
        }))
    }

    fn parse_assignment_or_call_stmt(&mut self) -> Result<Stmt> {
        let expr = self.parse_prefix_chain()?;
        if let Some(op) = self.current_compound_op() {
            self.bump();
            let value = self.parse_expr()?;
            return Ok(Stmt::CompoundAssignment {
                target: self.into_assign_target(expr)?,
                op,
                value,
            });
        }
        if self.match_symbol(Symbol::DoubleQuestionEqual) {
            let value = self.parse_expr()?;
            return Ok(Stmt::NullishAssignment {
                target: self.into_assign_target(expr)?,
                value,
            });
        }
        if self.check_symbol(Symbol::Assign) || self.check_symbol(Symbol::Comma) {
            let mut targets = vec![self.into_assign_target(expr)?];
            while self.match_symbol(Symbol::Comma) {
                let expr = self.parse_prefix_chain()?;
                targets.push(self.into_assign_target(expr)?);
            }
            self.expect_symbol(Symbol::Assign)?;
            let values = self.parse_expr_list()?;
            return Ok(Stmt::Assignment(Assignment { targets, values }));
        }
        Ok(Stmt::Call(expr))
    }

    fn parse_pattern(&mut self) -> Result<Pattern> {
        if self.match_symbol(Symbol::LBrace) {
            return self.parse_table_pattern();
        }
        if self.match_symbol(Symbol::LBracket) {
            return self.parse_array_pattern();
        }
        Ok(Pattern::Name(self.expect_identifier()?))
    }

    fn parse_table_pattern(&mut self) -> Result<Pattern> {
        let mut entries = Vec::new();
        let mut rest = None;
        if !self.check_symbol(Symbol::RBrace) {
            loop {
                if self.match_symbol(Symbol::Ellipsis) {
                    rest = Some(self.expect_identifier()?);
                    break;
                }
                let key = self.expect_identifier()?;
                let target = if self.match_symbol(Symbol::Colon) {
                    self.parse_pattern()?
                } else {
                    Pattern::Name(key.clone())
                };
                let default_value = if self.match_symbol(Symbol::Assign) {
                    Some(self.parse_expr()?)
                } else {
                    None
                };
                entries.push(TablePatternEntry {
                    key,
                    binding: PatternBinding {
                        target,
                        default_value,
                    },
                });
                if !self.match_symbol(Symbol::Comma) {
                    break;
                }
            }
        }
        self.expect_symbol(Symbol::RBrace)?;
        Ok(Pattern::Table { entries, rest })
    }

    fn parse_array_pattern(&mut self) -> Result<Pattern> {
        let mut items = Vec::new();
        let mut rest = None;
        if !self.check_symbol(Symbol::RBracket) {
            loop {
                if self.match_symbol(Symbol::Ellipsis) {
                    rest = Some(self.expect_identifier()?);
                    break;
                }
                let binding = if self.check_identifier_named("_") {
                    self.bump();
                    None
                } else {
                    let target = self.parse_pattern()?;
                    let default_value = if self.match_symbol(Symbol::Assign) {
                        Some(self.parse_expr()?)
                    } else {
                        None
                    };
                    Some(PatternBinding {
                        target,
                        default_value,
                    })
                };
                items.push(ArrayPatternItem { binding });
                if !self.match_symbol(Symbol::Comma) {
                    break;
                }
            }
        }
        self.expect_symbol(Symbol::RBracket)?;
        Ok(Pattern::Array { items, rest })
    }

    fn parse_expr_list(&mut self) -> Result<Vec<Expr>> {
        let mut values = vec![self.parse_expr()?];
        while self.match_symbol(Symbol::Comma) {
            values.push(self.parse_expr()?);
        }
        Ok(values)
    }

    fn parse_expr(&mut self) -> Result<Expr> {
        self.parse_expr_bp(0)
    }

    fn parse_expr_bp(&mut self, min_bp: u8) -> Result<Expr> {
        let mut left = self.parse_prefix_expr()?;

        if self.match_symbol(Symbol::DoubleColon) {
            let annotation = self.collect_type_annotation(&[
                StopToken::Symbol(Symbol::Comma),
                StopToken::Symbol(Symbol::RParen),
                StopToken::Symbol(Symbol::RBracket),
                StopToken::Symbol(Symbol::RBrace),
                StopToken::Keyword(Keyword::Then),
                StopToken::Keyword(Keyword::Do),
                StopToken::Keyword(Keyword::Else),
                StopToken::Keyword(Keyword::ElseIf),
                StopToken::Keyword(Keyword::End),
                StopToken::Keyword(Keyword::Until),
            ])?;
            left = Expr::TypeAssertion {
                expr: Box::new(left),
                annotation,
            };
        }

        loop {
            if self.match_symbol(Symbol::Question) {
                if min_bp > 1 {
                    self.rewind_one();
                    break;
                }
                let then_expr = self.parse_expr_bp(0)?;
                self.expect_symbol(Symbol::Colon)?;
                let else_expr = self.parse_expr_bp(1)?;
                left = Expr::Ternary {
                    condition: Box::new(left),
                    then_expr: Box::new(then_expr),
                    else_expr: Box::new(else_expr),
                };
                continue;
            }

            if self.match_symbol(Symbol::PipeGreater) {
                let (lbp, rbp) = (2, 3);
                if lbp < min_bp {
                    self.rewind_one();
                    break;
                }
                let stage = self.parse_pipe_stage()?;
                match left {
                    Expr::Pipe {
                        left: base,
                        mut stages,
                    } => {
                        stages.push(stage);
                        left = Expr::Pipe { left: base, stages };
                    }
                    _ => {
                        left = Expr::Pipe {
                            left: Box::new(left),
                            stages: vec![stage],
                        };
                    }
                }
                if rbp < min_bp {
                    break;
                }
                continue;
            }

            let Some((op, lbp, rbp)) = self.current_binary_op() else {
                break;
            };
            if lbp < min_bp {
                break;
            }
            self.bump();
            let right = self.parse_expr_bp(rbp)?;
            left = Expr::Binary {
                left: Box::new(left),
                op,
                right: Box::new(right),
            };
        }

        Ok(left)
    }

    fn parse_prefix_expr(&mut self) -> Result<Expr> {
        if self.match_keyword(Keyword::Nil) {
            return Ok(Expr::Nil);
        }
        if self.match_keyword(Keyword::True) {
            return Ok(Expr::Bool(true));
        }
        if self.match_keyword(Keyword::False) {
            return Ok(Expr::Bool(false));
        }
        if self.match_keyword(Keyword::If) {
            return self.parse_if_expr();
        }
        if self.match_keyword(Keyword::Function) {
            return self.parse_function_expr();
        }
        if self.match_keyword(Keyword::Not) {
            return Ok(Expr::Unary {
                op: UnaryOp::Not,
                expr: Box::new(self.parse_expr_bp(10)?),
            });
        }
        if self.match_symbol(Symbol::Minus) {
            return Ok(Expr::Unary {
                op: UnaryOp::Negate,
                expr: Box::new(self.parse_expr_bp(10)?),
            });
        }
        if self.match_symbol(Symbol::Hash) {
            return Ok(Expr::Unary {
                op: UnaryOp::Length,
                expr: Box::new(self.parse_expr_bp(10)?),
            });
        }
        if self.match_symbol(Symbol::LBrace) {
            return self.parse_table_expr();
        }
        if self.match_symbol(Symbol::LParen) {
            let expr = self.parse_expr()?;
            self.expect_symbol(Symbol::RParen)?;
            return Ok(self.parse_postfix(Expr::Paren(Box::new(expr)))?);
        }
        if self.match_symbol(Symbol::Ellipsis) {
            return Ok(Expr::VarArg);
        }
        if self.check(TokenKind::String) {
            let text = self.bump().lexeme;
            return Ok(Expr::String(text));
        }
        if self.check(TokenKind::Number) {
            let text = self.bump().lexeme;
            return Ok(Expr::Number(text));
        }
        if self.check(TokenKind::Identifier) {
            let name = self.bump().lexeme;
            return self.parse_postfix(Expr::Name(name));
        }

        Err(self.error_here("expected expression"))
    }

    fn parse_if_expr(&mut self) -> Result<Expr> {
        let condition = self.parse_expr()?;
        self.expect_keyword(Keyword::Then)?;
        let then_expr = self.parse_expr()?;
        let mut branches = vec![(condition, then_expr)];
        while self.match_keyword(Keyword::ElseIf) {
            let condition = self.parse_expr()?;
            self.expect_keyword(Keyword::Then)?;
            let value = self.parse_expr()?;
            branches.push((condition, value));
        }
        self.expect_keyword(Keyword::Else)?;
        let else_expr = self.parse_expr()?;
        Ok(Expr::IfElse {
            branches,
            else_expr: Box::new(else_expr),
        })
    }

    fn parse_function_expr(&mut self) -> Result<Expr> {
        let generics = if self.check_symbol(Symbol::Less) {
            Some(self.collect_balanced(Symbol::Less, Symbol::Greater)?)
        } else {
            None
        };
        let (params, return_type, body) = self.parse_function_body()?;
        Ok(Expr::Function(FunctionExpr {
            generics,
            params,
            return_type,
            body,
        }))
    }

    fn parse_table_expr(&mut self) -> Result<Expr> {
        let mut fields = Vec::new();
        if !self.check_symbol(Symbol::RBrace) {
            loop {
                if self.match_symbol(Symbol::LBracket) {
                    let key = self.parse_expr()?;
                    self.expect_symbol(Symbol::RBracket)?;
                    self.expect_symbol(Symbol::Assign)?;
                    let value = self.parse_expr()?;
                    fields.push(TableField::Indexed(key, value));
                } else if self.check(TokenKind::Identifier)
                    && self.check_symbol_at(1, Symbol::Assign)
                {
                    let name = self.bump().lexeme;
                    self.expect_symbol(Symbol::Assign)?;
                    let value = self.parse_expr()?;
                    fields.push(TableField::Named(name, value));
                } else {
                    fields.push(TableField::Value(self.parse_expr()?));
                }

                if !self.match_symbol(Symbol::Comma) && !self.match_symbol(Symbol::Semi) {
                    break;
                }
            }
        }
        self.expect_symbol(Symbol::RBrace)?;
        Ok(Expr::Table(fields))
    }

    fn parse_prefix_chain(&mut self) -> Result<Expr> {
        let base = if self.match_symbol(Symbol::LParen) {
            let expr = self.parse_expr()?;
            self.expect_symbol(Symbol::RParen)?;
            Expr::Paren(Box::new(expr))
        } else if self.check(TokenKind::Identifier) {
            Expr::Name(self.bump().lexeme)
        } else {
            return Err(self.error_here("expected assignable expression or call"));
        };
        self.parse_postfix(base)
    }

    fn parse_postfix(&mut self, base: Expr) -> Result<Expr> {
        let mut expr = base;
        loop {
            if self.match_symbol(Symbol::Dot) {
                let name = self.expect_identifier()?;
                expr = self.push_segment(expr, ChainSegment::Field { name, safe: false });
                continue;
            }
            if self.match_symbol(Symbol::LBracket) {
                let index = self.parse_expr()?;
                self.expect_symbol(Symbol::RBracket)?;
                expr = self.push_segment(
                    expr,
                    ChainSegment::Index {
                        expr: Box::new(index),
                        safe: false,
                    },
                );
                continue;
            }
            if self.match_symbol(Symbol::Colon) {
                let name = self.expect_identifier()?;
                let args = self.parse_args()?;
                expr = self.push_segment(
                    expr,
                    ChainSegment::MethodCall {
                        name,
                        args,
                        safe: false,
                    },
                );
                continue;
            }
            if self.check_call_start() {
                let args = self.parse_args()?;
                expr = self.push_segment(expr, ChainSegment::Call { args });
                continue;
            }
            if self.check_symbol(Symbol::Question)
                && self.check_symbol_at(1, Symbol::Dot)
                && self.check_symbol_at(2, Symbol::LBracket)
            {
                self.bump();
                self.bump();
                self.bump();
                let index = self.parse_expr()?;
                self.expect_symbol(Symbol::RBracket)?;
                expr = self.push_segment(
                    expr,
                    ChainSegment::Index {
                        expr: Box::new(index),
                        safe: true,
                    },
                );
                continue;
            }
            if self.check_symbol(Symbol::Question) && self.check_symbol_at(1, Symbol::Dot) {
                self.bump();
                self.bump();
                let name = self.expect_identifier()?;
                if self.check_call_start() {
                    let args = self.parse_args()?;
                    expr = self.push_segment(
                        expr,
                        ChainSegment::MethodCall {
                            name,
                            args,
                            safe: true,
                        },
                    );
                } else {
                    expr = self.push_segment(expr, ChainSegment::Field { name, safe: true });
                }
                continue;
            }
            break;
        }
        Ok(expr)
    }

    fn parse_pipe_stage(&mut self) -> Result<PipeStage> {
        if self.match_symbol(Symbol::Colon) {
            let name = self.expect_identifier()?;
            let args = self.parse_args()?;
            return Ok(PipeStage::Method { name, args });
        }

        let callee = self.parse_pipe_callee()?;
        if self.check_call_start() {
            let args = self.parse_args()?;
            Ok(PipeStage::Call {
                callee: Box::new(callee),
                args,
            })
        } else {
            Ok(PipeStage::Expr {
                callee: Box::new(callee),
            })
        }
    }

    fn parse_pipe_callee(&mut self) -> Result<Expr> {
        let mut expr = if self.match_symbol(Symbol::LParen) {
            let inner = self.parse_expr()?;
            self.expect_symbol(Symbol::RParen)?;
            Expr::Paren(Box::new(inner))
        } else if self.check(TokenKind::Identifier) {
            Expr::Name(self.bump().lexeme)
        } else {
            return Err(self.error_here("expected pipe stage"));
        };

        loop {
            if self.match_symbol(Symbol::Dot) {
                let name = self.expect_identifier()?;
                expr = self.push_segment(expr, ChainSegment::Field { name, safe: false });
                continue;
            }
            if self.match_symbol(Symbol::LBracket) {
                let index = self.parse_expr()?;
                self.expect_symbol(Symbol::RBracket)?;
                expr = self.push_segment(
                    expr,
                    ChainSegment::Index {
                        expr: Box::new(index),
                        safe: false,
                    },
                );
                continue;
            }
            break;
        }

        Ok(expr)
    }

    fn parse_args(&mut self) -> Result<Vec<Expr>> {
        if self.match_symbol(Symbol::LParen) {
            if self.match_symbol(Symbol::RParen) {
                return Ok(Vec::new());
            }
            let args = self.parse_expr_list()?;
            self.expect_symbol(Symbol::RParen)?;
            return Ok(args);
        }
        if self.check(TokenKind::String) {
            let text = self.bump().lexeme;
            return Ok(vec![Expr::String(text)]);
        }
        if self.match_symbol(Symbol::LBrace) {
            let table = self.parse_table_expr()?;
            return Ok(vec![table]);
        }
        Err(self.error_here("expected function arguments"))
    }

    fn into_assign_target(&self, expr: Expr) -> Result<AssignTarget> {
        match expr {
            Expr::Name(name) => Ok(AssignTarget::Name(name)),
            Expr::Chain { base, segments } => {
                if let Some(last) = segments.last() {
                    if segments.iter().any(|segment| {
                        matches!(
                            segment,
                            ChainSegment::Call { .. } | ChainSegment::MethodCall { .. }
                        )
                    }) {
                        return Err(CompilerError::Parse {
                            message: "call expressions cannot appear on the left-hand side of an assignment".to_string(),
                        });
                    }
                    let object = if segments.len() == 1 {
                        *base
                    } else {
                        Expr::Chain {
                            base,
                            segments: segments[..segments.len() - 1].to_vec(),
                        }
                    };
                    return match last {
                        ChainSegment::Field { name, safe: false } => Ok(AssignTarget::Field {
                            object: Box::new(object),
                            field: name.clone(),
                        }),
                        ChainSegment::Index { expr, safe: false } => Ok(AssignTarget::Index {
                            object: Box::new(object),
                            index: expr.clone(),
                        }),
                        _ => Err(CompilerError::Parse {
                            message: "optional chaining cannot appear on the left-hand side of an assignment".to_string(),
                        }),
                    };
                }
                Err(CompilerError::Parse {
                    message: "invalid assignment target".to_string(),
                })
            }
            Expr::Paren(expr) => self.into_assign_target(*expr),
            _ => Err(CompilerError::Parse {
                message: "invalid assignment target".to_string(),
            }),
        }
    }

    fn push_segment(&self, expr: Expr, segment: ChainSegment) -> Expr {
        match expr {
            Expr::Chain { base, mut segments } => {
                segments.push(segment);
                Expr::Chain { base, segments }
            }
            other => Expr::Chain {
                base: Box::new(other),
                segments: vec![segment],
            },
        }
    }

    fn current_binary_op(&self) -> Option<(BinaryOp, u8, u8)> {
        match &self.current().kind {
            TokenKind::Keyword(Keyword::Or) => Some((BinaryOp::Or, 4, 5)),
            TokenKind::Keyword(Keyword::And) => Some((BinaryOp::And, 5, 6)),
            TokenKind::Symbol(Symbol::DoubleQuestion) => Some((BinaryOp::Nullish, 3, 4)),
            TokenKind::Symbol(Symbol::Less) => Some((BinaryOp::Less, 6, 7)),
            TokenKind::Symbol(Symbol::LessEqual) => Some((BinaryOp::LessEqual, 6, 7)),
            TokenKind::Symbol(Symbol::Greater) => Some((BinaryOp::Greater, 6, 7)),
            TokenKind::Symbol(Symbol::GreaterEqual) => Some((BinaryOp::GreaterEqual, 6, 7)),
            TokenKind::Symbol(Symbol::Equal) => Some((BinaryOp::Equal, 6, 7)),
            TokenKind::Symbol(Symbol::NotEqual) => Some((BinaryOp::NotEqual, 6, 7)),
            TokenKind::Symbol(Symbol::DoubleDot) => Some((BinaryOp::Concat, 7, 7)),
            TokenKind::Symbol(Symbol::Plus) => Some((BinaryOp::Add, 8, 9)),
            TokenKind::Symbol(Symbol::Minus) => Some((BinaryOp::Subtract, 8, 9)),
            TokenKind::Symbol(Symbol::Star) => Some((BinaryOp::Multiply, 9, 10)),
            TokenKind::Symbol(Symbol::Slash) => Some((BinaryOp::Divide, 9, 10)),
            TokenKind::Symbol(Symbol::DoubleSlash) => Some((BinaryOp::FloorDivide, 9, 10)),
            TokenKind::Symbol(Symbol::Percent) => Some((BinaryOp::Modulo, 9, 10)),
            TokenKind::Symbol(Symbol::Caret) => Some((BinaryOp::Power, 11, 11)),
            _ => None,
        }
    }

    fn current_compound_op(&self) -> Option<CompoundOp> {
        match self.current().kind {
            TokenKind::Symbol(Symbol::PlusEqual) => Some(CompoundOp::Add),
            TokenKind::Symbol(Symbol::MinusEqual) => Some(CompoundOp::Subtract),
            TokenKind::Symbol(Symbol::StarEqual) => Some(CompoundOp::Multiply),
            TokenKind::Symbol(Symbol::SlashEqual) => Some(CompoundOp::Divide),
            TokenKind::Symbol(Symbol::DoubleSlashEqual) => Some(CompoundOp::FloorDivide),
            TokenKind::Symbol(Symbol::PercentEqual) => Some(CompoundOp::Modulo),
            TokenKind::Symbol(Symbol::CaretEqual) => Some(CompoundOp::Power),
            TokenKind::Symbol(Symbol::DoubleDotEqual) => Some(CompoundOp::Concat),
            _ => None,
        }
    }

    fn collect_type_annotation(&mut self, stops: &[StopToken]) -> Result<String> {
        let start = self.current().span.start;
        let mut depth_paren = 0usize;
        let mut depth_brace = 0usize;
        let mut depth_bracket = 0usize;
        let mut depth_angle = 0usize;
        let start_line = self.current().span.line;

        while !self.is_eof() {
            if depth_paren == 0
                && depth_brace == 0
                && depth_bracket == 0
                && depth_angle == 0
                && (stops.iter().any(|stop| stop.matches(self.current()))
                    || (stops.is_empty()
                        && self.current().span.line > start_line
                        && self.looks_like_statement_start()))
            {
                break;
            }

            match self.current().kind {
                TokenKind::Symbol(Symbol::LParen) => depth_paren += 1,
                TokenKind::Symbol(Symbol::RParen) => {
                    if depth_paren == 0 && stops.iter().any(|s| s.matches(self.current())) {
                        break;
                    }
                    depth_paren = depth_paren.saturating_sub(1);
                }
                TokenKind::Symbol(Symbol::LBrace) => depth_brace += 1,
                TokenKind::Symbol(Symbol::RBrace) => depth_brace = depth_brace.saturating_sub(1),
                TokenKind::Symbol(Symbol::LBracket) => depth_bracket += 1,
                TokenKind::Symbol(Symbol::RBracket) => {
                    depth_bracket = depth_bracket.saturating_sub(1)
                }
                TokenKind::Symbol(Symbol::Less) => depth_angle += 1,
                TokenKind::Symbol(Symbol::Greater) => depth_angle = depth_angle.saturating_sub(1),
                _ => {}
            }
            self.bump();
        }

        let end = self.previous().span.end;
        Ok(self.source[start..end].trim().to_string())
    }

    fn collect_balanced(&mut self, open: Symbol, close: Symbol) -> Result<String> {
        let start = self.current().span.start;
        let mut depth = 0usize;
        loop {
            let token = self.current().clone();
            match token.kind {
                TokenKind::Symbol(symbol) if symbol == open => depth += 1,
                TokenKind::Symbol(symbol) if symbol == close => {
                    depth = depth.saturating_sub(1);
                    self.bump();
                    if depth == 0 {
                        let end = self.previous().span.end;
                        return Ok(self.source[start..end].trim().to_string());
                    }
                    continue;
                }
                TokenKind::Eof => {
                    return Err(CompilerError::Parse {
                        message: format!(
                            "unterminated balanced token sequence starting with {:?}",
                            open
                        ),
                    });
                }
                _ => {}
            }
            self.bump();
        }
    }

    fn looks_like_statement_start(&self) -> bool {
        matches!(
            self.current().kind,
            TokenKind::Keyword(Keyword::Local)
                | TokenKind::Keyword(Keyword::Const)
                | TokenKind::Keyword(Keyword::Function)
                | TokenKind::Keyword(Keyword::If)
                | TokenKind::Keyword(Keyword::For)
                | TokenKind::Keyword(Keyword::While)
                | TokenKind::Keyword(Keyword::Repeat)
                | TokenKind::Keyword(Keyword::Return)
                | TokenKind::Keyword(Keyword::Break)
                | TokenKind::Keyword(Keyword::Continue)
                | TokenKind::Keyword(Keyword::Do)
                | TokenKind::Keyword(Keyword::Type)
                | TokenKind::Keyword(Keyword::Export)
                | TokenKind::Keyword(Keyword::End)
                | TokenKind::Keyword(Keyword::Else)
                | TokenKind::Keyword(Keyword::ElseIf)
                | TokenKind::Keyword(Keyword::Until)
        )
    }

    fn check_call_start(&self) -> bool {
        self.check_symbol(Symbol::LParen)
            || self.check(TokenKind::String)
            || self.check_symbol(Symbol::LBrace)
    }

    fn check_block_end(&self) -> bool {
        self.check_any_keyword(&[Keyword::End, Keyword::Else, Keyword::ElseIf, Keyword::Until])
            || self.check(TokenKind::Eof)
    }

    fn expect_identifier(&mut self) -> Result<String> {
        if self.check(TokenKind::Identifier) {
            return Ok(self.bump().lexeme);
        }
        Err(self.error_here("expected identifier"))
    }

    fn expect_keyword(&mut self, keyword: Keyword) -> Result<()> {
        if self.match_keyword(keyword) {
            Ok(())
        } else {
            Err(self.error_here(&format!("expected keyword `{keyword}`")))
        }
    }

    fn expect_symbol(&mut self, symbol: Symbol) -> Result<()> {
        if self.match_symbol(symbol) {
            Ok(())
        } else {
            Err(self.error_here(&format!("expected symbol `{}`", symbol_text(symbol))))
        }
    }

    fn match_keyword(&mut self, keyword: Keyword) -> bool {
        if self.check_keyword(keyword) {
            self.bump();
            true
        } else {
            false
        }
    }

    fn match_symbol(&mut self, symbol: Symbol) -> bool {
        if self.check_symbol(symbol) {
            self.bump();
            true
        } else {
            false
        }
    }

    fn check_keyword(&self, keyword: Keyword) -> bool {
        matches!(self.current().kind, TokenKind::Keyword(k) if k == keyword)
    }

    fn check_any_keyword(&self, keywords: &[Keyword]) -> bool {
        keywords.iter().any(|keyword| self.check_keyword(*keyword))
    }

    fn check_keyword_at(&self, offset: usize, keyword: Keyword) -> bool {
        matches!(self.peek(offset).kind, TokenKind::Keyword(k) if k == keyword)
    }

    fn check_symbol(&self, symbol: Symbol) -> bool {
        matches!(self.current().kind, TokenKind::Symbol(s) if s == symbol)
    }

    fn check_symbol_at(&self, offset: usize, symbol: Symbol) -> bool {
        matches!(self.peek(offset).kind, TokenKind::Symbol(s) if s == symbol)
    }

    fn check_identifier_named(&self, name: &str) -> bool {
        self.check(TokenKind::Identifier) && self.current().lexeme == name
    }

    fn check(&self, kind: TokenKind) -> bool {
        self.current().kind == kind
    }

    fn current(&self) -> &Token {
        self.peek(0)
    }

    fn previous(&self) -> &Token {
        &self.tokens[self.index.saturating_sub(1)]
    }

    fn peek(&self, offset: usize) -> &Token {
        self.tokens
            .get(self.index + offset)
            .unwrap_or_else(|| self.tokens.last().expect("lexer always emits eof"))
    }

    fn bump(&mut self) -> Token {
        let token = self.current().clone();
        if !matches!(token.kind, TokenKind::Eof) {
            self.index += 1;
        }
        token
    }

    fn rewind_one(&mut self) {
        self.index = self.index.saturating_sub(1);
    }

    fn is_eof(&self) -> bool {
        matches!(self.current().kind, TokenKind::Eof)
    }

    fn error_here(&self, message: &str) -> CompilerError {
        let Span { line, column, .. } = self.current().span;
        CompilerError::Parse {
            message: format!("{message} at {line}:{column}"),
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum StopToken {
    Symbol(Symbol),
    Keyword(Keyword),
}

impl StopToken {
    fn matches(&self, token: &Token) -> bool {
        match (self, &token.kind) {
            (StopToken::Symbol(left), TokenKind::Symbol(right)) => left == right,
            (StopToken::Keyword(left), TokenKind::Keyword(right)) => left == right,
            _ => false,
        }
    }
}

fn symbol_text(symbol: Symbol) -> &'static str {
    match symbol {
        Symbol::LParen => "(",
        Symbol::RParen => ")",
        Symbol::LBrace => "{",
        Symbol::RBrace => "}",
        Symbol::LBracket => "[",
        Symbol::RBracket => "]",
        Symbol::Comma => ",",
        Symbol::Dot => ".",
        Symbol::Colon => ":",
        Symbol::DoubleColon => "::",
        Symbol::Semi => ";",
        Symbol::Assign => "=",
        Symbol::Equal => "==",
        Symbol::NotEqual => "~=",
        Symbol::Less => "<",
        Symbol::LessEqual => "<=",
        Symbol::Greater => ">",
        Symbol::GreaterEqual => ">=",
        Symbol::Plus => "+",
        Symbol::Minus => "-",
        Symbol::Star => "*",
        Symbol::Slash => "/",
        Symbol::DoubleSlash => "//",
        Symbol::Percent => "%",
        Symbol::Caret => "^",
        Symbol::Hash => "#",
        Symbol::DoubleDot => "..",
        Symbol::Ellipsis => "...",
        Symbol::Arrow => "->",
        Symbol::PipeGreater => "|>",
        Symbol::PlusEqual => "+=",
        Symbol::MinusEqual => "-=",
        Symbol::StarEqual => "*=",
        Symbol::SlashEqual => "/=",
        Symbol::DoubleSlashEqual => "//=",
        Symbol::PercentEqual => "%=",
        Symbol::CaretEqual => "^=",
        Symbol::DoubleDotEqual => "..=",
        Symbol::Question => "?",
        Symbol::DoubleQuestion => "??",
        Symbol::DoubleQuestionEqual => "??=",
    }
}
