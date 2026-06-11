use facet::Facet;

use crate::{TypeRef, types::FFIType};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Facet)]
#[repr(u8)]
pub enum ReceiverType {
    Ref,    // &self
    RefMut, // &mut self
    Owned,  // self (consumes the handle)
}

#[derive(Debug, Clone, PartialEq, Eq, Facet)]
pub struct ParamInfo {
    pub name: String,
    pub ty: FFIType,
}

/// Overrides and annotations parsed from `#[koffi::export(...)]`.
#[derive(Debug, Clone, Default, Facet)]
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

#[derive(Debug, Clone, Facet)]
pub struct FieldInfo {
    pub name: String,
    pub ty: FFIType,
    /// True if this field has `#[serde(skip)]` or similar.
    pub skip_serde: bool,
}

#[derive(Debug, Clone, Facet)]
pub struct StructInfo {
    pub name: String,
    pub is_opaque: bool, // false = Data type
    pub fields: Vec<FieldInfo>,
    /// Kotlin package namespace (dot-separated), resolved at parse time.
    pub namespace: String,
    /// Rust module path from the crate root to the struct's declaring module.
    /// Empty when the struct is declared at the crate root.
    ///
    /// Example: `["camera", "ops"]` for a struct in `my_crate::camera::ops`.
    /// Used to generate the correct `use my_crate::camera::ops::MyStruct` in
    /// glue crates, and to build unique C/JNI symbol prefixes.
    pub rust_module_path: Vec<String>,
    pub doc: Vec<String>,
}

#[derive(Debug, Clone, Facet)]
pub struct EnumVariantInfo {
    pub name: String,
    pub fields: Vec<FieldInfo>,
    pub doc: Vec<String>,
}

#[derive(Debug, Clone, Facet)]
pub struct EnumInfo {
    pub name: String,
    pub variants: Vec<EnumVariantInfo>,
    /// Kotlin package namespace.
    pub namespace: String,
    /// Rust module path from the crate root to the enum's declaring module.
    pub rust_module_path: Vec<String>,
    pub doc: Vec<String>,
}

#[derive(Debug, Clone, Facet)]
pub struct FnInfo {
    /// Kotlin-side name (camelCase, possibly overridden by [`ExportArgs`]).
    pub name: String,
    /// Exact Rust identifier (may include `r#` raw prefix).
    pub rust_name: String,
    pub is_async: bool,
    pub params: Vec<ParamInfo>,
    pub ret_ty: FFIType,
    pub receiver: Option<ReceiverType>,
    /// If `Some`, this is a method on the named struct.
    pub parent_struct: Option<String>,
    /// Kotlin package namespace.
    pub namespace: String,
    /// Rust module path from the crate root to the function's containing module.
    /// Empty when the function is at the crate root.
    ///
    /// For a free function `my_crate::camera::take_photo`, this is `["camera"]`.
    /// For a method `my_crate::camera::Camera::open`, this is `["camera"]`.
    pub rust_module_path: Vec<String>,
    /// For methods: the Rust module path of the parent struct's declaration.
    /// Usually identical to `rust_module_path` but may differ when `impl`
    /// blocks and type declarations are in separate files.
    ///
    /// Used to generate the correct `use my_crate::{path}::{ParentStruct}`
    /// import in the glue crate.
    pub parent_rust_module_path: Vec<String>,
    pub doc: Vec<String>,
    pub args: ExportArgs,
}

/// The complete exported interface of a single crate, ready for codegen.
/// This is also the schema serialized to `koffi/schema.json`.
#[derive(Debug, Clone, Facet)]
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

    /// Returns all free functions (not methods on any struct).
    pub fn free_functions(&self) -> impl Iterator<Item = &FnInfo> {
        self.functions.iter().filter(|f| f.parent_struct.is_none())
    }

    /// True if the named type is defined as opaque in this interface.
    #[must_use]
    pub fn is_opaque(&self, name: &str) -> bool {
        self.structs.iter().any(|s| s.name == name && s.is_opaque)
    }

    /// Look up a struct by name.
    #[must_use]
    pub fn struct_info(&self, name: &str) -> Option<&StructInfo> {
        self.structs.iter().find(|s| s.name == name)
    }
}
