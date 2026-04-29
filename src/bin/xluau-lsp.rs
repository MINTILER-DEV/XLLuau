use std::{
    collections::{BTreeMap, HashMap, HashSet},
    error::Error,
    fs,
    io::{self, BufRead, BufReader, Write},
    path::{Path, PathBuf},
};

use serde_json::{Value, json};
use url::Url;
use xluau::{
    Compiler,
    ast::{Binding, Pattern, Stmt},
    compiler::CompilerError,
    formatter::format_source,
    lexer::{Keyword, Lexer, Span, Symbol, Token, TokenKind},
    module::ModuleResolver,
    parser::Parser,
};

fn main() -> Result<(), Box<dyn Error>> {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut reader = BufReader::new(stdin.lock());
    let mut writer = stdout.lock();
    let mut server = Server::default();

    while let Some(message) = read_message(&mut reader)? {
        let should_exit = server.handle_message(message, &mut writer)?;
        if should_exit {
            break;
        }
    }

    Ok(())
}

#[derive(Default)]
struct Server {
    documents: HashMap<String, String>,
    shutdown_requested: bool,
}

impl Server {
    fn handle_message(
        &mut self,
        message: Value,
        writer: &mut impl Write,
    ) -> Result<bool, Box<dyn Error>> {
        let method = message
            .get("method")
            .and_then(Value::as_str)
            .map(str::to_string);
        let id = message.get("id").cloned();
        let params = message.get("params").cloned().unwrap_or(Value::Null);

        match method.as_deref() {
            Some("initialize") => {
                if let Some(id) = id {
                    write_message(
                        writer,
                        &json!({
                            "jsonrpc": "2.0",
                            "id": id,
                            "result": {
                                "capabilities": {
                                    "textDocumentSync": 1,
                                    "documentFormattingProvider": true,
                                    "documentSymbolProvider": true,
                                    "completionProvider": {
                                        "triggerCharacters": [".", "\"", "'", "/", "@"]
                                    },
                                    "hoverProvider": true,
                                    "definitionProvider": true,
                                    "renameProvider": {
                                        "prepareProvider": true
                                    },
                                    "codeActionProvider": {
                                        "codeActionKinds": ["quickfix", "refactor.rename"]
                                    }
                                },
                                "serverInfo": {
                                    "name": "xluau-lsp"
                                }
                            }
                        }),
                    )?;
                }
            }
            Some("shutdown") => {
                self.shutdown_requested = true;
                if let Some(id) = id {
                    write_message(
                        writer,
                        &json!({
                            "jsonrpc": "2.0",
                            "id": id,
                            "result": Value::Null
                        }),
                    )?;
                }
            }
            Some("exit") => return Ok(true),
            Some("initialized")
            | Some("workspace/didChangeConfiguration")
            | Some("workspace/didChangeWatchedFiles") => {}
            Some("textDocument/didOpen") => {
                let uri = params["textDocument"]["uri"].as_str().unwrap_or_default().to_string();
                let text = params["textDocument"]["text"].as_str().unwrap_or_default().to_string();
                self.documents.insert(uri.clone(), text.clone());
                self.publish_diagnostics(writer, &uri, &text)?;
            }
            Some("textDocument/didChange") => {
                let uri = params["textDocument"]["uri"].as_str().unwrap_or_default().to_string();
                let text = params["contentChanges"]
                    .as_array()
                    .and_then(|changes| changes.last())
                    .and_then(|change| change.get("text"))
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                self.documents.insert(uri.clone(), text.clone());
                self.publish_diagnostics(writer, &uri, &text)?;
            }
            Some("textDocument/didClose") => {
                let uri = params["textDocument"]["uri"].as_str().unwrap_or_default().to_string();
                self.documents.remove(&uri);
                write_message(
                    writer,
                    &json!({
                        "jsonrpc": "2.0",
                        "method": "textDocument/publishDiagnostics",
                        "params": { "uri": uri, "diagnostics": [] }
                    }),
                )?;
            }
            Some("textDocument/formatting") => {
                if let Some(id) = id {
                    let uri = params["textDocument"]["uri"].as_str().unwrap_or_default();
                    let text = self.document_text(uri)?;
                    let formatted = format_source(&text);
                    let edit = json!([{
                        "range": full_document_range(&text),
                        "newText": formatted
                    }]);
                    write_message(
                        writer,
                        &json!({
                            "jsonrpc": "2.0",
                            "id": id,
                            "result": edit
                        }),
                    )?;
                }
            }
            Some("textDocument/documentSymbol") => {
                if let Some(id) = id {
                    let uri = params["textDocument"]["uri"].as_str().unwrap_or_default();
                    let text = self.document_text(uri)?;
                    let path = uri_to_path(uri)?;
                    let index = build_document_index(path, text);
                    let symbols = index.declarations.iter().map(decl_to_symbol).collect::<Vec<_>>();
                    write_message(
                        writer,
                        &json!({
                            "jsonrpc": "2.0",
                            "id": id,
                            "result": symbols
                        }),
                    )?;
                }
            }
            Some("textDocument/completion") => {
                if let Some(id) = id {
                    let result = self.completion_response(&params)?;
                    write_message(
                        writer,
                        &json!({
                            "jsonrpc": "2.0",
                            "id": id,
                            "result": result
                        }),
                    )?;
                }
            }
            Some("textDocument/hover") => {
                if let Some(id) = id {
                    let result = self.hover_response(&params)?;
                    write_message(
                        writer,
                        &json!({
                            "jsonrpc": "2.0",
                            "id": id,
                            "result": result
                        }),
                    )?;
                }
            }
            Some("textDocument/definition") => {
                if let Some(id) = id {
                    let result = self.definition_response(&params)?;
                    write_message(
                        writer,
                        &json!({
                            "jsonrpc": "2.0",
                            "id": id,
                            "result": result
                        }),
                    )?;
                }
            }
            Some("textDocument/prepareRename") => {
                if let Some(id) = id {
                    let result = self.prepare_rename_response(&params)?;
                    write_message(
                        writer,
                        &json!({
                            "jsonrpc": "2.0",
                            "id": id,
                            "result": result
                        }),
                    )?;
                }
            }
            Some("textDocument/rename") => {
                if let Some(id) = id {
                    let result = self.rename_response(&params)?;
                    write_message(
                        writer,
                        &json!({
                            "jsonrpc": "2.0",
                            "id": id,
                            "result": result
                        }),
                    )?;
                }
            }
            Some("textDocument/codeAction") => {
                if let Some(id) = id {
                    let result = self.code_action_response(&params)?;
                    write_message(
                        writer,
                        &json!({
                            "jsonrpc": "2.0",
                            "id": id,
                            "result": result
                        }),
                    )?;
                }
            }
            Some(_) => {
                if let Some(id) = id {
                    write_message(
                        writer,
                        &json!({
                            "jsonrpc": "2.0",
                            "id": id,
                            "result": Value::Null
                        }),
                    )?;
                }
            }
            None => {}
        }

        Ok(false)
    }

