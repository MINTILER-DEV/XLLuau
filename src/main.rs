use std::{
    io,
    path::{Path, PathBuf},
    process::ExitCode,
};

use clap::{Parser, Subcommand};
use xluau::compiler::Compiler;

#[derive(Debug, Parser)]
#[command(name = "xluau")]
#[command(about = "XLuau compiler")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Build { path: Option<PathBuf> },
    Check { path: Option<PathBuf> },
}

#[derive(Debug, Clone, Copy)]
enum Operation {
    Build,
    Check,
}

#[derive(Debug, Clone)]
enum InvocationTarget {
    ProjectRoot(PathBuf),
    SingleFile {
        compiler_root: PathBuf,
        file_path: PathBuf,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> Result<(), Box<dyn std::error::Error>> {
    let cwd = std::env::current_dir()?;
    match cli.command {
        Command::Build { path } => run_operation(Operation::Build, path, &cwd)?,
        Command::Check { path } => run_operation(Operation::Check, path, &cwd)?,
    }
    Ok(())
}

fn run_operation(
    operation: Operation,
    path: Option<PathBuf>,
    cwd: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    match resolve_invocation_target(path, cwd)? {
        InvocationTarget::ProjectRoot(root) => {
            let compiler = Compiler::discover(&root)?;
            for artifact in compiler.build_project()? {
                match operation {
                    Operation::Build => {
                        compiler.write_artifact(&artifact)?;
                        println!(
                            "built {} -> {}",
                            artifact.input.display(),
                            artifact.output.display()
                        );
                    }
                    Operation::Check => {
                        println!(
                            "checked {} ({}) bytes",
                            artifact.input.display(),
                            artifact.luau.len()
                        );
                    }
                }
            }
        }
        InvocationTarget::SingleFile {
            compiler_root,
            file_path,
        } => {
            let compiler = Compiler::discover(&compiler_root)?;
            let artifact = compiler.build_file(&file_path)?;
            match operation {
                Operation::Build => {
                    compiler.write_artifact(&artifact)?;
                    println!(
                        "built {} -> {}",
                        artifact.input.display(),
                        artifact.output.display()
                    );
                }
                Operation::Check => {
                    println!(
                        "checked {} ({}) bytes",
                        artifact.input.display(),
                        artifact.luau.len()
                    );
                }
            }
        }
    }
    Ok(())
}

fn resolve_invocation_target(
    path: Option<PathBuf>,
    cwd: &Path,
) -> Result<InvocationTarget, Box<dyn std::error::Error>> {
    let Some(path) = path else {
        return Ok(InvocationTarget::ProjectRoot(cwd.to_path_buf()));
    };

    let absolute = absolutize(cwd, &path);
    if absolute.is_dir() {
        return Ok(InvocationTarget::ProjectRoot(absolute));
    }

    if is_config_path(&absolute) {
        let root = absolute.parent().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("config path {} has no parent directory", absolute.display()),
            )
        })?;
        return Ok(InvocationTarget::ProjectRoot(root.to_path_buf()));
    }

    let compiler_root = nearest_project_root(
        absolute.parent().unwrap_or(cwd),
        cwd,
    );

    Ok(InvocationTarget::SingleFile {
        compiler_root,
        file_path: absolute,
    })
}

fn absolutize(cwd: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        cwd.join(path)
    }
}

fn nearest_project_root(start: &Path, fallback: &Path) -> PathBuf {
    for ancestor in start.ancestors() {
        if ancestor.join("xluau.config.json").is_file() {
            return ancestor.to_path_buf();
        }
    }
    fallback.to_path_buf()
}

fn is_config_path(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(|name| name.eq_ignore_ascii_case("xluau.config.json"))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::{Path, PathBuf},
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::{InvocationTarget, nearest_project_root, resolve_invocation_target};

    fn temp_dir(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("xluau_cli_{name}_{nonce}"));
        fs::create_dir_all(&root).expect("temp dir");
        root
    }

    fn write_file(path: &Path, contents: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("parent");
        }
        fs::write(path, contents).expect("write");
    }

    #[test]
    fn resolves_directory_as_project_root() {
        let root = temp_dir("dir_project");
        write_file(&root.join("xluau.config.json"), "{}");

        let target =
            resolve_invocation_target(Some(root.clone()), Path::new("D:/unused")).expect("target");
        match target {
            InvocationTarget::ProjectRoot(found) => assert_eq!(found, root),
            other => panic!("expected project root, got {other:?}"),
        }
    }

    #[test]
    fn resolves_config_file_as_project_root() {
        let root = temp_dir("config_project");
        let config = root.join("xluau.config.json");
        write_file(&config, "{}");

        let target =
            resolve_invocation_target(Some(config), Path::new("D:/unused")).expect("target");
        match target {
            InvocationTarget::ProjectRoot(found) => assert_eq!(found, root),
            other => panic!("expected project root, got {other:?}"),
        }
    }

    #[test]
    fn resolves_file_to_nearest_project_root() {
        let root = temp_dir("nearest_project");
        write_file(&root.join("xluau.config.json"), "{}");
        let source = root.join("src/nested/main.xl");
        write_file(&source, "return nil");

        let target =
            resolve_invocation_target(Some(source.clone()), Path::new("D:/unused")).expect("target");
        match target {
            InvocationTarget::SingleFile {
                compiler_root,
                file_path,
            } => {
                assert_eq!(compiler_root, root);
                assert_eq!(file_path, source);
            }
            other => panic!("expected single file, got {other:?}"),
        }
    }

    #[test]
    fn falls_back_to_cwd_when_no_project_config_exists() {
        let cwd = temp_dir("cwd_fallback");
        let file_root = temp_dir("no_project");
        let source = file_root.join("main.xl");
        write_file(&source, "return nil");

        let target =
            resolve_invocation_target(Some(source.clone()), &cwd).expect("target");
        match target {
            InvocationTarget::SingleFile {
                compiler_root,
                file_path,
            } => {
                assert_eq!(compiler_root, cwd);
                assert_eq!(file_path, source);
            }
            other => panic!("expected single file, got {other:?}"),
        }
    }

    #[test]
    fn finds_nearest_project_root() {
        let root = temp_dir("find_root");
        write_file(&root.join("xluau.config.json"), "{}");
        let nested = root.join("src/server/controllers");
        fs::create_dir_all(&nested).expect("nested");

        assert_eq!(nearest_project_root(&nested, Path::new("D:/fallback")), root);
    }
}
