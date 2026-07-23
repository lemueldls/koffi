use std::{
    error::Error,
    path::{Path, PathBuf},
};

use facet::Facet;
use facet_pretty::{FacetPretty, PrettyPrinter};
use figue::{self as args, FigueBuiltins};
use koffi_bindgen::{
    DiagnosticSink,
    build_steps::{BuildSteps, TargetPlatforms},
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
    /// Package artifacts.
    #[facet(rename = "pack")]
    Package(PackageArgs),

    /// Generate bindings.
    #[facet(rename = "gen")]
    Generate(PackageArgs),

    /// Dump crate IR (for debugging).
    DumpIr(PackageArgs),
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

fn main() -> Result<(), Box<dyn Error>> {
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
            package(args)?;
        }
        Command::Generate(args) => {
            generate(args)?;
        }
        Command::DumpIr(args) => {
            let crate_manifest = args.crate_path.join("Cargo.toml");
            let packages = collect_koffi_packages(&crate_manifest)?;

            for pkg in &packages {
                let crate_path = pkg.manifest_path.parent().unwrap_or_else(|| Path::new("."));

                match parse_crate(
                    crate_path,
                    &pkg.workspace_root,
                    pkg.name.clone(),
                    pkg.version.clone(),
                    &pkg.koffi_meta,
                    &[],
                ) {
                    Ok((ir, sink)) => {
                        // Emit diagnostics even in dump-ir mode so the user
                        // can see what was skipped or warned about.
                        if !sink.is_empty() {
                            sink.emit();
                            eprintln!();
                        }

                        let printer = PrettyPrinter::new()
                            .with_indent_size(4)
                            .with_max_content_len(80);

                        info!("IR for crate {} v{}:", ir.crate_name, ir.version);
                        println!("{}", ir.pretty_with(printer));
                    }
                    Err(e) => {
                        eprintln!("{}", e.diagnostic().render_cli());
                    }
                }
            }
        }
    }

    Ok(())
}

fn package(args: PackageArgs) -> Result<(), Box<dyn Error>> {
    let pkg_gen = generate(args)?;
    package_generation(pkg_gen)
}

fn generate(args: PackageArgs) -> Result<PackageGeneration, Box<dyn Error>> {
    debug!("Starting generation...");

    let crate_manifest = args.crate_path.join("Cargo.toml");
    debug!("Using crate manifest at {}", crate_manifest.display());

    let packages = collect_koffi_packages(&crate_manifest)?;
    debug!("Found {} koffi packages", packages.len());

    let mut pkg_schemas = Vec::new();
    let mut parsed_packages = Vec::new();
    // Collect diagnostics from all packages before deciding to abort.
    let mut all_diagnostics = DiagnosticSink::new();
    let mut had_errors = false;

    for pkg in packages {
        info!("Found koffi package: {} v{}", pkg.name, pkg.version);

        let crate_path = pkg.manifest_path.parent().unwrap_or_else(|| Path::new("."));

        debug!("Parsing crate at {}", crate_path.display());

        match parse_crate(
            crate_path,
            &pkg.workspace_root,
            pkg.name.clone(),
            pkg.version.clone(),
            &pkg.koffi_meta,
            &pkg_schemas,
        ) {
            Ok((ir, sink)) => {
                info!(
                    "Parsed bindings for crate {} v{}: {} structs, {} enums, {} functions",
                    ir.crate_name,
                    ir.version,
                    ir.structs.len(),
                    ir.enums.len(),
                    ir.functions.len(),
                );

                if sink.has_errors() {
                    had_errors = true;
                }
                if args.deny_warnings && sink.has_warnings() {
                    had_errors = true;
                }
                all_diagnostics.extend(sink);

                pkg_schemas.push(ir.clone());
                parsed_packages.push((pkg, ir));
            }
            Err(e) => {
                // Fatal error (I/O, rustdoc crash, etc.)

                // emit all prior diagnostics first so the user sees everything.
                all_diagnostics.emit();
                eprintln!("{}", e.diagnostic().render_cli());
                had_errors = true;
            }
        }
    }

    // Always emit all collected diagnostics before proceeding.
    all_diagnostics.emit();

    if had_errors {
        let flag = if args.deny_warnings {
            " (--deny-warnings is set)"
        } else {
            ""
        };
        eprintln!("\nkoffi: aborting generation due to errors{flag}");
        std::process::exit(1);
    }

    if parsed_packages.is_empty() {
        return Err(Box::from(
            "No koffi packages parsed successfully; nothing to generate.",
        ));
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
    let binding_packages = parsed_packages
        .iter()
        .map(|(pkg, ir)| {
            let crate_path = pkg.manifest_path.parent().unwrap_or_else(|| Path::new("."));
            let is_root = pkg.is_root;

            BindingPackage {
                ir,
                crate_path,
                is_root,
            }
        })
        .collect::<Vec<_>>();

    let crate_name = root_ir.crate_name.clone();
    let manifest_path = root_pkg.manifest_path.clone();

    debug!("Generating bindings to {}", out_dir.display());
    generate_package_set(
        &binding_packages,
        &crate_name,
        &root_ir.version,
        &out_dir,
        &target_platforms,
    )?;

    info!("Generation complete!");

    Ok(PackageGeneration {
        args,
        crate_name,
        manifest_path,
        target_platforms,
        out_dir,
    })
}

pub struct PackageGeneration {
    pub args: PackageArgs,
    pub crate_name: String,
    pub manifest_path: PathBuf,
    pub target_platforms: TargetPlatforms,
    pub out_dir: PathBuf,
}

fn package_generation(pkg_gen: PackageGeneration) -> Result<(), Box<dyn Error>> {
    let crate_ident = pkg_gen.crate_name.replace('-', "_");
    let steps = BuildSteps {
        crate_path: pkg_gen
            .manifest_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf(),
        out_dir: pkg_gen.out_dir.clone(),
        glue_path: pkg_gen.out_dir.join("rust"),
        crate_ident: crate_ident.clone(),
        lib_name: format!("{crate_ident}_koffi_glue"),
    };

    info!(
        "Building artifacts for targets: {}",
        pkg_gen.target_platforms
    );
    steps.build_targets(&pkg_gen.target_platforms, pkg_gen.args.release)?;

    info!("Artifacts built successfully!");

    Ok(())
}