    fn document_text(&self, uri: &str) -> Result<String, Box<dyn Error>> {
        if let Some(text) = self.documents.get(uri) {
            return Ok(text.clone());
        }
        let path = uri_to_path(uri)?;
        Ok(fs::read_to_string(path)?)
    }

    fn completion_response(&self, params: &Value) -> Result<Value, Box<dyn Error>> {
        let uri = params["textDocument"]["uri"].as_str().unwrap_or_default();
        let text = self.document_text(uri)?;
        let path = uri_to_path(uri)?;
        let index = build_document_index(path.clone(), text.clone());
        let position = &params["position"];
        let line = position["line"].as_u64().unwrap_or(0) as usize;
        let character = position["character"].as_u64().unwrap_or(0) as usize;
        let offset = position_to_offset(&text, line, character);
        let mut items = Vec::new();

        if let Some(receiver) = member_completion_receiver(&text, offset) {
            for item in member_completion_items(&index, &receiver) {
                items.push(item);
            }
        } else if let Some((token_index, token)) = token_at_offset(&index.tokens, offset) {
            if token.kind == TokenKind::String && is_require_string_token(&index.tokens, token_index)
            {
                let compiler = compiler_for_path(&path)?;
                for (alias, target) in &compiler.config.paths {
                    items.push(json!({
                        "label": alias_completion_label(alias),
                        "kind": 17,
                        "detail": format!("maps to {}", target),
                        "insertText": alias_completion_label(alias)
                    }));
                }
            }
        }

        if items.is_empty() {
            let mut seen = HashSet::<String>::new();
            for keyword in KEYWORD_COMPLETIONS {
                if seen.insert((*keyword).to_string()) {
                    items.push(keyword_completion_item(keyword));
                }
            }
            for decl in &index.declarations {
                if seen.insert(decl.name.clone()) {
                    items.push(json!({
                        "label": decl.name,
                        "kind": decl.kind,
                        "detail": decl.detail
                    }));
                }
            }
        }

        Ok(json!({
            "isIncomplete": false,
            "items": items
        }))
    }

    fn hover_response(&self, params: &Value) -> Result<Value, Box<dyn Error>> {
        let uri = params["textDocument"]["uri"].as_str().unwrap_or_default();
        let text = self.document_text(uri)?;
        let path = uri_to_path(uri)?;
        let index = build_document_index(path.clone(), text.clone());
        let position = &params["position"];
        let line = position["line"].as_u64().unwrap_or(0) as usize;
        let character = position["character"].as_u64().unwrap_or(0) as usize;
        let offset = position_to_offset(&text, line, character);

        if let Some((token_index, token)) = token_at_offset(&index.tokens, offset) {
            if token.kind == TokenKind::String && is_require_string_token(&index.tokens, token_index)
            {
                let specifier = decode_string_token(token);
                let compiler = compiler_for_path(&path)?;
                let resolver = ModuleResolver::new(compiler.root.clone(), compiler.config.clone());
                if let Some(resolved) = resolver.resolve_require_path(&path, &specifier)? {
                    let contents = format!(
                        "```xluau\nrequire(\"{}\")\n```\n\nResolves to `{}`\n\nEmits `{}`",
                        specifier,
                        display_path(&compiler.root, &resolved.source_path),
                        resolved.emitted_require
                    );
                    return Ok(json!({
                        "contents": {
                            "kind": "markdown",
                            "value": contents
                        },
                        "range": range_from_token(token)
                    }));
                }
            }

            if token.kind == TokenKind::Identifier {
                if let Some(decl) = find_declaration(&index, &token.lexeme) {
                    return Ok(json!({
                        "contents": {
                            "kind": "markdown",
                            "value": format!("```xluau\n{}\n```", decl.hover)
                        },
                        "range": range_from_span(decl.name_span, decl.name.len())
                    }));
                }
            }
        }

        Ok(Value::Null)
    }

    fn definition_response(&self, params: &Value) -> Result<Value, Box<dyn Error>> {
        let uri = params["textDocument"]["uri"].as_str().unwrap_or_default();
        let text = self.document_text(uri)?;
        let path = uri_to_path(uri)?;
        let index = build_document_index(path.clone(), text.clone());
        let position = &params["position"];
        let line = position["line"].as_u64().unwrap_or(0) as usize;
        let character = position["character"].as_u64().unwrap_or(0) as usize;
        let offset = position_to_offset(&text, line, character);

        if let Some((token_index, token)) = token_at_offset(&index.tokens, offset) {
            if token.kind == TokenKind::String && is_require_string_token(&index.tokens, token_index)
            {
                let specifier = decode_string_token(token);
                let compiler = compiler_for_path(&path)?;
                let resolver = ModuleResolver::new(compiler.root.clone(), compiler.config.clone());
                if let Some(resolved) = resolver.resolve_require_path(&path, &specifier)? {
                    return Ok(json!([{
                        "uri": path_to_uri(&resolved.source_path)?,
                        "range": {
                            "start": { "line": 0, "character": 0 },
                            "end": { "line": 0, "character": 0 }
                        }
                    }]));
                }
            }

            if token.kind == TokenKind::Identifier {
                if let Some(decl) = find_declaration(&index, &token.lexeme) {
                    return Ok(json!([{
                        "uri": path_to_uri(&decl.path)?,
                        "range": range_from_span(decl.name_span, decl.name.len())
                    }]));
                }
            }
        }

        Ok(Value::Null)
    }

