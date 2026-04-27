use std::{
    fs,
    path::{Path, PathBuf},
};

use glob::glob;
use luau_parser::parser::Parser as LuauParser;
use thiserror::Error;

use crate::{
    config::XluauConfig, emitter::Emitter, formatter::format_luau, lexer::Lexer, parser::Parser,
};

pub type Result<T> = std::result::Result<T, CompilerError>;

#[derive(Debug)]
pub struct BuildArtifact {
    pub input: PathBuf,
    pub output: PathBuf,
    pub luau: String,
}

#[derive(Debug)]
pub struct Compiler {
    pub root: PathBuf,
    pub config: XluauConfig,
}

#[derive(Debug, Error)]
pub enum CompilerError {
    #[error("io error while accessing {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("invalid config at {path}: {source}")]
    Config {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("lex error: {message}")]
    Lex { message: String },
    #[error("parse error: {message}")]
    Parse { message: String },
    #[error("semantic errors:\n{messages:?}")]
    Semantic { messages: Vec<String> },
    #[error("format error: {message}")]
    Format { message: String },
    #[error("luau validation error: {message}")]
    Validation { message: String },
    #[error("{0}")]
    Other(String),
}

impl Compiler {
    pub fn discover(root: impl Into<PathBuf>) -> Result<Self> {
        let root = root.into();
        let config = XluauConfig::load_from(&root)?;
        Ok(Self { root, config })
    }

    pub fn compile_source(&self, source: &str) -> Result<String> {
        if self.validate_luau(source).is_ok() {
            return format_luau(source);
        }

        let tokens = Lexer::new(source).tokenize()?;
        let mut parser = Parser::new(source, tokens);
        let program = parser.parse_program()?;
        let mut emitter = Emitter::new();
        let raw = emitter.emit_program(&program)?;
        self.validate_luau(&raw)?;
        format_luau(&raw)
    }

