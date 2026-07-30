use crate::schema::{ScalarKind, Schema, SchemaFn, SchemaStruct, SchemaTypeRef};

impl Schema {
    #[must_use]
    pub fn crate_ident(&self) -> String {
        self.crate_name.replace('-', "_")
    }
}

impl SchemaFn {
    /// Absolute path to the real item this entry describes,
    /// `::crate::Type::method` for a method, `::crate::function` for a free
    /// function. The leading `::` is Rust's "start from the crate root"
    /// syntax, deliberate: it keeps generated code correct even if some
    /// local name in the generated crate happens to shadow part of the
    /// path.
    ///
    /// For a method, this is built from `receiver.rust_absolute_path()`, the
    /// receiver *type's own* module path, not `self.module_path` (where the
    /// impl block happens to be written); those can differ.
    #[must_use]
    pub fn rust_absolute_path(&self) -> String {
        if let Some(recv) = &self.receiver {
            format!("{}::{}", recv.rust_absolute_path(), self.rust_name)
        } else {
            let base = self
                .module_path
                .as_deref()
                .map(|path| format!("::{path}"))
                .unwrap_or_default();

            format!("{base}::{}", self.rust_name)
        }
    }

    /// Delegates to `c_abi_symbol` (generator/c.rs). See the comment there:
    /// this used to be a second, independent formula that didn't know about
    /// the receiver, and silently disagreed with `c_abi_symbol` for every
    /// method.
    #[must_use]
    pub fn unique_ident(&self) -> String {
        self.c_abi_symbol()
    }
}

impl SchemaStruct {
    #[must_use]
    pub fn unique_ident(&self) -> String {
        let mod_infix = self.module_path.as_deref().unwrap_or("").replace("::", "_");
        format!("__koffi_struct_{mod_infix}_{}", self.name)
    }

    #[must_use]
    pub fn rust_absolute_path(&self) -> String {
        match &self.module_path {
            Some(module_path) => format!("::{module_path}::{}", self.name),
            None => self.name.clone(),
        }
    }
}

impl SchemaTypeRef {
    /// The Rust type name to use on the *Cabi* side: a struct's generated
    /// shadow type, or the primitive itself for a scalar (Rust's own scalar
    /// names are already valid identifiers and already what facet reports
    /// via `effective_name`, no separate table needed). Built on the same
    /// `abi_ident_infix` (schema.rs) that `c_abi_symbol` uses, so a struct's
    /// shadow type name and the ABI-symbol infix for a method of that struct
    /// can never drift apart the way `unique_ident`/`c_abi_symbol` once did.
    #[must_use]
    pub fn unique_ident(&self) -> String {
        match self {
            SchemaTypeRef::Scalar(k) => k.rust_type_name().to_string(),
            SchemaTypeRef::Struct { .. } => format!("__koffi_struct_{}", self.abi_ident_infix()),
        }
    }

    /// The real, non-generated Rust path: `::crate::module::Name` for a
    /// struct, or the bare primitive name for a scalar.
    #[must_use]
    pub fn rust_absolute_path(&self) -> String {
        match self {
            SchemaTypeRef::Struct { name, module_path } => {
                match module_path {
                    Some(mp) => format!("::{mp}::{name}"),
                    None => name.clone(),
                }
            }
            SchemaTypeRef::Scalar(k) => k.rust_type_name().to_string(),
        }
    }
}

impl ScalarKind {
    #[must_use]
    pub const fn rust_type_name(&self) -> &'static str {
        match self {
            ScalarKind::Bool => "bool",
            ScalarKind::U8 => "u8",
            ScalarKind::U16 => "u16",
            ScalarKind::U32 => "u32",
            ScalarKind::U64 => "u64",
            ScalarKind::I8 => "i8",
            ScalarKind::I16 => "i16",
            ScalarKind::I32 => "i32",
            ScalarKind::I64 => "i64",
            ScalarKind::F32 => "f32",
            ScalarKind::F64 => "f64",
        }
    }
}
