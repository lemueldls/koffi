use std::{fs, path::PathBuf};

use koffi_build::build_crate;
use koffi_codegen::extract::extract_schema;

#[test]
fn examples() -> anyhow::Result<()> {
    let examples_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../examples");

    let dir = fs::read_dir(&examples_dir).expect("failed to read examples directory");
    for entry in dir {
        let entry = entry.expect("failed to read example directory entry");
        let path = entry.path();

        let (crate_name, cdylib_path) = build_crate(&path, false, &[])?;
        let schema = extract_schema(crate_name.clone(), &cdylib_path)?;

        insta::assert_debug_snapshot!(crate_name, schema);
    }

    Ok(())
}
