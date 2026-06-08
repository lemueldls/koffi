use std::{fs, path::PathBuf};

use cargo_metadata::MetadataCommand;
use facet::Facet;
use figue::{self as args, FigueBuiltins};
use koffi_bindgen::{codegen, parser};
use tracing::{Level, debug, info};

#[derive(Facet, Debug)]
pub struct Args {
    #[facet(args::subcommand)]
    pub command: Command,

    /// --help / --version / --completions, for free.
    #[facet(flatten)]
    pub builtins: FigueBuiltins,
}

#[repr(u8)]
#[derive(Facet, Debug)]
pub enum Command {
    /// Generate bindings.
    #[facet(rename = "gen")]
    Generate {
        /// Path to Rust crate root (Cargo.toml directory).
        #[facet(args::positional, default = ".")]
        crate_path: PathBuf,

        /// Output directory.
        #[facet(args::named, args::short = 'o', default = "generated")]
        out: PathBuf,

        /// Rerun on Rust source changes.
        #[facet(args::named, args::short = 'w')]
        watch: bool,

        /// Print generated file list.
        #[facet(args::named, args::short = 'v')]
        verbose: bool,
    },

    /// Package platform artifacts.
    #[facet(rename = "pack")]
    Package {},
}

pub fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli: Args = figue::from_std_args().unwrap();

    match cli.command {
        Command::Generate {
            crate_path,
            out,
            watch,
            verbose,
        } => {
            tracing_subscriber::fmt()
                .with_max_level(if verbose { Level::DEBUG } else { Level::INFO })
                .init();

            debug!("Scanning crate at {}...", crate_path.display());

            // Resolve package name from Cargo.toml
            let cargo_toml_path = crate_path.join("Cargo.toml");

            let mut cmd = MetadataCommand::new();
            cmd.manifest_path(cargo_toml_path);
            let metadata = cmd.exec()?;
            let root_package = metadata.root_package().ok_or("No root package found")?;
            let crate_name = root_package.name.replace('-', "_");

            let koffi_meta = root_package.metadata.get("koffi");
            let namespace = koffi_meta
                .and_then(|m| m.get("namespace"))
                .and_then(|n| n.as_str())
                .unwrap_or("generated");
            debug!("Found namespace: {namespace}");

            // Parse Rust crate AST
            let ir = parser::parse_crate(root_package, namespace.to_owned())?;
            debug!(
                "Found {} structs, {} enums, {} exported functions",
                ir.structs.len(),
                ir.enums.len(),
                ir.functions.len()
            );

            // Generate Kotlin KMP bindings
            info!("Generating Kotlin bindings into {}...", out.display());
            codegen::kotlin::generate_kotlin(&ir, &out, &crate_name)?;

            // Generate Rust FFI glue
            info!("Generating Rust glue code into {}...", out.display());
            codegen::rust::generate_rust(&ir, &out, &crate_name)?;

            // Generate Cargo.toml and src/lib.rs for the glue crate
            let absolute_crate_path = fs::canonicalize(&crate_path)?;
            let absolute_crate_path_str = absolute_crate_path.to_str().unwrap().replace('\\', "/");

            // Try to resolve the runtime path relative to workspace
            let mut runtime_path = PathBuf::from("crates/runtime");
            if runtime_path.exists() {
                runtime_path = fs::canonicalize(runtime_path)?;
            } else {
                // Fallback relative to the current workspace root
                todo!()
            }
            let absolute_runtime_path_str = runtime_path.to_str().unwrap().replace('\\', "/");

            let glue_cargo_toml = format!(
                "[package]\n\
                name = \"{}-koffi-glue\"\n\
                version = \"0.1.0\"\n\
                edition = \"2024\"\n\n\
                [lib]\n\
                crate-type = [\"cdylib\", \"staticlib\"]\n\n\
                [dependencies]\n\
                {} = {{ path = \"{}\" }}\n\
                koffi-runtime = {{ path = \"{}\" }}\n\
                jni = {{ version = \"0.22.4\", optional = true }}\n\
                postcard = \"1.1.3\"\n\
                serde = {{ version = \"1.0\", features = [\"derive\"] }}\n\n\
                [features]\n\
                android = [\"jni\"]\n\
                ios = []\n\
                desktop = [\"jni\"]\n",
                crate_name,
                crate_name.replace('-', "_"),
                absolute_crate_path_str,
                absolute_runtime_path_str
            );

            fs::write(out.join("rust/Cargo.toml"), glue_cargo_toml)?;

            let glue_lib_rs = "\
                #[cfg(feature = \"android\")]\n\
                pub mod jni_glue;\n\n\
                #[cfg(feature = \"desktop\")]\n\
                pub mod jni_glue;\n\n\
                pub mod cabi_glue;\n";

            fs::write(out.join("rust/src/lib.rs"), glue_lib_rs)?;

            debug!("Bindings generation completed successfully!");
        }
        Command::Package { .. } => {}
    }

    Ok(())
}
