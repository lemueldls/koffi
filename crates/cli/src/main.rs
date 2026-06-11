use std::path::{Path, PathBuf};

use facet::Facet;
use facet_pretty::{FacetPretty, PrettyPrinter};
use figue::{self as args, FigueBuiltins};
use koffi_bindgen::{
    build_steps::BuildSteps,
    codegen::{copy_runtime, generate_all},
    meta::collect_koffi_packages,
    parser::parse_crate,
};
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
    /// Generate bindings.
    #[facet(rename = "gen")]
    Generate(GenerateArgs),

    /// Dump crate IR (for debugging).
    DumpIr(GenerateArgs),
}

#[derive(Facet)]
pub struct GenerateArgs {
    /// Path to Rust crate root (Cargo.toml directory).
    #[facet(args::positional, default = ".")]
    pub crate_path: PathBuf,

    /// Output directory.
    #[facet(args::named, args::short = 'o', default = "generated")]
    pub out: PathBuf,

    /// Path to the koffi-runtime crate (required when building the glue crate).
    #[facet(args::named, args::short = 'r', default = "crates/runtime")]
    pub runtime_path: PathBuf,

    /// Rerun on Rust source changes.
    #[facet(args::named, args::short = 'w')]
    pub watch: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli: Cli = figue::from_std_args().unwrap();

    tracing_subscriber::fmt()
        .with_max_level(if cli.verbose {
            Level::DEBUG
        } else {
            Level::INFO
        })
        .init();

    match cli.command {
        Command::Generate(args) => {
            debug!("Starting generation...");

            let crate_manifest = args.crate_path.join("Cargo.toml");
            debug!("Using crate manifest at {}", crate_manifest.display());

            let packages = collect_koffi_packages(&crate_manifest)?;
            debug!("Found {} koffi packages", packages.len());

            for pkg in &packages {
                info!("Found koffi package: {} v{}", pkg.name, pkg.version);

                let crate_path = pkg.manifest_path.parent().unwrap_or_else(|| Path::new("."));

                debug!("Parsing crate at {}", crate_path.display());

                let ir = parse_crate(
                    crate_path,
                    &pkg.workspace_root,
                    pkg.name.clone(),
                    pkg.version.clone(),
                    &pkg.koffi_meta,
                    &[],
                )?;

                let out_dir = std::path::absolute(&args.out)?;
                let runtime_path = std::path::absolute(&args.runtime_path)?;

                debug!("Generating bindings to {}", out_dir.display());
                debug!("Using koffi-runtime at {}", runtime_path.display());

                info!(
                    "Generating bindings for crate {} v{}",
                    ir.crate_name, ir.version
                );
                generate_all(&ir, &out_dir, crate_path, &runtime_path)?;

                let kotlin_runtime_path = PathBuf::from("crates/runtime/kotlin");
                debug!(
                    "Copying koffi-runtime from {} to {}",
                    kotlin_runtime_path.display(),
                    out_dir.join("kotlin/runtime").display()
                );
                copy_runtime(&kotlin_runtime_path, &out_dir)?;

                let build = BuildSteps {
                    crate_path: args.crate_path.clone(),
                    out_dir: out_dir.clone(),
                    glue_path: out_dir.join("rust"),
                    crate_ident: ir.crate_name.replace('-', "_"),
                    lib_name: format!("{}_koffi_glue", ir.crate_name.replace('-', "_")),
                };

                info!("Building JVM artifacts...");
                build.run_jvm()?;
                // info!("Building native artifacts...");
                // build.run_native_mingw()?;
                // info!("Building Android artifacts...");
                // build.run_android()?;

                info!("Generation complete!");
            }
        }

        Command::DumpIr(args) => {
            let crate_manifest = args.crate_path.join("Cargo.toml");
            let packages = collect_koffi_packages(&crate_manifest)?;

            for pkg in &packages {
                let crate_path = pkg.manifest_path.parent().unwrap_or_else(|| Path::new("."));

                let ir = parse_crate(
                    crate_path,
                    &pkg.workspace_root,
                    pkg.name.clone(),
                    pkg.version.clone(),
                    &pkg.koffi_meta,
                    &[],
                )?;

                let printer = PrettyPrinter::new()
                    .with_indent_size(4)
                    .with_max_content_len(80);

                info!("IR for crate {} v{}:", ir.crate_name, ir.version);
                println!("{}", ir.pretty_with(printer));
            }
        }
    }

    Ok(())
}