    fn prepare_rename_response(&self, params: &Value) -> Result<Value, Box<dyn Error>> {
        let uri = params["textDocument"]["uri"].as_str().unwrap_or_default();
        let text = self.document_text(uri)?;
        let path = uri_to_path(uri)?;
        let index = build_document_index(path, text.clone());
        let position = &params["position"];
        let line = position["line"].as_u64().unwrap_or(0) as usize;
        let character = position["character"].as_u64().unwrap_or(0) as usize;
        let offset = position_to_offset(&text, line, character);

        if let Some((token_index, token)) = token_at_offset(&index.tokens, offset) {
            if token.kind == TokenKind::Identifier && find_declaration(&index, &token.lexeme).is_some()
            {
                return Ok(range_from_token(token));
            }
            if token.kind == TokenKind::String && is_require_string_token(&index.tokens, token_index)
            {
                return Ok(range_from_token(token));
            }
        }

        Ok(Value::Null)
    }

    fn rename_response(&self, params: &Value) -> Result<Value, Box<dyn Error>> {
        let uri = params["textDocument"]["uri"].as_str().unwrap_or_default();
        let text = self.document_text(uri)?;
        let path = uri_to_path(uri)?;
        let index = build_document_index(path.clone(), text.clone());
        let position = &params["position"];
        let line = position["line"].as_u64().unwrap_or(0) as usize;
        let character = position["character"].as_u64().unwrap_or(0) as usize;
        let new_name = params["newName"].as_str().unwrap_or_default();
        let offset = position_to_offset(&text, line, character);
        let mut changes = BTreeMap::<String, Vec<Value>>::new();

        if let Some((token_index, token)) = token_at_offset(&index.tokens, offset) {
            if token.kind == TokenKind::Identifier && find_declaration(&index, &token.lexeme).is_some()
            {
                let edits = index
                    .tokens
                    .iter()
                    .filter(|candidate| candidate.kind == TokenKind::Identifier && candidate.lexeme == token.lexeme)
                    .map(|candidate| json!({
                        "range": range_from_token(candidate),
                        "newText": new_name
                    }))
                    .collect::<Vec<_>>();
                changes.insert(uri.to_string(), edits);
                return Ok(json!({ "changes": changes }));
            }

            if token.kind == TokenKind::String && is_require_string_token(&index.tokens, token_index)
            {
                let old_specifier = decode_string_token(token);
                let compiler = compiler_for_path(&path)?;
                let documents = workspace_documents(&compiler, &self.documents)?;
                for document in documents {
                    let edits = document
                        .tokens
                        .iter()
                        .enumerate()
                        .filter(|(candidate_index, candidate)| {
                            candidate.kind == TokenKind::String
                                && is_require_string_token(&document.tokens, *candidate_index)
                                && decode_string_token(candidate) == old_specifier
                        })
                        .map(|(_, candidate)| {
                            let replacement = rewrap_string_literal(&candidate.lexeme, new_name);
                            json!({
                                "range": range_from_token(candidate),
                                "newText": replacement
                            })
                        })
                        .collect::<Vec<_>>();
                    if !edits.is_empty() {
                        changes.insert(path_to_uri(&document.path)?, edits);
                    }
                }
                return Ok(json!({ "changes": changes }));
            }
        }

        Ok(Value::Null)
    }

