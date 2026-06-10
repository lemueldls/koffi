use heck::{AsLowerCamelCase, AsPascalCase, AsSnakeCase};
use koffi_ir::{FFIType, FnInfo, ParamInfo};

pub fn camel_case(s: &str) -> String {
    AsLowerCamelCase(s).to_string()
}

pub fn pascal_case(s: &str) -> String {
    AsPascalCase(s).to_string()
}

pub fn snake_case(s: &str) -> String {
    AsSnakeCase(s).to_string()
}

/// Emit the schema hash as a Rust `u64` hex literal.
///
/// Used in generated code to embed the hash into `serialize_envelope` calls.
///
/// ```rust
/// serialize_envelope(&val, 0xa3f9b2c1d4e50167_u64)
/// ```
pub fn schema_hash_hex(ty: &FFIType) -> String {
    let hash = match ty {
        FFIType::Data(r) | FFIType::Opaque(r) => r.schema_hash,
        _ => 0,
    };

    format!("0x{hash:016x}_u64")
}

pub fn needs_buf_read(ty: &FFIType) -> bool {
    matches!(ty, FFIType::String | FFIType::Bytes | FFIType::Data(_))
}
