use koffi_ir::{EnumInfo, FFIType, FnInfo, ParamInfo, StructInfo};

/// Crate-relative path for a free function's `use` statement.
///
/// `camera::take_photo` -> `"camera::take_photo"`
/// (root) `add_i32`     -> `"add_i32"`
#[must_use]
pub fn rust_fn_use_path(f: &FnInfo) -> String {
    if f.rust_module_path.is_empty() {
        f.rust_name.clone()
    } else {
        format!("{}::{}", f.rust_module_path.join("::"), f.rust_name)
    }
}

/// Crate-relative path for a struct's `use` statement.
#[must_use]
pub fn rust_struct_use_path(s: &StructInfo) -> String {
    if s.rust_module_path.is_empty() {
        s.name.clone()
    } else {
        format!("{}::{}", s.rust_module_path.join("::"), s.name)
    }
}

/// Crate-relative path for an enum's `use` statement.
#[must_use]
pub fn rust_enum_use_path(e: &EnumInfo) -> String {
    if e.rust_module_path.is_empty() {
        e.name.clone()
    } else {
        format!("{}::{}", e.rust_module_path.join("::"), e.name)
    }
}

/// Crate-relative path for a method's parent type `use` statement.
///
/// Uses `parent_rust_module_path` rather than `rust_module_path` so that the
/// type is imported from its *declaration* site, not from the `impl` site.
#[must_use]
pub fn rust_parent_use_path(f: &FnInfo) -> String {
    let parent = f.parent_struct.as_deref().unwrap_or("");
    if f.parent_rust_module_path.is_empty() {
        parent.to_string()
    } else {
        format!("{}::{parent}", f.parent_rust_module_path.join("::"))
    }
}

#[must_use]
pub fn rust_use_path_with_parent(f: &FnInfo) -> String {
    if let Some(parent) = &f.parent_struct {
        let parent_path = if f.parent_rust_module_path.is_empty() {
            parent.to_owned()
        } else {
            format!("{}::{}", f.parent_rust_module_path.join("::"), parent)
        };
        format!("{}::{}", parent_path, f.rust_name)
    } else {
        rust_fn_use_path(f)
    }
}

// Each alias is unique within a generated glue crate. The `__koffi_` prefix
// avoids clashing with any user identifiers. Module path segments are joined
// with `_` and separated from the item name by `__`.

/// Unique alias for a free function import.
///
/// `camera::take_photo`  -> `__koffi_camera__take_photo`
/// (root) `add_i32`      -> `__koffi__add_i32`
#[must_use]
pub fn rust_fn_alias(f: &FnInfo) -> String {
    let raw = f.rust_name.trim_start_matches("r#");
    if f.rust_module_path.is_empty() {
        format!("__koffi__{raw}")
    } else {
        format!("__koffi_{}_{raw}", f.rust_module_path.join("_"))
    }
}

/// Unique alias for a struct import.
#[must_use]
pub fn rust_struct_alias(s: &StructInfo) -> String {
    if s.rust_module_path.is_empty() {
        format!("__koffi__{}", s.name)
    } else {
        format!("__koffi_{}_{}", s.rust_module_path.join("_"), s.name)
    }
}

/// Unique alias for an enum import.
#[must_use]
pub fn rust_enum_alias(e: &EnumInfo) -> String {
    if e.rust_module_path.is_empty() {
        format!("__koffi__{}", e.name)
    } else {
        format!("__koffi_{}_{}", e.rust_module_path.join("_"), e.name)
    }
}

/// Unique alias for a method's parent type, using `parent_rust_module_path`.
#[must_use]
pub fn rust_parent_alias(f: &FnInfo) -> String {
    let parent = f.parent_struct.as_deref().unwrap_or("");
    if f.parent_rust_module_path.is_empty() {
        format!("__koffi__{parent}")
    } else {
        format!("__koffi_{}_{parent}", f.parent_rust_module_path.join("_"))
    }
}

/// Map [`FFIType`] to the Rust JNI type on the glue side.
#[must_use]
pub fn rust_jni_type(ty: &FFIType) -> String {
    match ty {
        FFIType::Bool => "jboolean".into(),
        FFIType::I8 | FFIType::U8 => "jbyte".into(),
        FFIType::I16 | FFIType::U16 => "jshort".into(),
        FFIType::I32 | FFIType::U32 => "jint".into(),
        FFIType::I64 | FFIType::U64 => "jlong".into(),
        FFIType::F32 => "jfloat".into(),
        FFIType::F64 => "jdouble".into(),
        FFIType::Unit => "()".into(),
        FFIType::String => "JString<'a>".into(),
        FFIType::Bytes => "JByteArray<'a>".into(),
        FFIType::Opaque(_) => "jlong".into(),
        _ => "JByteArray<'a>".into(),
    }
}

/// Emit a comma-separated list of local variable names for a function call.
///
/// Each parameter's local variable is named `_p_{param_name}` to avoid Rust
/// keyword conflicts and shadow-related compiler warnings.
#[must_use]
pub fn rust_call_args(params: &[ParamInfo]) -> String {
    params
        .iter()
        .map(|p| format!("_p_{}", p.name.trim_start_matches("r#")))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Emit `_p_{name}: {jni_type}` pairs for a JNI function signature.
///
/// Prepends `handle: jlong` when `has_receiver` is true (instance methods).
#[must_use]
pub fn rust_params_jni(params: &[ParamInfo], has_receiver: bool) -> String {
    let mut list = Vec::new();
    if has_receiver {
        list.push("handle: jlong".to_string());
    }
    for p in params {
        let name = format!("_p_{}", p.name.trim_start_matches("r#"));
        list.push(format!("{name}: {}", rust_jni_type(&p.ty)));
    }
    list.join(", ")
}

#[must_use]
pub fn rust_cabi_type(ty: &FFIType) -> String {
    match ty {
        FFIType::Bool => "bool".into(),
        FFIType::I8 => "i8".into(),
        FFIType::I16 => "i16".into(),
        FFIType::I32 => "i32".into(),
        FFIType::I64 => "i64".into(),
        FFIType::U8 => "u8".into(),
        FFIType::U16 => "u16".into(),
        FFIType::U32 => "u32".into(),
        FFIType::U64 => "u64".into(),
        FFIType::F32 => "f32".into(),
        FFIType::F64 => "f64".into(),
        FFIType::Unit => "()".into(),
        FFIType::String | FFIType::Bytes | FFIType::Data(_) => "*const u8".into(),
        FFIType::Opaque(_) => "u64".into(),
        _ => "KoffiByteBuf".into(),
    }
}

#[must_use]
pub fn rust_cabi_return_type(ty: &FFIType) -> String {
    match ty {
        FFIType::Unit => "()".into(),
        FFIType::Bool => "bool".into(),
        FFIType::I8 => "i8".into(),
        FFIType::I16 => "i16".into(),
        FFIType::I32 => "i32".into(),
        FFIType::I64 => "i64".into(),
        FFIType::U8 => "u8".into(),
        FFIType::U16 => "u16".into(),
        FFIType::U32 => "u32".into(),
        FFIType::U64 => "u64".into(),
        FFIType::F32 => "f32".into(),
        FFIType::F64 => "f64".into(),
        FFIType::Opaque(_) => "u64".into(),
        _ => "KoffiByteBuf".into(),
    }
}
