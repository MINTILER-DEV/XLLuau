use std::{
    fs,
    path::{Path, PathBuf},
};

use glob::glob;
use luau_parser::parser::Parser as LuauParser;
use thiserror::Error;

use crate::{
    config::XluauConfig,
    emitter::Emitter,
    formatter::format_luau,
    lexer::Lexer,
    module::{ModuleResolver, detect_circular_dependencies},
    parser::Parser,
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
        self.compile_source_with_path(source, Path::new("<memory>"))
    }

    pub fn build_file(&self, path: &Path) -> Result<BuildArtifact> {
        let input = if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.root.join(path)
        };
        self.check_cycles(std::slice::from_ref(&input))?;
        let source = fs::read_to_string(&input).map_err(|source| CompilerError::Io {
            path: input.clone(),
            source,
        })?;
        let luau = self.compile_source_with_path(&source, &input)?;
        let output = self.output_path_for(&input)?;
        Ok(BuildArtifact {
            input,
            output,
            luau,
        })
    }

    pub fn build_project(&self) -> Result<Vec<BuildArtifact>> {
        let files = self.collect_project_files()?;
        self.check_cycles(&files)?;

        let mut artifacts = Vec::new();
        for path in files {
            let source = fs::read_to_string(&path).map_err(|source| CompilerError::Io {
                path: path.clone(),
                source,
            })?;
            let luau = self.compile_source_with_path(&source, &path)?;
            let output = self.output_path_for(&path)?;
            artifacts.push(BuildArtifact {
                input: path,
                output,
                luau,
            });
        }
        Ok(artifacts)
    }

    fn collect_project_files(&self) -> Result<Vec<PathBuf>> {
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
                files.push(path);
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
        let cst = Self::parse_luau(source, "<memory>");
        if cst.errors.is_empty() || Self::wrapped_chunk_is_valid(source) {
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

    fn parse_luau(source: &str, uri: &str) -> luau_parser::types::Pointer<luau_parser::types::Cst> {
        let mut parser = LuauParser::new(source);
        parser.parse(uri)
    }

    fn wrapped_chunk_is_valid(source: &str) -> bool {
        let wrapped = format!("do\n{source}\nend");
        Self::parse_luau(&wrapped, "<memory:wrapped>")
            .errors
            .is_empty()
    }

    fn compile_source_with_path(&self, source: &str, path: &Path) -> Result<String> {
        let resolver = self.module_resolver();
        let rewritten_source = resolver.rewrite_requires(source, path)?;
        if self.validate_luau(&rewritten_source).is_ok() {
            return format_luau(&rewritten_source);
        }

        let tokens = Lexer::new(source).tokenize()?;
        let mut parser = Parser::new(source, tokens);
        let program = parser.parse_program()?;
        let mut emitter = Emitter::with_luau_target(self.config.luau_target.clone());
        let raw = emitter.emit_program(&program)?;
        let rewritten_output = resolver.rewrite_requires(&raw, path)?;
        self.validate_luau(&rewritten_output)?;
        format_luau(&rewritten_output)
    }

    fn module_resolver(&self) -> ModuleResolver {
        ModuleResolver::new(self.root.clone(), self.config.clone())
    }

    fn check_cycles(&self, entry_points: &[PathBuf]) -> Result<()> {
        detect_circular_dependencies(&self.module_resolver(), entry_points)
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::{Path, PathBuf},
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::Compiler;

    fn compiler() -> Compiler {
        Compiler::discover(".").expect("compiler")
    }

    fn temp_project(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("xluau_{name}_{nonce}"));
        fs::create_dir_all(&root).expect("temp project root");
        root
    }

    fn write_file(root: &Path, relative: &str, contents: &str) {
        let path = root.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("parent");
        }
        fs::write(path, contents).expect("write file");
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

    #[test]
    fn accepts_top_level_return_in_luau_passthrough() {
        let source = r#"
local value = 1
return value
"#;
        let output = compiler().compile_source(source).unwrap();
        assert!(output.contains("local value = 1"));
        assert!(output.contains("return value"));
    }

    #[test]
    fn resolves_aliases_and_index_files_for_filesystem_target() {
        let root = temp_project("phase3_filesystem");
        write_file(
            &root,
            "xluau.config.json",
            r#"{
  "include": ["src/**/*.xl"],
  "baseDir": "src",
  "target": "filesystem",
  "paths": {
    "@shared": "./src/shared"
  }
}"#,
        );
        write_file(
            &root,
            "src/main.xl",
            r#"local utils = require "@shared/utils"
local math = require("@shared/math")
"#,
        );
        write_file(&root, "src/shared/utils/init.xl", "return {}");
        write_file(&root, "src/shared/math.xl", "return {}");

        let compiler = Compiler::discover(&root).unwrap();
        let artifact = compiler.build_file(&root.join("src/main.xl")).unwrap();
        assert!(artifact.luau.contains(r#"require("./src/shared/utils")"#));
        assert!(artifact.luau.contains(r#"require("./src/shared/math")"#));
    }

    #[test]
    fn resolves_wildcard_aliases_for_filesystem_target() {
        let root = temp_project("phase3_filesystem_wildcard");
        write_file(
            &root,
            "xluau.config.json",
            r#"{
  "include": ["src/**/*.xl"],
  "baseDir": "src",
  "target": "filesystem",
  "paths": {
    "@shared/*": "./src/shared/*"
  }
}"#,
        );
        write_file(
            &root,
            "src/main.xl",
            r#"local math = require "@shared/math""#,
        );
        write_file(&root, "src/shared/math.xl", "return {}");

        let compiler = Compiler::discover(&root).unwrap();
        let artifact = compiler.build_file(&root.join("src/main.xl")).unwrap();
        assert!(artifact.luau.contains(r#"require("./src/shared/math")"#));
    }

    #[test]
    fn resolves_aliases_for_roblox_target() {
        let root = temp_project("phase3_roblox");
        write_file(
            &root,
            "xluau.config.json",
            r#"{
  "include": ["src/**/*.xl"],
  "baseDir": "src",
  "target": "roblox",
  "paths": {
    "@shared": "./src/shared"
  }
}"#,
        );
        write_file(
            &root,
            "src/server/main.xl",
            r#"local math = require "@shared/math""#,
        );
        write_file(&root, "src/shared/math.xl", "return {}");

        let compiler = Compiler::discover(&root).unwrap();
        let artifact = compiler
            .build_file(&root.join("src/server/main.xl"))
            .unwrap();
        assert!(
            artifact
                .luau
                .contains("require(script.Parent.Parent.shared.math)")
        );
    }

    #[test]
    fn resolves_aliases_for_custom_target() {
        let root = temp_project("phase3_custom");
        write_file(
            &root,
            "xluau.config.json",
            r#"{
  "include": ["src/**/*.xl"],
  "baseDir": "src",
  "target": "custom",
  "customTargetFunction": "resolveModule",
  "paths": {
    "@shared": "./src/shared"
  }
}"#,
        );
        write_file(
            &root,
            "src/main.xl",
            r#"local math = require "@shared/math""#,
        );
        write_file(&root, "src/shared/math.xl", "return {}");

        let compiler = Compiler::discover(&root).unwrap();
        let artifact = compiler.build_file(&root.join("src/main.xl")).unwrap();
        assert!(
            artifact
                .luau
                .contains(r#"require(resolveModule("shared/math"))"#)
        );
    }

    #[test]
    fn detects_circular_dependencies() {
        let root = temp_project("phase3_cycle");
        write_file(
            &root,
            "xluau.config.json",
            r#"{
  "include": ["src/**/*.xl"],
  "baseDir": "src",
  "target": "filesystem",
  "paths": {
    "@app": "./src"
  }
}"#,
        );
        write_file(&root, "src/main.xl", r#"local a = require "@app/a""#);
        write_file(&root, "src/a.xl", r#"local b = require "@app/b""#);
        write_file(&root, "src/b.xl", r#"local c = require "@app/c""#);
        write_file(&root, "src/c.xl", r#"local a = require "@app/a""#);

        let compiler = Compiler::discover(&root).unwrap();
        let error = compiler.build_file(&root.join("src/main.xl")).unwrap_err();
        assert!(format!("{error}").contains("Circular dependency detected"));
        assert!(format!("{error}").contains("src/a.xl"));
        assert!(format!("{error}").contains("src/b.xl"));
        assert!(format!("{error}").contains("src/c.xl"));
    }

    #[test]
    fn leaves_non_alias_string_requires_unchanged() {
        let source = r#"
local sibling = require "./sibling"
local parent = require("../parent")
"#;
        let output = compiler().compile_source(source).unwrap();
        assert!(output.contains(r#"require "./sibling""#));
        assert!(output.contains(r#"require("../parent")"#));
    }

    #[test]
    fn lowers_phase4_switch_enum_and_do_expression() {
        let source = r#"
enum Direction
    North
    South
    East
    West
end

local dir: Direction = Direction.North

switch dir
    case Direction.North
        print("north")
    case Direction.South
        print("south")
    case Direction.East
        print("east")
    case Direction.West
        print("west")
end

local label = switch dir
    case Direction.North then "N"
    case Direction.South then "S"
    default then "?"
end

local distance = do
    local dx = b.x - a.x
    local dy = b.y - a.y
    math.sqrt(dx ^ 2 + dy ^ 2)
end
"#;
        let output = compiler().compile_source(source).unwrap();
        assert!(output.contains("type Direction = \"North\" | \"South\" | \"East\" | \"West\""));
        assert!(output.contains("local Direction = table.freeze({"));
        assert!(output.contains("local _sw"));
        assert!(output.contains("local _swexpr"));
        assert!(output.contains("local _de"));
        assert!(output.contains("math.sqrt"));
    }

    #[test]
    fn lowers_phase4_match_and_comprehensions() {
        let source = r#"
type Result = { kind: "ok", value: number } | { kind: "err", error: string }

local doubled = { x * 2 for _, x in numbers if x > 0 }
local byValue = { [x] = x ^ 2 for _, x in numbers }
local flat = { value for _, row in matrix for _, value in row }

match result
    { kind = "ok", value = v }
        print(v)
    { kind = "err", error = e }
        print(e)
end
"#;
        let output = compiler().compile_source(source).unwrap();
        assert!(output.contains("local _comp"));
        assert!(output.contains("table.insert("));
        assert!(output.contains("for _, x in numbers do"));
        assert!(output.contains("local _mbind"));
        assert!(output.contains("local _mcond"));
        assert!(output.contains("print(v)"));
        assert!(output.contains("print(e)"));
    }

    #[test]
    fn rejects_non_exhaustive_switch_over_union() {
        let source = r#"
type Direction = "North" | "South"
local dir: Direction = "North"

switch dir
    case "North"
        print("north")
end
"#;
        let err = compiler().compile_source(source).unwrap_err();
        assert!(format!("{err}").contains("non-exhaustive switch"));
        assert!(format!("{err}").contains("South"));
    }

    #[test]
    fn rejects_non_exhaustive_match_over_discriminated_union() {
        let source = r#"
type Result = { kind: "ok", value: number } | { kind: "err", error: string }
local result: Result = { kind = "ok", value = 1 }

match result
    { kind = "ok", value = v }
        print(v)
end
"#;
        let err = compiler().compile_source(source).unwrap_err();
        assert!(format!("{err}").contains("non-exhaustive match"));
        assert!(format!("{err}").contains("err"));
    }

    #[test]
    fn lowers_phase5_generics_and_explicit_type_arguments() {
        let source = r#"
local function max<T extends Comparable>(a: T, b: T): T
    return if a > b then a else b
end

local function makeEmpty<T>(): T
    return nil :: any
end

local function fetch<T, Err = string>(url: string): Result<T, Err>
    return nil :: any
end

local empty = makeEmpty::<{ x: number, y: number }>()
local user = fetch::<User>("/api/user")
"#;
        let output = compiler().compile_source(source).unwrap();
        assert!(output.contains("local function max<T>(a: (T & Comparable), b: (T & Comparable)): T"));
        assert!(output.contains("((makeEmpty :: () -> { x: number, y: number }))()"));
        assert!(output.contains("((fetch :: (string) -> Result<User, string>))(\"/api/user\")"));
    }

    #[test]
    fn lowers_phase5_type_utilities_and_freeze() {
        let source = r#"
type Config = {
    readonly host: string,
    port: number,
    timeout: number?,
}

type PartialConfig = Partial<Config>
type HostConfig = Pick<Config, "host" | "port">
type Flags = Record<"debug" | "verbose", boolean>
type Present = Exclude<"ok" | "err" | nil, nil>

local function fetchUser(id: string, retries: number): User
    return nil :: any
end

type UserResult = ReturnType<typeof(fetchUser)>
type FetchParams = Parameters<typeof(fetchUser)>

const DEFAULTS = freeze {
    timeout = 30,
    retries = 3,
    host = "localhost",
}

type Defaults = Readonly<typeof(DEFAULTS)>
"#;
        let output = compiler().compile_source(source).unwrap();
        assert!(output.contains("type Config = { host: string, port: number, timeout: number? }"));
        assert!(output.contains("type PartialConfig = { host: string?, port: number?, timeout: number? }"));
        assert!(output.contains("type HostConfig = { host: string, port: number }"));
        assert!(output.contains("type Flags = { debug: boolean, verbose: boolean }"));
        assert!(output.contains("type Present = \"ok\" | \"err\""));
        assert!(output.contains("type UserResult = User"));
        assert!(output.contains("type FetchParams = (string, number)"));
        assert!(output.contains("local DEFAULTS = table.freeze({timeout = 30, retries = 3, host = \"localhost\"})"));
        assert!(output.contains("type Defaults = { timeout: number, retries: number, host: string }"));
    }

    #[test]
    fn lowers_phase5_readonly_for_legacy_target() {
        let root = temp_project("phase5_legacy_readonly");
        write_file(
            &root,
            "xluau.config.json",
            r#"{
  "include": ["src/**/*.xl"],
  "luauTarget": "legacy"
}"#,
        );
        write_file(
            &root,
            "src/main.xl",
            r#"
type Config = {
    readonly host: string,
    port: number,
}
"#,
        );

        let compiler = Compiler::discover(&root).unwrap();
        let artifact = compiler.build_file(&root.join("src/main.xl")).unwrap();
        assert!(artifact.luau.contains("type Config = { host: string, port: number }"));
        assert!(!artifact.luau.contains("readonly host"));
        assert!(!artifact.luau.contains("read host"));
    }
}