    pub fn build_file(&self, path: &Path) -> Result<BuildArtifact> {
        let input = if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.root.join(path)
        };
        let source = fs::read_to_string(&input).map_err(|source| CompilerError::Io {
            path: input.clone(),
            source,
        })?;
        let luau = self.compile_source(&source)?;
        let output = self.output_path_for(&input)?;
        Ok(BuildArtifact {
            input,
            output,
            luau,
        })
    }

    pub fn build_project(&self) -> Result<Vec<BuildArtifact>> {
        let mut files = Vec::new();
        for pattern in &self.config.include {
            let absolute = self.root.join(pattern);
            let Some(pattern) = absolute.to_str() else {
                return Err(CompilerError::Other(format!(
                    "unsupported glob pattern path: {}",
                    absolute.display()
                )));
            };
            for entry in glob(pattern).map_err(|error| CompilerError::Other(error.to_string()))? {
                let path = entry.map_err(|error| CompilerError::Other(error.to_string()))?;
                if self
                    .config
                    .exclude
                    .iter()
                    .any(|exclude| path.to_string_lossy().contains(exclude))
                {
                    continue;
                }
                files.push(self.build_file(&path)?);
            }
        }
        Ok(files)
    }

    pub fn write_artifact(&self, artifact: &BuildArtifact) -> Result<()> {
        if let Some(parent) = artifact.output.parent() {
            fs::create_dir_all(parent).map_err(|source| CompilerError::Io {
                path: parent.to_path_buf(),
                source,
            })?;
        }
        fs::write(&artifact.output, &artifact.luau).map_err(|source| CompilerError::Io {
            path: artifact.output.clone(),
            source,
        })
    }

    fn output_path_for(&self, input: &Path) -> Result<PathBuf> {
        let relative = input
            .strip_prefix(&self.root)
            .or_else(|_| input.strip_prefix(self.root.join(&self.config.base_dir)))
            .unwrap_or(input);
        let mut output = self.root.join(&self.config.out_dir).join(relative);
        output.set_extension("luau");
        Ok(output)
    }

    fn validate_luau(&self, source: &str) -> Result<()> {
        let mut parser = LuauParser::new(source);
        let cst = parser.parse("<memory>");
        if cst.errors.is_empty() {
            Ok(())
        } else {
            Err(CompilerError::Validation {
                message: cst
                    .errors
                    .iter()
                    .map(|error| format!("{error:?}"))
                    .collect::<Vec<_>>()
                    .join("\n"),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Compiler;

    fn compiler() -> Compiler {
        Compiler::discover(".").expect("compiler")
    }

    #[test]
    fn lowers_nullish_and_ternary() {
        let source = r#"
local timeout = config.timeout ?? 30
local label = isAdmin ? "admin" : "user"
"#;
        let output = compiler().compile_source(source).unwrap();
        assert!(output.contains("local _lhs0 = config.timeout"));
        assert!(output.contains("if _lhs0 ~= nil then _lhs0 else 30"));
        assert!(output.contains(r#"if isAdmin then "admin" else "user""#));
    }

    #[test]
    fn lowers_optional_chain_and_pipe() {
        let source = r#"
local words = str |> :lower() |> :split(" ")
local hp = entity?.GetHealth()
local first = list?.[1]
"#;
        let output = compiler().compile_source(source).unwrap();
        assert!(output.contains(r#"local _pipe0 = str:lower()"#));
        assert!(output.contains(r#"_pipe0:split(" ")"#));
        assert!(output.contains("local _opt"));
        assert!(output.contains("GetHealth"));
        assert!(output.contains("[1]"));
    }

    #[test]
    fn lowers_const_and_destructuring() {
        let source = r#"
const PI = 3.14
local { x, y: posY, role = "user" } = point
"#;
        let output = compiler().compile_source(source).unwrap();
        assert!(output.contains("local PI = 3.14"));
        assert!(output.contains("local x ="));
        assert!(output.contains("local posY ="));
        assert!(output.contains(r#"if _d"#));
    }

    #[test]
    fn rejects_const_reassignment() {
        let source = r#"
const PI = 3.14
PI = 4
"#;
        let err = compiler().compile_source(source).unwrap_err();
        assert!(format!("{err}").contains("cannot assign to const"));
    }

    #[test]
    fn lowers_nullish_assignment_pipe_placeholders_and_destructured_scopes() {
        let source = r#"
config.retries ??= getRetries()
local result = numbers |> filter(_, isEven) |> map(_, double)

function update({ x, y }: Point, [head, ...tail]: ArrayLike)
    return x, y, head, tail
end

for { name, score } in players do
    print(name, score)
end
"#;
        let output = compiler().compile_source(source).unwrap();
        assert!(output.contains("if config.retries == nil then"));
        assert!(output.contains("config.retries = getRetries()"));
        assert!(output.contains("filter(numbers, isEven)"));
        assert!(output.contains("map("));
        assert!(output.contains("function update(_param"));
        assert!(output.contains("local head ="));
        assert!(output.contains("for _for"));
        assert!(output.contains("local name ="));
        assert!(output.contains("local score ="));
    }

    #[test]
    fn accepts_wide_luau_grammar_via_passthrough() {
        let source = r##"
local foo: { [string]: number, bar: (number, string) -> () } = { bar = function(_x, _y) end }
foo.bar(1, "x")
foo["a"] += 1
local cast = foo :: any
local label = if cast then `value: {cast}` else "none"
export type Box<T = string> = { value: T }
for key, value in foo do
    if key then
        continue
    end
end
"##;
        let output = compiler().compile_source(source).unwrap();
        assert!(output.contains("foo[\"a\"] += 1"));
        assert!(output.contains("foo :: any"));
        assert!(output.contains("`value: {cast}`"));
        assert!(output.contains("export type Box"));
        assert!(output.contains("continue"));
    }

    #[test]
    fn supports_mixed_xluau_with_luau_compound_and_type_assertion() {
        let source = r#"
local count = (value :: number) ?? 0
stats.total += 1
"#;
        let output = compiler().compile_source(source).unwrap();
        assert!(output.contains("(value :: number)"));
        assert!(output.contains("stats.total = stats.total + 1"));
        assert!(output.contains("if _lhs"));
    }
}
