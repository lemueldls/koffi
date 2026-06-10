use heck::AsSnakeCase;
use koffi_ir::{FFIType, ParamInfo};

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

/// Format a JNI Rust parameter list (jni crate types).
#[must_use]
pub fn rust_params_jni(params: &[ParamInfo], has_receiver: bool, namespace: &str) -> String {
    let mut list = Vec::new();
    if has_receiver {
        list.push("handle_id: jlong".into());
    }

    for p in params {
        list.push(format!(
            "{}: {}",
            rust_prefix_param(&p.name, namespace),
            rust_jni_type(&p.ty)
        ));
    }

    list.join(", ")
}

#[must_use]
pub fn rust_prefix_ident(ident: &str, namespace: &str) -> String {
    let namespace = AsSnakeCase(namespace).to_string();
    let ident = ident.trim_start_matches("r#");

    format!("source_{namespace}_{ident}")
}

#[must_use]
pub fn rust_prefix_param(param: &str, namespace: &str) -> String {
    let namespace = AsSnakeCase(namespace).to_string();
    let param = param.trim_start_matches("r#");

    format!("param_{namespace}_{param}")
}

#[must_use]
pub fn rust_prefix_native(native: &str, namespace: &str) -> String {
    let namespace = AsSnakeCase(namespace).to_string();
    let native = native.trim_start_matches("r#");

    format!("native_{namespace}_{native}")
}

#[must_use]
pub fn rust_call_args(params: &[ParamInfo], namespace: &str) -> String {
    params
        .iter()
        .map(|p| rust_prefix_ident(&p.name, namespace))
        .collect::<Vec<_>>()
        .join(", ")
}

#[must_use]
pub fn rust_c_native_type(ty: &FFIType) -> String {
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
        FFIType::String | FFIType::Bytes | FFIType::Data(_) => "*const u8".into(), /* pointer + length */
        FFIType::Opaque(_) => "u64".into(),                                        // handle ID
        _ => "KoffiByteBuf".into(),
    }
}

#[must_use]
pub fn rust_c_native_return_type(ty: &FFIType) -> String {
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
        FFIType::Opaque(_) => "u64".into(), // handle ID
        _ => "KoffiByteBuf".into(),         // returned by value, caller frees
    }
}