    fn code_action_response(&self, params: &Value) -> Result<Value, Box<dyn Error>> {
        let uri = params["textDocument"]["uri"].as_str().unwrap_or_default();
        let text = self.document_text(uri)?;
        let path = uri_to_path(uri)?;
        let index = build_document_index(path, text.clone());
        let line = params["range"]["start"]["line"].as_u64().unwrap_or(0) as usize;
        let diagnostics = params["context"]["diagnostics"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        let mut actions = Vec::new();

        for diagnostic in diagnostics {
            let message = diagnostic["message"].as_str().unwrap_or_default();
            if let Some(name) = extract_const_name(message) {
                if let Some(decl) = index
                    .declarations
                    .iter()
                    .find(|decl| decl.name == name && decl.is_const)
                    && let Some(keyword_span) = decl.keyword_span
                {
                    actions.push(json!({
                        "title": format!("Convert const `{}` to local", name),
                        "kind": "quickfix",
                        "diagnostics": [diagnostic.clone()],
                        "edit": {
                            "changes": {
                                uri: [{
                                    "range": range_from_span(keyword_span, "const".len()),
                                    "newText": "local"
                                }]
                            }
                        }
                    }));
                }
            }

            if message.contains("non-exhaustive switch")
                && let Some(edit) = switch_default_edit(&text, line)
            {
                actions.push(json!({
                    "title": "Add switch default branch",
                    "kind": "quickfix",
                    "diagnostics": [diagnostic.clone()],
                    "edit": { "changes": { uri: [edit] } }
                }));
            }

            if message.contains("non-exhaustive match")
                && let Some(edit) = match_fallback_edit(&text, line)
            {
                actions.push(json!({
                    "title": "Add match fallback branch",
                    "kind": "quickfix",
                    "diagnostics": [diagnostic.clone()],
                    "edit": { "changes": { uri: [edit] } }
                }));
            }
        }

        Ok(json!(actions))
    }

    fn publish_diagnostics(
        &self,
        writer: &mut impl Write,
        uri: &str,
        text: &str,
    ) -> Result<(), Box<dyn Error>> {
        let diagnostics = diagnostics_for(uri, text);
        write_message(
            writer,
            &json!({
                "jsonrpc": "2.0",
                "method": "textDocument/publishDiagnostics",
                "params": {
                    "uri": uri,
                    "diagnostics": diagnostics
                }
            }),
        )?;
        Ok(())
    }
}

const KIND_CLASS: u32 = 5;
const KIND_FUNCTION: u32 = 12;
const KIND_VARIABLE: u32 = 13;
const KIND_ENUM: u32 = 10;
const KIND_TYPE: u32 = 11;
const KIND_PROPERTY: u32 = 7;
const KIND_ENUM_MEMBER: u32 = 20;
const KEYWORD_COMPLETIONS: &[&str] = &[
    "local",
    "const",
    "function",
    "task function",
    "object",
    "enum",
    "signal",
    "state",
    "watch",
    "if",
    "switch",
    "match",
    "for",
    "while",
    "do",
    "return",
    "spawn",
    "yield",
];

#[derive(Debug, Clone)]
struct Declaration {
    name: String,
    kind: u32,
    detail: String,
    hover: String,
    path: PathBuf,
    name_span: Span,
    keyword_span: Option<Span>,
    is_const: bool,
}

#[derive(Debug, Clone)]
struct MemberInfo {
    name: String,
    kind: u32,
    detail: String,
}

#[derive(Debug, Clone)]
struct DocumentIndex {
    path: PathBuf,
    tokens: Vec<Token>,
    declarations: Vec<Declaration>,
    object_members: HashMap<String, Vec<MemberInfo>>,
    enum_members: HashMap<String, Vec<MemberInfo>>,
    typed_bindings: HashMap<String, String>,
}

fn diagnostics_for(uri: &str, text: &str) -> Vec<Value> {
    let Ok(path) = uri_to_path(uri) else {
        return vec![diagnostic(0, 0, "invalid file URI".to_string())];
    };
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let root = nearest_project_root(path.parent().unwrap_or(&cwd), &cwd);
    let Ok(compiler) = Compiler::discover(&root) else {
        return Vec::new();
    };

    match compiler.compile_source_at_path(text, &path) {
        Ok(_) => Vec::new(),
        Err(error) => error_to_diagnostics(error),
    }
}

fn error_to_diagnostics(error: CompilerError) -> Vec<Value> {
    match error {
        CompilerError::Lex { message } | CompilerError::Parse { message } => {
            vec![diagnostic_from_message(&message)]
        }
        CompilerError::Semantic { messages } => messages
            .into_iter()
            .map(|message| diagnostic_from_message(&message))
            .collect(),
        CompilerError::Validation { message } => message
            .lines()
            .map(|line| diagnostic_from_validation(line.trim()))
            .collect(),
        other => vec![diagnostic(0, 0, other.to_string())],
    }
}

fn diagnostic_from_message(message: &str) -> Value {
    if let Some((line, col)) = extract_one_based_position(message) {
        diagnostic(line.saturating_sub(1), col.saturating_sub(1), message.to_string())
    } else {
        diagnostic(0, 0, message.to_string())
    }
}

fn diagnostic_from_validation(message: &str) -> Value {
    if let Some((line, col)) = extract_zero_based_validation_position(message) {
        diagnostic(line, col, message.to_string())
    } else {
        diagnostic(0, 0, message.to_string())
    }
}

fn diagnostic(line: usize, col: usize, message: String) -> Value {
    json!({
        "range": {
            "start": { "line": line, "character": col },
            "end": { "line": line, "character": col + 1 }
        },
        "severity": 1,
        "source": "xluau",
        "message": message
    })
}

fn build_document_index(path: PathBuf, source: String) -> DocumentIndex {
    let tokens = Lexer::new(&source).tokenize().unwrap_or_default();
    let program = parse_program_best_effort(&source);
    let mut declarations = Vec::new();
    let mut object_members = HashMap::<String, Vec<MemberInfo>>::new();
    let mut enum_members = HashMap::<String, Vec<MemberInfo>>::new();
    let mut typed_bindings = HashMap::<String, String>::new();

    if let Some(program) = program {
        for stmt in &program.block {
            match stmt {
                Stmt::Function(function) => {
                    if let Some(name_span) =
                        find_identifier_span(&tokens, function.span.start, function.span.end, &function.name.root, 0)
                    {
                        let signature = render_function_signature(function);
                        declarations.push(Declaration {
                            name: function.name.root.clone(),
                            kind: KIND_FUNCTION,
                            detail: signature.clone(),
                            hover: signature,
                            path: path.clone(),
                            name_span,
                            keyword_span: first_keyword_span(
                                &tokens,
                                function.span.start,
                                function.span.end,
                                Keyword::Function,
                            ),
                            is_const: false,
                        });
                    }
                }
                Stmt::Object(object) => {
                    if let Some(name_span) =
                        find_identifier_span(&tokens, object.span.start, object.span.end, &object.name, 0)
                    {
                        let mut members = Vec::new();
                        for field in &object.fields {
                            members.push(MemberInfo {
                                name: field.name.clone(),
                                kind: KIND_PROPERTY,
                                detail: format!("{}: {}", field.name, field.annotation),
                            });
                        }
                        for method in &object.methods {
                            members.push(MemberInfo {
                                name: method.name.clone(),
                                kind: KIND_FUNCTION,
                                detail: render_object_method_signature(method),
                            });
                        }
                        object_members.insert(object.name.clone(), members);
                        declarations.push(Declaration {
                            name: object.name.clone(),
                            kind: KIND_CLASS,
                            detail: format!("object {}", object.name),
                            hover: render_object_hover(object),
                            path: path.clone(),
                            name_span,
                            keyword_span: first_keyword_span(
                                &tokens,
                                object.span.start,
                                object.span.end,
                                Keyword::Object,
                            ),
                            is_const: false,
                        });
                    }
                }
                Stmt::Enum(decl) => {
                    if let Some(name_span) =
                        find_identifier_span(&tokens, decl.span.start, decl.span.end, &decl.name, 0)
                    {
                        let members = decl
                            .members
                            .iter()
                            .map(|member| MemberInfo {
                                name: member.name.clone(),
                                kind: KIND_ENUM_MEMBER,
                                detail: format!("{}.{}", decl.name, member.name),
                            })
                            .collect::<Vec<_>>();
                        enum_members.insert(decl.name.clone(), members);
                        declarations.push(Declaration {
                            name: decl.name.clone(),
                            kind: KIND_ENUM,
                            detail: format!("enum {}", decl.name),
                            hover: render_enum_hover(decl),
                            path: path.clone(),
                            name_span,
                            keyword_span: first_keyword_span(
                                &tokens,
                                decl.span.start,
                                decl.span.end,
                                Keyword::Enum,
                            ),
                            is_const: false,
                        });
                    }
                }
                Stmt::Signal(signal) => {
                    if let Some(name_span) =
                        find_identifier_span(&tokens, signal.span.start, signal.span.end, &signal.name, 0)
                    {
                        let signature = render_signal_signature(signal);
                        declarations.push(Declaration {
                            name: signal.name.clone(),
                            kind: KIND_VARIABLE,
                            detail: signature.clone(),
                            hover: signature,
                            path: path.clone(),
                            name_span,
                            keyword_span: first_keyword_span(
                                &tokens,
                                signal.span.start,
                                signal.span.end,
                                Keyword::Signal,
                            ),
                            is_const: false,
                        });
                    }
                }
                Stmt::State(state) => {
                    if let Pattern::Name(name) = &state.binding.pattern
                        && let Some(name_span) =
                            find_identifier_span(&tokens, state.span.start, state.span.end, name, 0)
                    {
                        let annotation = state.binding.type_annotation.clone();
                        if let Some(annotation) = &annotation {
                            typed_bindings.insert(name.clone(), annotation.clone());
                        }
                        declarations.push(Declaration {
                            name: name.clone(),
                            kind: KIND_VARIABLE,
                            detail: render_state_signature(name, annotation.as_deref()),
                            hover: render_state_signature(name, annotation.as_deref()),
                            path: path.clone(),
                            name_span,
                            keyword_span: first_keyword_span(
                                &tokens,
                                state.span.start,
                                state.span.end,
                                Keyword::State,
                            ),
                            is_const: false,
                        });
                    }
                }
                Stmt::TypeAlias { raw, span } => {
                    if let Some(name) = type_alias_name(raw)
                        && let Some(name_span) =
                            find_identifier_span(&tokens, span.start, span.end, &name, 0)
                    {
                        declarations.push(Declaration {
                            name,
                            kind: KIND_TYPE,
                            detail: raw.clone(),
                            hover: raw.clone(),
                            path: path.clone(),
                            name_span,
                            keyword_span: first_keyword_span(
                                &tokens,
                                span.start,
                                span.end,
                                Keyword::Type,
                            ),
                            is_const: false,
                        });
                    }
                }
                Stmt::Local(local) => {
                    let mut occurrence = 0usize;
                    for binding in &local.bindings {
                        if let Binding {
                            pattern: Pattern::Name(name),
                            type_annotation,
                        } = binding
                            && let Some(name_span) =
                                find_identifier_span(&tokens, local.span.start, local.span.end, name, occurrence)
                        {
                            occurrence += 1;
                            if let Some(annotation) = type_annotation {
                                typed_bindings.insert(name.clone(), annotation.clone());
                            }
                            declarations.push(Declaration {
                                name: name.clone(),
                                kind: KIND_VARIABLE,
                                detail: render_local_signature(name, type_annotation.as_deref(), local.is_const),
                                hover: render_local_signature(name, type_annotation.as_deref(), local.is_const),
                                path: path.clone(),
                                name_span,
                                keyword_span: Some(local.span),
                                is_const: local.is_const,
                            });
                        }
                    }
                }
                _ => {}
            }
        }
    }

    DocumentIndex {
        path,
        tokens,
        declarations,
        object_members,
        enum_members,
        typed_bindings,
    }
}

fn parse_program_best_effort(source: &str) -> Option<xluau::ast::Program> {
    let mut candidate = source.to_string();
    loop {
        let tokens = Lexer::new(&candidate).tokenize().ok()?;
        if let Ok(program) = Parser::new(&candidate, tokens).parse_program() {
            return Some(program);
        }
        let Some(line_break) = candidate.rfind('\n') else {
            return None;
        };
        candidate.truncate(line_break);
    }
}

fn workspace_documents(
    compiler: &Compiler,
    open_documents: &HashMap<String, String>,
) -> Result<Vec<DocumentIndex>, Box<dyn Error>> {
    let mut paths = compiler.collect_project_files()?;
    let mut seen = paths.iter().cloned().collect::<HashSet<_>>();
    for uri in open_documents.keys() {
        if let Ok(path) = uri_to_path(uri)
            && path.starts_with(&compiler.root)
            && seen.insert(path.clone())
        {
            paths.push(path);
        }
    }

    let mut documents = Vec::new();
    for path in paths {
        let uri = path_to_uri(&path)?;
        let source = open_documents
            .get(&uri)
            .cloned()
            .unwrap_or_else(|| fs::read_to_string(&path).unwrap_or_default());
        documents.push(build_document_index(path, source));
    }
    Ok(documents)
}

fn compiler_for_path(path: &Path) -> Result<Compiler, Box<dyn Error>> {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let root = nearest_project_root(path.parent().unwrap_or(&cwd), &cwd);
    Ok(Compiler::discover(root)?)
}

fn find_declaration<'a>(index: &'a DocumentIndex, name: &str) -> Option<&'a Declaration> {
    index.declarations.iter().find(|decl| decl.name == name)
}

