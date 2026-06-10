use serde::{Deserialize, Serialize};

use crate::{TypeRef, types::FFIType};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReceiverType {
    Ref,    // &self
    RefMut, // &mut self
    Owned,  // self (consumes the handle)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParamInfo {
    pub name: String,
    pub ty: FFIType,
}

/// Overrides and annotations parsed from `#[koffi::export(...)]`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExportArgs {
    /// Override the generated Kotlin function/method name.
    pub name: Option<String>,
    /// Override the Kotlin package for this item only.
    pub package: Option<String>,
    /// Emit `@Deprecated("...")` on the Kotlin declaration.
    pub deprecated: Option<String>,
    /// Reserved for future: run on Dispatchers.IO.
    pub blocking: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldInfo {
    pub name: String,
    pub ty: FFIType,
    /// True if this field has `#[serde(skip)]` or similar.
    pub skip_serde: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructInfo {
    pub name: String,
    pub is_opaque: bool, // false = Data type
    pub fields: Vec<FieldInfo>,
    pub namespace: String, // resolved at parse time
    pub doc: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnumVariantInfo {
    pub name: String,
    pub fields: Vec<FieldInfo>,
    pub doc: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnumInfo {
    pub name: String,
    pub variants: Vec<EnumVariantInfo>,
    pub namespace: String,
    pub doc: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FnInfo {
    /// Kotlin-side name (camelCase, possibly overridden by [`ExportArgs`]).
    pub kotlin_name: String,
    /// Exact Rust identifier.
    pub rust_name: String,
    pub is_async: bool,
    pub params: Vec<ParamInfo>,
    pub ret_ty: FFIType,
    pub receiver: Option<ReceiverType>,
    /// If `Some`, this is a method on the named struct.
    pub parent_struct: Option<String>,
    pub namespace: String,
    pub doc: Option<Vec<String>>,
    pub args: ExportArgs,
}

/// The complete exported interface of a single crate, ready for codegen.
/// This is also the schema serialized to `koffi/schema.json`.
#[derive(Debug, Serialize, Deserialize)]
pub struct CrateInterface {
    /// Default Kotlin package namespace for all items in this crate.
    pub namespace: String,
    pub crate_name: String,
    pub version: String,
    pub structs: Vec<StructInfo>,
    pub enums: Vec<EnumInfo>,
    pub functions: Vec<FnInfo>,
    /// [`TypeRef`]s imported from other crates that appear in this interface.
    /// The codegen uses these to emit `import` statements and avoid
    /// re-declaring types owned by dependency crates.
    pub imports: Vec<TypeRef>,
}

impl CrateInterface {
    /// Returns all functions that belong to the given struct.
    pub fn methods_of<'a>(&'a self, struct_name: &str) -> impl Iterator<Item = &'a FnInfo> {
        self.functions
            .iter()
            .filter(move |f| f.parent_struct.as_deref() == Some(struct_name))
    }

    /// Returns all free functions (not methods).
    pub fn free_functions(&self) -> impl Iterator<Item = &FnInfo> {
        self.functions.iter().filter(|f| f.parent_struct.is_none())
    }

    /// True if the named type is defined as opaque in this interface.
    #[must_use]
    pub fn is_opaque(&self, name: &str) -> bool {
        self.structs.iter().any(|s| s.name == name && s.is_opaque)
    }
}
