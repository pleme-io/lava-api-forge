//! lava-api-forge CLI.
//!
//! Usage:
//!   lava-api-forge generate --spec <path> --kind openapi|botocore --out <file>

use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};
use lava_api_forge::{run_from_file, SpecKind};

#[derive(Parser)]
#[command(name = "lava-api-forge", about = "Cloud API spec → typed tatara-lisp bindings")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Generate a <service>-api.tlisp from one API spec.
    Generate {
        /// Path to the API spec file.
        #[arg(long)]
        spec: PathBuf,
        /// Spec format.
        #[arg(long, value_enum)]
        kind: SpecKindArg,
        /// Output .tlisp file path.
        #[arg(long)]
        out: PathBuf,
    },
}

#[derive(Clone, Copy, ValueEnum)]
enum SpecKindArg {
    Openapi,
    Botocore,
}

impl From<SpecKindArg> for SpecKind {
    fn from(v: SpecKindArg) -> Self {
        match v {
            SpecKindArg::Openapi => SpecKind::Openapi,
            SpecKindArg::Botocore => SpecKind::Botocore,
        }
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Generate { spec, kind, out } => {
            run_from_file(&spec, &out, kind.into())?;
            eprintln!("wrote {}", out.display());
        }
    }
    Ok(())
}
