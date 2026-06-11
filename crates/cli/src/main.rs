use std::path::PathBuf;

use facet::Facet;
use figue::{self as args, FigueBuiltins};
use koffi_bindgen::{
    build_steps::BuildSteps,
    codegen::{copy_runtime, generate_all},
    meta::collect_koffi_deps,
    parser::parse_crate,
};
use tracing::{Level, debug, info};

#[derive(Facet, Debug)]
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
#[derive(Facet, Debug)]
pub enum Command {
    /// Generate bindings.
    #[facet(rename = "gen")]
    Generate(GenerateArgs),

    /// Dump crate IR (for debugging).
    DumpIr(GenerateArgs),
}

#[derive(Facet, Debug)]
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

            let deps = collect_koffi_deps(&crate_manifest)?;
            debug!("Found {} koffi deps", deps.len());

            for dep in &deps {
                info!(
                    "Found koffi dep: {} v{}",
                    dep.package.name, dep.package.version
                );

                let crate_path = dep
                    .package
                    .manifest_path
                    .parent()
                    .map_or_else(|| PathBuf::from("."), |p| p.as_std_path().to_path_buf());
                let crate_name = dep.package.name.to_string();
                let version = dep.package.version.to_string();

                debug!("Parsing crate at {}", crate_path.display());

                let ir = parse_crate(
                    &crate_path,
                    &dep.workspace_root,
                    crate_name,
                    version,
                    &dep.koffi_meta,
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
                let paths = generate_all(&ir, &out_dir, &crate_path, &runtime_path)?;

                if cli.verbose {
                    debug!(
                        "Generated kotlin common at {}",
                        paths.kotlin_common.display()
                    );
                    debug!("Generated kotlin jvm at {}", paths.kotlin_jvm.display());
                    debug!(
                        "Generated kotlin native at {}",
                        paths.kotlin_native.display()
                    );
                    debug!(
                        "Generated kotlin loader at {}",
                        paths.kotlin_loader.display()
                    );
                    debug!(
                        "Generated rust jni glue at {}",
                        paths.rust_jni_glue.display()
                    );
                    debug!(
                        "Generated rust cabi glue at {}",
                        paths.rust_cabi_glue.display()
                    );
                    debug!("Generated c header at {}", paths.c_header.display());
                    debug!("Generated cinterop def at {}", paths.cinterop_def.display());
                    debug!(
                        "Generated glue cargo toml at {}",
                        paths.glue_cargo_toml.display()
                    );
                }

                let kotlin_runtime_path = PathBuf::from("crates/runtime/kotlin");
                debug!(
                    "Copying koffi-runtime from {} to {}",
                    kotlin_runtime_path.display(),
                    out_dir.join("kotlin/runtime").display()
                );
                copy_runtime(&kotlin_runtime_path, &out_dir)?;

                let steps = BuildSteps {
                    crate_path: args.crate_path.clone(),
                    out_dir: out_dir.clone(),
                    glue_path: out_dir.join("rust"),
                    crate_ident: ir.crate_name.replace('-', "_"),
                    lib_name: format!("{}_koffi_glue", ir.crate_name.replace('-', "_")),
                };

                info!("Building JVM artifacts...");
                steps.run_jvm()?;
                // info!("Building native artifacts...");
                // steps.run_native_mingw()?;
                info!("Building Android artifacts...");
                steps.run_android()?;

                info!("Generation complete!");
            }
        }

        Command::DumpIr(args) => {
            let crate_manifest = args.crate_path.join("Cargo.toml");
            let deps = collect_koffi_deps(&crate_manifest)?;

            for dep in &deps {
                let crate_path = dep
                    .package
                    .manifest_path
                    .parent()
                    .map_or_else(|| PathBuf::from("."), |p| p.as_std_path().to_path_buf());
                let crate_name = dep.package.name.to_string();
                let version = dep.package.version.to_string();

                let ir = parse_crate(
                    &crate_path,
                    &dep.workspace_root,
                    crate_name,
                    version,
                    &dep.koffi_meta,
                    &[],
                )?;

                println!("IR for crate {} v{}:", ir.crate_name, ir.version);
                println!("{ir:#?}");
            }
        }
    }

    Ok(())
}
