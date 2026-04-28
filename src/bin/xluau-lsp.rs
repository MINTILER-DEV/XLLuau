use std::{
    collections::HashMap,
    error::Error,
    fs,
    io::{self, BufRead, BufReader, Write},
    path::{Path, PathBuf},
};

use serde_json::{Value, json};
use url::Url;
use xluau::{
    Compiler,
    compiler::CompilerError,
    formatter::format_source,
    lexer::{Lexer, Span},
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
                                    "documentSymbolProvider": true
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
                    let symbols = document_symbols(&text);
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

fn diagnostics_for(uri: &str, text: &str) -> Vec<Value> {
    let Ok(path) = uri_to_path(uri) else {
        return vec![diagnostic(
            0,
            0,
            "invalid file URI".to_string(),
        )];
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

fn document_symbols(source: &str) -> Vec<Value> {
    let Ok(tokens) = Lexer::new(source).tokenize() else {
        return Vec::new();
    };
    let Ok(program) = Parser::new(source, tokens).parse_program() else {
        return Vec::new();
    };
    let mut symbols = Vec::new();
    for stmt in &program.block {
        match stmt {
            xluau::ast::Stmt::Function(function) => symbols.push(symbol(
                function.name.root.clone(),
                12,
                function.span,
            )),
            xluau::ast::Stmt::Object(object) => {
                symbols.push(symbol(object.name.clone(), 5, object.span))
            }
            xluau::ast::Stmt::Enum(decl) => symbols.push(symbol(decl.name.clone(), 10, decl.span)),
            xluau::ast::Stmt::Signal(signal_decl) => {
                symbols.push(symbol(signal_decl.name.clone(), 13, signal_decl.span))
            }
            xluau::ast::Stmt::State(state) => {
                if let xluau::ast::Pattern::Name(name) = &state.binding.pattern {
                    symbols.push(symbol(name.clone(), 13, state.span));
                }
            }
            xluau::ast::Stmt::TypeAlias { raw, span } => {
                if let Some(name) = type_alias_name(raw) {
                    symbols.push(symbol(name, 11, *span));
                }
            }
            _ => {}
        }
    }
    symbols
}

fn symbol(name: String, kind: u32, span: Span) -> Value {
    let start_line = span.line.saturating_sub(1);
    let start_char = span.column.saturating_sub(1);
    let end_char = start_char + name.len();
    json!({
        "name": name,
        "kind": kind,
        "range": {
            "start": { "line": start_line, "character": start_char },
            "end": { "line": start_line, "character": end_char }
        },
        "selectionRange": {
            "start": { "line": start_line, "character": start_char },
            "end": { "line": start_line, "character": end_char }
        }
    })
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
    use super::{
        diagnostic_from_message, extract_one_based_position, extract_zero_based_validation_position,
        type_alias_name,
    };

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
}