fn decl_to_symbol(decl: &Declaration) -> Value {
    json!({
        "name": decl.name,
        "kind": decl.kind,
        "containerName": Value::Null,
        "location": {
            "uri": path_to_uri(&decl.path).unwrap_or_default(),
            "range": range_from_span(decl.name_span, decl.name.len())
        }
    })
}

fn render_function_signature(function: &xluau::ast::FunctionDecl) -> String {
    let mut text = String::new();
    if function.is_task {
        text.push_str("task ");
    }
    text.push_str("function ");
    text.push_str(&function.name.root);
    if let Some(generics) = &function.generics {
        text.push_str(generics);
    }
    text.push('(');
    text.push_str(&render_params(&function.params));
    text.push(')');
    if let Some(return_type) = &function.return_type {
        text.push_str(": ");
        text.push_str(return_type);
    }
    text
}

fn render_object_method_signature(method: &xluau::ast::ObjectMethod) -> String {
    let mut text = String::new();
    text.push_str("function ");
    if method.is_static {
        text.push_str("static ");
    }
    text.push_str(&method.name);
    if let Some(generics) = &method.generics {
        text.push_str(generics);
    }
    text.push('(');
    text.push_str(&render_params(&method.params));
    text.push(')');
    if let Some(return_type) = &method.return_type {
        text.push_str(": ");
        text.push_str(return_type);
    }
    text
}

