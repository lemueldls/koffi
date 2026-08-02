use std::path::Path;

use koffi_core::{FnShapeRef, TypeShapeRef};
use object::Object;

use crate::schema::{Schema, build_schema};

/// A handle to a loaded library that keeps the binary mapped in memory.
pub struct LoadedShapeLibrary {
    lib: libloading::Library,
}

impl LoadedShapeLibrary {
    /// Opens a shared library and keeps it loaded in memory.
    pub fn open(lib_path: &Path) -> anyhow::Result<Self> {
        let lib = unsafe { libloading::Library::new(lib_path)? };

        Ok(Self { lib })
    }

    /// Resolves an exported `TypeShapeRef` symbol from the loaded library.
    ///
    /// # Safety
    ///
    /// The symbol must be a valid `TypeShapeRef` exported by the library.
    pub unsafe fn get_type_shape(&self, symbol_name: &str) -> anyhow::Result<TypeShapeRef> {
        let sym: libloading::Symbol<*const TypeShapeRef> =
            unsafe { self.lib.get(symbol_name.as_bytes()) }?;

        Ok(unsafe { **sym })
    }

    /// Resolves an exported `FnShapeRef` symbol from the loaded library.
    ///
    /// # Safety
    ///
    /// The symbol must be a valid `FnShapeRef` exported by the library.
    pub unsafe fn get_fn_shape(&self, symbol_name: &str) -> anyhow::Result<FnShapeRef> {
        let sym: libloading::Symbol<*const FnShapeRef> =
            unsafe { self.lib.get(symbol_name.as_bytes()) }?;

        Ok(unsafe { **sym })
    }
}

pub fn discover_fn_symbols(library_path: &Path) -> anyhow::Result<Vec<String>> {
    let data = std::fs::read(library_path)?;
    let file = object::File::parse(&*data)?;

    let symbols = file
        .exports()?
        .filter_map(|sym| {
            let name_or_ordinal = sym.expect("failed to get symbol name").into_name();
            let name = std::str::from_utf8(name_or_ordinal.name()?).ok()?;
            let is_koffi_fn = name.starts_with("__KOFFI_FN_") && name.ends_with("_ENTRY");

            is_koffi_fn.then(|| name.to_string())
        })
        .collect();

    Ok(symbols)
}

pub fn extract_schema(crate_name: String, library_path: &Path) -> anyhow::Result<Schema> {
    let lib = LoadedShapeLibrary::open(library_path)?;
    let symbol_names = discover_fn_symbols(library_path)?;

    let fn_entries = symbol_names
        .iter()
        .map(|name| {
            let shape = unsafe { lib.get_fn_shape(name)? };
            Ok(shape)
        })
        .collect::<anyhow::Result<Vec<_>>>()?;

    build_schema(crate_name, &fn_entries)
}
