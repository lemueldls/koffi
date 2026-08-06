use std::{
    fs,
    path::{Path, PathBuf},
};

use koffi_build::{
    OutputDirs, build_crate,
    config::{AndroidAbi, JvmBackend, KoffiConfig, TargetPlatform},
};
use koffi_codegen::{extract::extract_schema, generator::render_all};

#[test]
fn examples() -> anyhow::Result<()> {
    let examples_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../examples");

    let dir = fs::read_dir(&examples_dir).expect("failed to read examples directory");
    for entry in dir {
        let entry = entry.expect("failed to read example directory entry");
        let path = entry.path();

        let (crate_name, cdylib_path) = build_crate(&path, false, &[])?;
        let schema = extract_schema(&crate_name, &cdylib_path)?;

        insta::assert_debug_snapshot!(crate_name, schema);
    }

    Ok(())
}

/// Renders every example with the three backend configs (ffm/jvm, jni, and
/// native cinterop) and snapshots each emitted file. The variants mirror the
/// smoke-test layouts under `examples/*/tests*`: one crate, three glue
/// pathways. The jni variant keeps android in its platforms so the android
/// templates stay covered; the native variant pairs linuxX64 with jvm (ffm)
/// like the original example config did.
#[test]
fn render() -> anyhow::Result<()> {
    let examples_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../examples");
    let variants: [(&str, KoffiConfig); 3] = [
        ("ffm", KoffiConfig {
            platforms: vec![TargetPlatform::Jvm],
            jvm_backend: JvmBackend::Ffm,
            android_abis: vec![AndroidAbi::Arm64V8a, AndroidAbi::X86_64],
            cross_compile: false,
        }),
        ("jni", KoffiConfig {
            platforms: vec![TargetPlatform::Jvm, TargetPlatform::Android],
            jvm_backend: JvmBackend::Jni,
            android_abis: vec![AndroidAbi::Arm64V8a, AndroidAbi::X86_64],
            cross_compile: false,
        }),
        ("native", KoffiConfig {
            platforms: vec![TargetPlatform::Jvm, TargetPlatform::LinuxX64],
            jvm_backend: JvmBackend::Ffm,
            android_abis: vec![AndroidAbi::Arm64V8a, AndroidAbi::X86_64],
            cross_compile: false,
        }),
    ];

    let mut examples: Vec<PathBuf> = fs::read_dir(&examples_dir)?
        .map(|entry| {
            entry
                .expect("failed to read example directory entry")
                .path()
        })
        .filter(|path| path.join("Cargo.toml").exists())
        .collect();
    examples.sort();

    for path in examples {
        let (crate_name, cdylib_path) = build_crate(&path, false, &[])?;
        let schema = extract_schema(&crate_name, &cdylib_path)?;

        for (variant, config) in &variants {
            // Renders into the workspace target dir: the generated Cargo.toml
            // relativizes the source crate path, which only works under the
            // workspace root.
            let render_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../target/render-snapshots")
                .join(&crate_name)
                .join(variant);
            if render_dir.exists() {
                fs::remove_dir_all(&render_dir)?;
            }
            let dirs = OutputDirs {
                crate_path: path.clone(),
                rust_out_dir: render_dir.join("rust"),
                kotlin_out_dir: render_dir.join("kotlin"),
            };
            render_all(&schema, &dirs, config)?;

            let mut files = Vec::new();
            collect_files(&render_dir, &mut files)?;
            files.sort();
            for file in files {
                let rel = file
                    .strip_prefix(&render_dir)
                    .expect("collected file under render dir")
                    .to_string_lossy()
                    .replace(['/', '.', '@'], "_");
                insta::assert_snapshot!(
                    format!("{crate_name}_{variant}_{rel}"),
                    fs::read_to_string(&file)?
                );
            }
        }
    }

    Ok(())
}

fn collect_files(dir: &Path, out: &mut Vec<PathBuf>) -> anyhow::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            collect_files(&path, out)?;
        } else {
            out.push(path);
        }
    }

    Ok(())
}
