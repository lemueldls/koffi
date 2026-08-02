use heck::ToSnakeCase;

use crate::schema::{
    ScalarKind, Schema, SchemaEnum, SchemaEnumVariant, SchemaFn, SchemaStruct, SchemaTypeRef,
};

impl Schema {
    #[must_use]
    pub fn crate_ident(&self) -> String {
        self.crate_name.replace('-', "_")
    }
}

impl SchemaFn {
    /// Absolute path to the real item this entry describes:
    /// `::crate::Type::method` for anything inside an `impl` block (method
    /// or receiver-less associated fn alike, keyed off `parent`, not off
    /// whether it takes a receiver), `::crate::function` for a free
    /// function. The leading `::` keeps generated code correct even if a
    /// local name shadows part of the path.
    ///
    /// With a `parent`, this is built from `parent.rust_absolute_path()`,
    /// the parent type's module path, not `self.module_path` (where the
    /// impl block happens to be written); those can differ.
    #[must_use]
    pub fn rust_absolute_path(&self) -> String {
        if let Some(parent) = &self.parent {
            format!("{}::{}", parent.rust_absolute_path(), self.rust_name)
        } else {
            let base = self
                .module_path
                .as_deref()
                .map(|path| format!("::{path}"))
                .unwrap_or_default();

            format!("{base}::{}", self.rust_name)
        }
    }

    /// Delegates to `c_abi_symbol` (generator/c.rs): a second, independent
    /// formula here used to silently disagree with it for every method.
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

impl SchemaEnum {
    #[must_use]
    pub fn unique_ident(&self) -> String {
        let mod_infix = self.module_path.as_deref().unwrap_or("").replace("::", "_");
        format!("__koffi_enum_{mod_infix}_{}", self.name)
    }

    #[must_use]
    pub fn rust_absolute_path(&self) -> String {
        match &self.module_path {
            Some(module_path) => format!("::{module_path}::{}", self.name),
            None => self.name.clone(),
        }
    }

    /// `Variant` -> `variant`: the wire union member / payload-struct
    /// suffix for a variant (Rust-style `snake_case`).
    #[must_use]
    pub fn variant_wire_name(&self, v: &SchemaEnumVariant) -> String {
        v.name.to_snake_case()
    }

    /// Wire payload struct for one variant: `__koffi_enum_Status_busy`.
    #[must_use]
    pub fn payload_ident(&self, v: &SchemaEnumVariant) -> String {
        format!("{}_{}", self.unique_ident(), self.variant_wire_name(v))
    }

    /// The wire union over every variant's payload struct, for
    /// data-carrying enums.
    #[must_use]
    pub fn payload_union_ident(&self) -> String {
        format!("{}_payload", self.unique_ident())
    }
}

impl SchemaTypeRef {
    #[must_use]
    pub fn unique_ident(&self) -> String {
        match self {
            SchemaTypeRef::Scalar(k) => k.rust_type_name().to_string(),
            SchemaTypeRef::Struct { .. } => format!("__koffi_struct_{}", self.abi_ident_infix()),
            SchemaTypeRef::Enum { .. } => format!("__koffi_enum_{}", self.abi_ident_infix()),
        }
    }

    #[must_use]
    pub fn rust_absolute_path(&self) -> String {
        match self {
            SchemaTypeRef::Struct { name, module_path } | SchemaTypeRef::Enum { name, module_path, .. } => {
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

    /// A Rust literal for an enum discriminant of this kind, as stored on
    /// the wire. Unsigned kinds wrap negative values to their bit pattern
    /// (`#[repr(C)]` enum `Error = -1` is stored as `0xFFFF_FFFF`).
    // The narrowing casts are the point: wrapping to the wire width is
    // exactly the bit-pattern semantics of the enum's repr.
    #[must_use]
    #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation, clippy::cast_lossless)]
    pub fn wire_discriminant_literal(&self, value: &i64) -> String {
        let value = *value;
        match self {
            ScalarKind::U8 => (value as u8 as u64).to_string(),
            ScalarKind::U16 => (value as u16 as u64).to_string(),
            ScalarKind::U32 => (value as u32 as u64).to_string(),
            ScalarKind::U64 => (value as u64).to_string(),
            _ => value.to_string(),
        }
    }
}
