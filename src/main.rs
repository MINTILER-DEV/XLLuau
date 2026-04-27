use std::{path::PathBuf, process::ExitCode};

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
    let compiler = Compiler::discover(std::env::current_dir()?)?;
    match cli.command {
        Command::Build { path } => {
            if let Some(path) = path {
                let artifact = compiler.build_file(&path)?;
                compiler.write_artifact(&artifact)?;
                println!(
                    "built {} -> {}",
                    artifact.input.display(),
                    artifact.output.display()
                );
            } else {
                for artifact in compiler.build_project()? {
                    compiler.write_artifact(&artifact)?;
                    println!(
                        "built {} -> {}",
                        artifact.input.display(),
                        artifact.output.display()
                    );
                }
            }
        }
        Command::Check { path } => {
            if let Some(path) = path {
                let artifact = compiler.build_file(&path)?;
                println!(
                    "checked {} ({}) bytes",
                    artifact.input.display(),
                    artifact.luau.len()
                );
            } else {
                for artifact in compiler.build_project()? {
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