fn render_object_hover(object: &xluau::ast::ObjectDecl) -> String {
    let mut lines = vec![if let Some(base) = &object.extends {
        format!("object {} extends {}", object.name, base)
    } else {
        format!("object {}", object.name)
    }];
    for field in &object.fields {
        lines.push(format!("  {}: {}", field.name, field.annotation));
    }
    for method in &object.methods {
        lines.push(format!("  {}", render_object_method_signature(method)));
    }
    lines.join("\n")
}

fn render_enum_hover(decl: &xluau::ast::EnumDecl) -> String {
    let mut lines = vec![format!("enum {}", decl.name)];
    for member in &decl.members {
        lines.push(format!("  {}", member.name));
    }
    lines.join("\n")
}

fn render_signal_signature(signal: &xluau::ast::SignalDecl) -> String {
    let params = signal
        .params
        .iter()
        .map(|param| {
            if let Some(annotation) = &param.annotation {
                format!("{}: {}", param.name, annotation)
            } else {
                param.name.clone()
            }
        })
        .collect::<Vec<_>>()
        .join(", ");
    format!("signal {}({})", signal.name, params)
}

fn render_state_signature(name: &str, annotation: Option<&str>) -> String {
    match annotation {
        Some(annotation) => format!("state {}: {}", name, annotation),
        None => format!("state {}", name),
    }
}

fn render_local_signature(name: &str, annotation: Option<&str>, is_const: bool) -> String {
    let prefix = if is_const { "const" } else { "local" };
    match annotation {
        Some(annotation) => format!("{} {}: {}", prefix, name, annotation),
        None => format!("{} {}", prefix, name),
    }
}

