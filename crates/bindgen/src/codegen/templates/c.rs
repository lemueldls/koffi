use koffi_ir::{FFIType, FnInfo};

/// Generate the C-ABI export symbol for a function.
///
/// Convention (globally unique across all koffi plugins):
/// - Free function in `camera` module: `{crate}_camera_{name}`
/// - Method on `Camera` in `camera` module: `{crate}_camera_Camera_{name}`
/// - Root-level free function: `{crate}_{name}`
/// - Root-level method: `{crate}_{TypeName}_{name}`
///
/// The crate identifier prefix ensures no two plugins can emit the same C
/// symbol even if they export identically-named functions.
#[must_use]
pub fn c_abi_symbol(f: &FnInfo, crate_ident: &str) -> String {
    let rust_name = f.rust_name.trim_start_matches("r#");

    let mod_infix = if f.rust_module_path.is_empty() {
        String::new()
    } else {
        format!("_{}", f.rust_module_path.join("_"))
    };

    match &f.parent_struct {
        Some(parent) => format!("{crate_ident}{mod_infix}_{parent}_{rust_name}"),
        None => format!("{crate_ident}{mod_infix}_{rust_name}"),
    }
}

/// Map [`FFIType`] to its C parameter type for the iOS cinterop header.
///
/// Strings and byte slices arrive as `(*const uint8_t, size_t)` pairs; the
/// header template emits the companion `_len` parameter via `needs_len_param`.
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
        FFIType::String => "const uint8_t*".into(),
        FFIType::Bytes => "const uint8_t*".into(),
        FFIType::Opaque(_) => "uint64_t".into(),
        FFIType::Data(_) => "const uint8_t*".into(),
        _ => "KoffiByteBuf".into(),
    }
}

/// Map [`FFIType`] to its C return type.
///
/// Differs from `c_type` (used for parameters): `String` and `Bytes` become
/// `KoffiByteBuf` (returned by value, caller frees), not a pointer.
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
        FFIType::String => "KoffiByteBuf".into(),
        FFIType::Bytes => "KoffiByteBuf".into(),
        FFIType::Opaque(_) => "uint64_t".into(),
        _ => "KoffiByteBuf".into(),
    }
}
