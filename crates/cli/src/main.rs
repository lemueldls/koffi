use std::path::{Path, PathBuf};

use facet::Facet;
use facet_pretty::{FacetPretty, PrettyPrinter};
use figue::{self as args, FigueBuiltins};
use koffi_bindgen::{
    build_steps::BuildSteps,
    codegen::{BindingPackage, generate_package_set},
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

    /// Enable release mode.
    #[facet(args::named, args::short = 'r')]
    pub release: bool,

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

            let mut pkg_schemas = Vec::new();
            let mut parsed_packages = Vec::new();

            for pkg in packages {
                info!("Found koffi package: {} v{}", pkg.name, pkg.version);

                let crate_path = pkg.manifest_path.parent().unwrap_or_else(|| Path::new("."));

                debug!("Parsing crate at {}", crate_path.display());

                let ir = parse_crate(
                    crate_path,
                    &pkg.workspace_root,
                    pkg.name.clone(),
                    pkg.version.clone(),
                    &pkg.koffi_meta,
                    &pkg_schemas,
                );

                match ir {
                    Ok(ir) => {
                        info!(
                            "Parsed bindings for crate {} v{}",
                            ir.crate_name, ir.version
                        );

                        pkg_schemas.push(ir.clone());
                        parsed_packages.push((pkg, ir));
                    }
                    Err(e) => {
                        eprintln!("{}", e.diagnostic().render_cli());
                    }
                }
            }

            if parsed_packages.is_empty() {
                return Ok(());
            }

            let root_index = parsed_packages
                .iter()
                .position(|(pkg, _)| pkg.is_root)
                .unwrap_or_else(|| parsed_packages.len().saturating_sub(1));
            let (root_pkg, root_ir) = &parsed_packages[root_index];
            let target_platforms = root_pkg
                .koffi_meta
                .target_platforms
                .clone()
                .unwrap_or_default();
            let out_dir = std::path::absolute(&args.out)?;
            let runtime_path = std::path::absolute("crates/runtime")?;
            let binding_packages = parsed_packages
                .iter()
                .map(|(pkg, ir)| {
                    let crate_path = pkg.manifest_path.parent().unwrap_or_else(|| Path::new("."));
                    BindingPackage { ir, crate_path }
                })
                .collect::<Vec<_>>();

            debug!("Generating bindings to {}", out_dir.display());
            debug!("Using koffi-runtime at {}", runtime_path.display());
            generate_package_set(
                &binding_packages,
                &root_ir.crate_name,
                &root_ir.version,
                &out_dir,
                &target_platforms,
            )?;

            let crate_ident = root_ir.crate_name.replace('-', "_");
            let build = BuildSteps {
                crate_path: root_pkg
                    .manifest_path
                    .parent()
                    .unwrap_or_else(|| Path::new("."))
                    .to_path_buf(),
                out_dir: out_dir.clone(),
                glue_path: out_dir.join("rust"),
                crate_ident: crate_ident.clone(),
                lib_name: format!("{crate_ident}_koffi_glue"),
            };

            info!("Building artifacts for targets: {}", target_platforms);
            build.run_targets(&target_platforms, args.release)?;

            info!("Generation complete!");
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