fn render_params(params: &[xluau::ast::Param]) -> String {
    params
        .iter()
        .map(|param| match param {
            xluau::ast::Param::Binding(binding) => match &binding.pattern {
                Pattern::Name(name) => match &binding.type_annotation {
                    Some(annotation) => format!("{}: {}", name, annotation),
                    None => name.clone(),
                },
                _ => "_".to_string(),
            },
            xluau::ast::Param::VarArg(Some(annotation)) => format!("...: {}", annotation),
            xluau::ast::Param::VarArg(None) => "...".to_string(),
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn find_identifier_span(
    tokens: &[Token],
    start: usize,
    _end: usize,
    name: &str,
    occurrence: usize,
) -> Option<Span> {
    tokens
        .iter()
        .filter(|token| {
            token.kind == TokenKind::Identifier
                && token.lexeme == name
                && token.span.start >= start
        })
        .nth(occurrence)
        .map(|token| token.span)
}

fn first_keyword_span(
    tokens: &[Token],
    start: usize,
    _end: usize,
    keyword: Keyword,
) -> Option<Span> {
    tokens
        .iter()
        .find(|token| {
            token.kind == TokenKind::Keyword(keyword)
                && token.span.start >= start
        })
        .map(|token| token.span)
}

fn member_completion_receiver(source: &str, offset: usize) -> Option<String> {
    let before = source.get(..offset)?;
    let trimmed = before.trim_end();
    let stripped = trimmed
        .strip_suffix('.')
        .or_else(|| trimmed.strip_suffix("?."))
        .or_else(|| trimmed.strip_suffix(':'))?;
    let name = stripped
        .chars()
        .rev()
        .take_while(|ch| ch.is_ascii_alphanumeric() || *ch == '_')
        .collect::<String>()
        .chars()
        .rev()
        .collect::<String>();
    if name.is_empty() {
        None
    } else {
        Some(name)
    }
}

fn member_completion_items(index: &DocumentIndex, receiver: &str) -> Vec<Value> {
    let mut items = Vec::new();
    if let Some(enum_members) = index.enum_members.get(receiver) {
        for member in enum_members {
            items.push(json!({
                "label": member.name,
                "kind": member.kind,
                "detail": member.detail
            }));
        }
        return items;
    }

    let receiver_type = index
        .typed_bindings
        .get(receiver)
        .and_then(|annotation| simple_type_name(annotation));
    let Some(receiver_type) = receiver_type else {
        return items;
    };
    if let Some(members) = index.object_members.get(receiver_type) {
        for member in members {
            items.push(json!({
                "label": member.name,
                "kind": member.kind,
                "detail": member.detail
            }));
        }
    }
    items
}

fn simple_type_name(annotation: &str) -> Option<&str> {
    let trimmed = annotation.trim();
    let start = trimmed
        .find(|ch: char| ch.is_ascii_alphabetic() || ch == '_')?;
    let head = &trimmed[start..];
    let end = head
        .find(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'))
        .unwrap_or(head.len());
    Some(&head[..end])
}

fn token_at_offset(tokens: &[Token], offset: usize) -> Option<(usize, &Token)> {
    tokens.iter().enumerate().find(|(_, token)| {
        offset >= token.span.start && offset <= token.span.end && token.kind != TokenKind::Eof
    })
}

fn is_require_string_token(tokens: &[Token], index: usize) -> bool {
    matches!(
        (
            tokens.get(index.wrapping_sub(1)).map(|token| &token.kind),
            tokens.get(index.wrapping_sub(2)).map(|token| &token.kind),
        ),
        (Some(TokenKind::Identifier), _)
            if tokens[index - 1].lexeme == "require"
    ) || matches!(
        (
            tokens.get(index.wrapping_sub(1)).map(|token| &token.kind),
            tokens.get(index.wrapping_sub(2)).map(|token| &token.kind),
            tokens.get(index.wrapping_sub(3)).map(|token| &token.kind),
        ),
        (
            Some(TokenKind::Symbol(Symbol::LParen)),
            Some(TokenKind::Identifier),
            _
        ) if tokens[index - 2].lexeme == "require"
    )
}

fn decode_string_token(token: &Token) -> String {
    token
        .lexeme
        .strip_prefix(['"', '\'', '`'])
        .and_then(|text| text.strip_suffix(['"', '\'', '`']))
        .unwrap_or(&token.lexeme)
        .to_string()
}

fn rewrap_string_literal(original: &str, replacement_inner: &str) -> String {
    let quote = original.chars().next().unwrap_or('"');
    format!("{quote}{replacement_inner}{quote}")
}

fn keyword_completion_item(keyword: &str) -> Value {
    json!({
        "label": keyword,
        "kind": 14,
        "detail": "keyword"
    })
}

fn alias_completion_label(alias: &str) -> String {
    alias.strip_suffix("/*")
        .map(|prefix| format!("{prefix}/"))
        .unwrap_or_else(|| alias.to_string())
}

fn extract_const_name(message: &str) -> Option<&str> {
    let start = message.find("const `")? + "const `".len();
    let rest = &message[start..];
    let end = rest.find('`')?;
    Some(&rest[..end])
}

fn switch_default_edit(source: &str, line: usize) -> Option<Value> {
    let lines = source.lines().collect::<Vec<_>>();
    let (switch_line, end_line, indent) = block_bounds(&lines, line, "switch")?;
    let is_expression = lines
        .iter()
        .take(end_line)
        .skip(switch_line)
        .any(|line| line.trim_start().starts_with("case ") && line.contains(" then "));
    let new_text = if is_expression {
        format!("{indent}default then nil\n")
    } else {
        format!("{indent}default\n{indent}    -- TODO\n")
    };
    Some(json!({
        "range": {
            "start": { "line": end_line, "character": 0 },
            "end": { "line": end_line, "character": 0 }
        },
        "newText": new_text
    }))
}

fn match_fallback_edit(source: &str, line: usize) -> Option<Value> {
    let lines = source.lines().collect::<Vec<_>>();
    let (_match_line, end_line, indent) = block_bounds(&lines, line, "match")?;
    Some(json!({
        "range": {
            "start": { "line": end_line, "character": 0 },
            "end": { "line": end_line, "character": 0 }
        },
        "newText": format!("{indent}_\n{indent}    -- TODO\n")
    }))
}

fn block_bounds(lines: &[&str], requested_line: usize, head_keyword: &str) -> Option<(usize, usize, String)> {
    let head_line = (0..=requested_line.min(lines.len().saturating_sub(1)))
        .rev()
        .find(|line| lines[*line].trim_start().starts_with(head_keyword))?;
    let body_indent = format!("{}    ", leading_whitespace(lines[head_line]));
    let mut depth = 0isize;
    for (index, line) in lines.iter().enumerate().skip(head_line + 1) {
        let trimmed = line.trim_start();
        if starts_end_scoped_block(trimmed) {
            depth += 1;
        }
        if trimmed == "end" {
            if depth == 0 {
                return Some((head_line, index, body_indent));
            }
            depth -= 1;
        }
    }
    None
}

fn starts_end_scoped_block(trimmed: &str) -> bool {
    [
        "if ",
        "switch ",
        "match ",
        "while ",
        "for ",
        "do",
        "function ",
        "task function ",
        "object ",
        "enum ",
        "on ",
        "once ",
        "watch ",
        "spawn ",
    ]
    .iter()
    .any(|prefix| trimmed.starts_with(prefix))
}

fn leading_whitespace(line: &str) -> &str {
    let indent_len = line.len().saturating_sub(line.trim_start().len());
    &line[..indent_len]
}

fn range_from_token(token: &Token) -> Value {
    range_from_span(token.span, token.lexeme.chars().count())
}

fn range_from_span(span: Span, len: usize) -> Value {
    let start_line = span.line.saturating_sub(1);
    let start_char = span.column.saturating_sub(1);
    json!({
        "start": { "line": start_line, "character": start_char },
        "end": { "line": start_line, "character": start_char + len }
    })
}

fn full_document_range(source: &str) -> Value {
    let mut lines = source.lines();
    let mut last_line = 0usize;
    let mut last_char = 0usize;
    for (index, line) in lines.by_ref().enumerate() {
        last_line = index;
        last_char = line.chars().count();
    }
    json!({
        "start": { "line": 0, "character": 0 },
        "end": { "line": last_line, "character": last_char }
    })
}

fn nearest_project_root(start: &Path, fallback: &Path) -> PathBuf {
    for ancestor in start.ancestors() {
        if ancestor.join("xluau.config.json").is_file() {
            return ancestor.to_path_buf();
        }
    }
    fallback.to_path_buf()
}

fn uri_to_path(uri: &str) -> Result<PathBuf, Box<dyn Error>> {
    let url = Url::parse(uri)?;
    Ok(url.to_file_path().map_err(|_| io::Error::other("non-file URI"))?)
}

fn path_to_uri(path: &Path) -> Result<String, Box<dyn Error>> {
    Ok(Url::from_file_path(path)
        .map_err(|_| io::Error::other("invalid file path"))?
        .to_string())
}

fn position_to_offset(source: &str, line: usize, character: usize) -> usize {
    let mut current_line = 0usize;
    let mut current_char = 0usize;
    for (offset, ch) in source.char_indices() {
        if current_line == line && current_char == character {
            return offset;
        }
        if ch == '\n' {
            current_line += 1;
            current_char = 0;
            if current_line > line {
                return offset;
            }
        } else {
            current_char += 1;
        }
    }
    source.len()
}

fn display_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn extract_one_based_position(message: &str) -> Option<(usize, usize)> {
    let (_, rest) = message.rsplit_once(" at ")?;
    let (line, col) = rest.split_once(':')?;
    Some((line.parse().ok()?, col.parse().ok()?))
}

fn extract_zero_based_validation_position(message: &str) -> Option<(usize, usize)> {
    let line_marker = "line: ";
    let char_marker = "character: ";
    let line_start = message.find(line_marker)? + line_marker.len();
    let line = message[line_start..]
        .chars()
        .take_while(|ch| ch.is_ascii_digit())
        .collect::<String>()
        .parse()
        .ok()?;
    let char_start = message.find(char_marker)? + char_marker.len();
    let col = message[char_start..]
        .chars()
        .take_while(|ch| ch.is_ascii_digit())
        .collect::<String>()
        .parse()
        .ok()?;
    Some((line, col))
}

fn type_alias_name(raw: &str) -> Option<String> {
    let trimmed = raw.trim().strip_prefix("export ").unwrap_or(raw.trim());
    let rest = trimmed.strip_prefix("type ")?;
    let eq = rest.find('=')?;
    Some(
        rest[..eq]
            .trim()
            .split('<')
            .next()
            .unwrap_or_default()
            .trim()
            .to_string(),
    )
}

fn read_message(reader: &mut impl BufRead) -> Result<Option<Value>, Box<dyn Error>> {
    let mut content_length = None::<usize>;
    loop {
        let mut header = String::new();
        let bytes = reader.read_line(&mut header)?;
        if bytes == 0 {
            return Ok(None);
        }
        let trimmed = header.trim();
        if trimmed.is_empty() {
            break;
        }
        if let Some(rest) = trimmed.strip_prefix("Content-Length:") {
            content_length = Some(rest.trim().parse()?);
        }
    }

    let Some(content_length) = content_length else {
        return Ok(None);
    };
    let mut body = vec![0u8; content_length];
    reader.read_exact(&mut body)?;
    Ok(Some(serde_json::from_slice(&body)?))
}

fn write_message(writer: &mut impl Write, value: &Value) -> Result<(), Box<dyn Error>> {
    let body = serde_json::to_vec(value)?;
    write!(writer, "Content-Length: {}\r\n\r\n", body.len())?;
    writer.write_all(&body)?;
    writer.flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{
        collections::HashMap,
        fs,
        path::{Path, PathBuf},
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::{
        build_document_index, decode_string_token, diagnostic_from_message,
        extract_one_based_position, extract_zero_based_validation_position, is_require_string_token,
        member_completion_items, path_to_uri, range_from_token, switch_default_edit, token_at_offset,
        type_alias_name, workspace_documents,
    };
    use xluau::Compiler;
    use xluau::lexer::{Lexer, TokenKind};
    use xluau::parser::Parser;

    fn temp_project(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("xluau_lsp_{name}_{nonce}"));
        fs::create_dir_all(&root).expect("temp root");
        root
    }

    fn write_file(root: &Path, relative: &str, contents: &str) {
        let path = root.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("parent");
        }
        fs::write(path, contents).expect("write");
    }

    #[test]
    fn parses_one_based_positions() {
        assert_eq!(extract_one_based_position("expected expression at 4:12"), Some((4, 12)));
    }

    #[test]
    fn parses_validation_positions() {
        let input = "Error { start: Position { line: 0, character: 21 }, message: \"oops\" }";
        assert_eq!(extract_zero_based_validation_position(input), Some((0, 21)));
    }

    #[test]
    fn extracts_type_alias_names() {
        assert_eq!(type_alias_name("type Result<T> = T"), Some("Result".to_string()));
        assert_eq!(
            type_alias_name("export type Settings = { enabled: boolean }"),
            Some("Settings".to_string())
        );
    }

    #[test]
    fn builds_diagnostics_from_parse_messages() {
        let diagnostic = diagnostic_from_message("expected expression at 3:5");
        assert_eq!(diagnostic["range"]["start"]["line"], 2);
        assert_eq!(diagnostic["range"]["start"]["character"], 4);
    }

    #[test]
    fn detects_require_string_tokens() {
        let source = r#"local math = require("@shared/math")"#.to_string();
        let index = build_document_index(PathBuf::from("main.xl"), source);
        let (token_index, token) = index
            .tokens
            .iter()
            .enumerate()
            .find(|(_, token)| token.kind == TokenKind::String)
            .expect("string token");
        assert!(is_require_string_token(&index.tokens, token_index));
        assert_eq!(decode_string_token(token), "@shared/math");
    }

    #[test]
    fn completes_enum_members() {
        let source = r#"
enum Direction
    North
    South
end
"#
        .trim_start()
        .to_string();
        let tokens = Lexer::new(&source).tokenize().expect("tokens");
        let parsed = Parser::new(&source, tokens).parse_program();
        assert!(parsed.is_ok(), "{parsed:?}");
        let index = build_document_index(PathBuf::from("main.xl"), source);
        let items = member_completion_items(&index, "Direction");
        assert!(items.iter().any(|item| item["label"] == "North"));
        assert!(items.iter().any(|item| item["label"] == "South"));
    }

    #[test]
    fn resolves_workspace_documents_with_open_overrides() {
        let root = temp_project("workspace_docs");
        write_file(
            &root,
            "xluau.config.json",
            r#"{"include":["src/**/*.xl"],"outDir":"out"}"#,
        );
        write_file(&root, "src/main.xl", "local value = 1\n");
        let compiler = Compiler::discover(&root).expect("compiler");
        let uri = path_to_uri(&root.join("src/main.xl")).expect("uri");
        let mut open = HashMap::new();
        open.insert(uri, "local value = 2\n".to_string());
        let docs = workspace_documents(&compiler, &open).expect("docs");
        assert_eq!(docs.len(), 1);
        assert!(docs[0].tokens.iter().any(|token| token.lexeme == "2"));
    }

    #[test]
    fn builds_switch_default_edit() {
        let source = "switch value\n    case 1\n        print(1)\nend\n";
        let edit = switch_default_edit(source, 0).expect("edit");
        assert_eq!(edit["newText"], "    default\n        -- TODO\n");
    }

    #[test]
    fn token_ranges_cover_identifier_text() {
        let source = "local hero = 1".to_string();
        let index = build_document_index(PathBuf::from("main.xl"), source.clone());
        let offset = source.find("hero").expect("hero");
        let (_, token) = token_at_offset(&index.tokens, offset).expect("token");
        let range = range_from_token(token);
        assert_eq!(range["start"]["character"], 6);
        assert_eq!(range["end"]["character"], 10);
    }
}
