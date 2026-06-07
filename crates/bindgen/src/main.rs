use std::{fs, path::PathBuf};

use clap::Parser;

mod codegen;
mod parser;

#[derive(Parser, Debug)]
#[command(
    name = "koffi-bindgen",
    version = "0.1.0",
    about = "Koffi FFI Binding Generator"
)]
pub struct Args {
    #[arg(
        long = "crate",
        help = "Path to Rust crate root (Cargo.toml directory)"
    )]
    pub crate_path: PathBuf,

    #[arg(long, default_value = "./dist", help = "Output directory")]
    pub out: PathBuf,

    #[arg(
        long,
        default_value = "all",
        help = "Limit generation to one target (android|ios|desktop|web|all)"
    )]
    pub target: String,

    #[arg(long, help = "Default Kotlin package prefix")]
    pub package: Option<String>,

    #[arg(long, default_value = "2.1", help = "Kotlin language version")]
    pub kotlin_version: String,

    #[arg(long, help = "Exclude koffi-std integrations")]
    pub no_std: bool,

    #[arg(long, help = "Rerun on Rust source changes")]
    pub watch: bool,

    #[arg(long, help = "Print generated file list")]
    pub verbose: bool,
}

fn main() {
    let args = Args::parse();
    if let Err(e) = run(&args) {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}

fn run(args: &Args) -> Result<(), Box<dyn std::error::Error>> {
    println!("Scanning crate at {}...", args.crate_path.display());

    // Resolve package name from Cargo.toml
    let cargo_toml_path = args.crate_path.join("Cargo.toml");
    let cargo_toml_str = fs::read_to_string(&cargo_toml_path).map_err(|e| {
        format!(
            "Failed to read Cargo.toml at {}: {e}",
            cargo_toml_path.display()
        )
    })?;
    let cargo_toml: toml::Value = toml::from_str(&cargo_toml_str)?;
    let crate_name = cargo_toml
        .get("package")
        .and_then(|p| p.get("name"))
        .and_then(|n| n.as_str())
        .ok_or("Invalid Cargo.toml: missing package name")?;

    // Parse Rust crate AST
    let ir = parser::parse_crate(&args.crate_path, args.package.clone())?;
    println!("Found namespace: {}", ir.namespace);
    println!(
        "Found {} structs, {} enums, {} exported functions",
        ir.structs.len(),
        ir.enums.len(),
        ir.functions.len()
    );

    // Generate Kotlin KMP bindings
    println!("Generating Kotlin bindings into {}...", args.out.display());
    codegen::kotlin::generate_kotlin(&ir, &args.out, crate_name)?;

    // Generate Rust FFI glue
    println!("Generating Rust glue code into {}...", args.out.display());
    codegen::rust::generate_rust(&ir, &args.out, crate_name)?;

    // Generate Cargo.toml and src/lib.rs for the glue crate
    let absolute_crate_path = fs::canonicalize(&args.crate_path)?;
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
        jni = {{ version = \"0.21.1\", optional = true }}\n\
        postcard = \"1.0.8\"\n\
        serde = {{ version = \"1.0.200\", features = [\"derive\"] }}\n\n\
        [features]\n\
        android = [\"jni\"]\n\
        ios = []\n\
        desktop = [\"jni\"]\n",
        crate_name,
        crate_name.replace('-', "_"),
        absolute_crate_path_str,
        absolute_runtime_path_str
    );

    fs::write(args.out.join("rust/Cargo.toml"), glue_cargo_toml)?;

    let glue_lib_rs = "\
        #[cfg(feature = \"android\")]\n\
        pub mod jni_glue;\n\n\
        #[cfg(feature = \"desktop\")]\n\
        pub mod jni_glue;\n\n\
        pub mod cabi_glue;\n";

    fs::write(args.out.join("rust/src/lib.rs"), glue_lib_rs)?;

    println!("Bindings generation completed successfully!");

    Ok(())
}
