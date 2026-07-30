use std::path::PathBuf;

use facet::Facet;
use figue::{self as args, FigueBuiltins};
use koffi_build::{OutputDirs, build_and_stage_cabi, build_crate};
use koffi_codegen::{extract::extract_schema, generator};
use tracing::{Level, debug, info};

#[derive(Facet)]
pub struct Cli {
    #[facet(args::subcommand)]
    pub command: Command,

    /// Verbose logging (debug level).
    #[facet(args::named, args::short = 'v')]
    pub verbose: bool,

    /// --help / --version / --completions, for free.
    #[facet(flatten)]
    pub builtins: FigueBuiltins,
}

#[repr(u8)]
#[derive(Facet)]
pub enum Command {
    /// Package artifacts.
    #[facet(rename = "pack")]
    Package(PackageArgs),

    /// Generate bindings.
    #[facet(rename = "gen")]
    Generate(PackageArgs),
}

#[derive(Facet)]
pub struct PackageArgs {
    /// Path to Rust crate root (Cargo.toml directory).
    #[facet(args::positional, default = ".")]
    pub crate_path: PathBuf,

    /// Output directory.
    #[facet(args::named, args::short = 'o', default = "generated")]
    pub out: PathBuf,

    /// Enable release mode.
    #[facet(args::named, args::short = 'r')]
    pub release: bool,

    /// Rerun on Rust source changes.
    #[facet(args::named, args::short = 'w')]
    pub watch: bool,

    /// Treat warnings as errors and abort generation if any are found.
    #[facet(args::named, args::short = 'D')]
    pub deny_warnings: bool,
}

fn main() -> anyhow::Result<()> {
    let cli: Cli = figue::from_std_args().unwrap();

    tracing_subscriber::fmt()
        .with_max_level(if cli.verbose {
            Level::DEBUG
        } else {
            Level::INFO
        })
        .init();

    match cli.command {
        Command::Package(args) => {
            let dirs = generate(&args)?;

            info!("packaging the generated glue crate");
            build_and_stage_cabi(&dirs, args.release)?;
            info!("packaging completed successfully");
        }
        Command::Generate(args) => {
            generate(&args)?;
        }
    }

    Ok(())
}

fn generate(args: &PackageArgs) -> anyhow::Result<OutputDirs> {
    let (crate_name, cdylib_path) = build_crate(&args.crate_path, args.release, &[])?;
    info!("generating code for {crate_name}");

    debug!("extracting schema from {}", cdylib_path.display());
    let schema = extract_schema(crate_name, &cdylib_path)?;

    let crate_path = args.crate_path.clone();
    let rust_out_dir = args.out.join("rust");
    let kotlin_out_dir = args.out.join("kotlin");

    let dirs = OutputDirs {
        crate_path,
        rust_out_dir,
        kotlin_out_dir,
    };

    generator::render_all(&schema, &dirs)?;
    info!("code generation completed successfully");

    Ok(dirs)
}
