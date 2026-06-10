use koffi_ir::{FFIType, FnInfo};

/// Map [`FFIType`] to its C-ABI type for the iOS cinterop header.
#[must_use]
pub fn c_type(ty: &FFIType) -> String {
    match ty {
        FFIType::Bool => "bool".into(),
        FFIType::I8 => "int8_t".into(),
        FFIType::I16 => "int16_t".into(),
        FFIType::I32 => "int32_t".into(),
        FFIType::I64 => "int64_t".into(),
        FFIType::U8 => "uint8_t".into(),
        FFIType::U16 => "uint16_t".into(),
        FFIType::U32 => "uint32_t".into(),
        FFIType::U64 => "uint64_t".into(),
        FFIType::F32 => "float".into(),
        FFIType::F64 => "double".into(),
        FFIType::Unit => "void".into(),
        FFIType::String => "*const char".into(), // CString
        FFIType::Bytes => "*const uint8_t".into(),
        FFIType::Opaque(_) => "uint64_t".into(), // handle ID
        _ => "KoffiByteBuf".into(),
    }
}

/// Generate the C-ABI export symbol for a function.
///
/// Convention: `{crate_ident}_{parent}_{rust_name}` for methods,
/// `{crate_ident}_{rust_name}` for free functions. This is the name used in
/// both the generated `.h` header and `cabi_glue.rs`.
#[must_use]
pub fn c_abi_symbol(f: &FnInfo, crate_ident: &str) -> String {
    let name = f.rust_name.trim_start_matches("r#");

    match &f.parent_struct {
        Some(parent) => format!("{crate_ident}_{parent}_{name}"),
        None => format!("{crate_ident}_{name}"),
    }
}

/// Map [`FFIType`] to its C return type.
///
/// Differs from `c_type` (used for parameters) in that `String` and `Bytes`
/// become `KoffiByteBuf` (returned by value, caller frees), not a pointer.
#[must_use]
pub fn c_return_type(ty: &FFIType) -> String {
    match ty {
        FFIType::Unit => "void".into(),
        FFIType::Bool => "bool".into(),
        FFIType::I8 => "int8_t".into(),
        FFIType::I16 => "int16_t".into(),
        FFIType::I32 => "int32_t".into(),
        FFIType::I64 => "int64_t".into(),
        FFIType::U8 => "uint8_t".into(),
        FFIType::U16 => "uint16_t".into(),
        FFIType::U32 => "uint32_t".into(),
        FFIType::U64 => "uint64_t".into(),
        FFIType::F32 => "float".into(),
        FFIType::F64 => "double".into(),
        FFIType::String => "KoffiByteBuf".into(), // UTF-8 bytes, no null term
        FFIType::Bytes => "KoffiByteBuf".into(),
        FFIType::Opaque(_) => "uint64_t".into(), // handle ID
        _ => "KoffiByteBuf".into(),              // postcard-serialized Data
    }
}
