use heck::{AsLowerCamelCase, AsPascalCase, AsSnakeCase};
use koffi_ir::{FFIType, ParamInfo};

/// Map [`FFIType`] to its Kotlin commonMain representation.
pub fn kotlin_type(ty: &FFIType) -> askama::Result<String> {
    Ok(match ty {
        FFIType::Bool => "Boolean".into(),
        FFIType::I8 => "Byte".into(),
        FFIType::I16 => "Short".into(),
        FFIType::I32 => "Int".into(),
        FFIType::I64 => "Long".into(),
        FFIType::U8 => "UByte".into(),
        FFIType::U16 => "UShort".into(),
        FFIType::U32 => "UInt".into(),
        FFIType::U64 => "ULong".into(),
        FFIType::F32 => "Float".into(),
        FFIType::F64 => "Double".into(),
        FFIType::Unit => "Unit".into(),
        FFIType::String => "String".into(),
        FFIType::Bytes => "ByteArray".into(),
        FFIType::Option(inner) => format!("{}?", kotlin_type(inner)?),
        FFIType::Result(ok, err) => {
            format!(
                "rs.koffi.KoffiResult<{}, {}>",
                kotlin_type(ok)?,
                kotlin_type(err)?
            )
        }
        FFIType::Vec(inner) => format!("List<{}>", kotlin_type(inner)?),
        FFIType::Map(k, v) => format!("Map<{}, {}>", kotlin_type(k)?, kotlin_type(v)?),
        FFIType::Set(inner) => format!("Set<{}>", kotlin_type(inner)?),
        FFIType::Opaque(r) | FFIType::Data(r) => r.name.clone(),
    })
}

/// Map [`FFIType`] to its JNI wire type (what crosses the JNI boundary).
/// Complex types become `ByteArray` (postcard-serialized).
/// Opaque types become Long (handle ID).
pub fn jni_type(ty: &FFIType) -> askama::Result<String> {
    Ok(match ty {
        FFIType::Bool => "Boolean".into(),
        FFIType::I8 | FFIType::U8 => "Byte".into(),
        FFIType::I16 | FFIType::U16 => "Short".into(),
        FFIType::I32 | FFIType::U32 => "Int".into(),
        FFIType::I64 | FFIType::U64 => "Long".into(),
        FFIType::F32 => "Float".into(),
        FFIType::F64 => "Double".into(),
        FFIType::Unit => "Unit".into(),
        FFIType::String => "String".into(),
        FFIType::Bytes => "ByteArray".into(),
        FFIType::Opaque(_) => "Long".into(),
        _ => "ByteArray".into(), // postcard-serialized
    })
}

/// Map [`FFIType`] to the Rust JNI type on the glue side.
pub fn jni_rust_type(ty: &FFIType) -> askama::Result<String> {
    Ok(match ty {
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
    })
}

/// Map [`FFIType`] to its C-ABI type for the iOS cinterop header.
pub fn c_type(ty: &FFIType) -> askama::Result<String> {
    Ok(match ty {
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
    })
}

pub fn camel_case(s: &str) -> askama::Result<String> {
    Ok(AsLowerCamelCase(s).to_string())
}

pub fn pascal_case(s: &str) -> askama::Result<String> {
    Ok(AsPascalCase(s).to_string())
}

pub fn snake_case(s: &str) -> askama::Result<String> {
    Ok(AsSnakeCase(s).to_string())
}

pub fn jni_symbol(namespace: &str, pkg_pascal: &str, method: &str) -> askama::Result<String> {
    // JNI symbol: Java_{pkg_underscored}_{ClassName}_{methodName}
    // Underscores in identifiers must be escaped as _1
    let pkg = namespace.replace('.', "_");
    let method_escaped = method.replace('_', "_1");

    Ok(format!("Java_{pkg}_{pkg_pascal}Jni_{method_escaped}"))
}

/// Format a parameter list for commonMain (Kotlin types).
pub fn params_common(params: &[ParamInfo]) -> askama::Result<String> {
    Ok(params
        .iter()
        .map(|p| {
            format!(
                "{}: {}",
                AsLowerCamelCase(&p.name),
                kotlin_type(&p.ty).unwrap_or_default()
            )
        })
        .collect::<Vec<_>>()
        .join(", "))
}

/// Format a JNI parameter list (wire types), optionally prepending handleId.
pub fn params_jni(params: &[ParamInfo]) -> askama::Result<String> {
    let args: Vec<String> = params
        .iter()
        .map(|p| {
            format!(
                "{}: {}",
                AsLowerCamelCase(&p.name),
                jni_type(&p.ty).unwrap_or_default()
            )
        })
        .collect();

    Ok(args.join(", "))
}

/// Format a JNI Rust parameter list (jni crate types).
pub fn params_jni_rust(params: &[ParamInfo], has_receiver: bool) -> askama::Result<String> {
    let mut list = Vec::new();
    if has_receiver {
        list.push("handle_id: jlong".into());
    }

    for p in params {
        list.push(format!(
            "{}: {}",
            p.name,
            jni_rust_type(&p.ty).unwrap_or_default()
        ));
    }

    Ok(list.join(", "))
}

pub const fn async_kw(is_async: &bool) -> askama::Result<&'static str> {
    Ok(if *is_async { "suspend " } else { "" })
}

/// Generate the JNI method name used in the Kotlin JNI object.
pub fn jni_method_name(parent: &Option<String>, rust_name: &str) -> askama::Result<String> {
    Ok(match parent {
        Some(s) => format!("koffi_struct_{s}_{rust_name}"),
        None => format!("koffi_fn_{rust_name}"),
    })
}

/// Emit the JNI call expression for a function return, including
/// postcard deserialization for non-blittable return types.
pub fn jni_return_expr(
    pkg: &str,
    jni_name: &str,
    params: &[ParamInfo],
    ret: &FFIType,
    has_receiver: bool,
) -> askama::Result<String> {
    let mut args = Vec::new();
    if has_receiver {
        args.push("handleId".into());
    }

    for p in params {
        let name = AsLowerCamelCase(&p.name).to_string();
        if p.ty.is_blittable() || p.ty == FFIType::String || p.ty == FFIType::Bytes {
            args.push(name);
        } else {
            args.push(format!("rs.koffi.serialize({name})"));
        }
    }

    let call = if has_receiver {
        format!("{pkg}Jni.{jni_name}({})", args.join(", "))
    } else {
        format!("{pkg}Jni.koffi_fn_{jni_name}({})", args.join(", "))
    };

    Ok(
        if ret.is_blittable()
            || *ret == FFIType::Unit
            || *ret == FFIType::String
            || *ret == FFIType::Bytes
        {
            format!("return {call}")
        } else {
            format!("return rs.koffi.deserialize({call})")
        },
    )
}

#[must_use]
pub fn jni_rust_return_conversion(ty: &FFIType) -> String {
    match ty {
        FFIType::Bool => "val as jboolean".to_string(),
        FFIType::I8 | FFIType::U8 => "val as jbyte".to_string(),
        FFIType::I16 | FFIType::U16 => "val as jshort".to_string(),
        FFIType::I32 | FFIType::U32 => "val as jint".to_string(),
        FFIType::I64 | FFIType::U64 => "val as jlong".to_string(),
        FFIType::F32 => "val as jfloat".to_string(),
        FFIType::F64 => "val as jdouble".to_string(),
        FFIType::Unit => "()".to_string(),
        FFIType::String => "env.new_string(val).expect(\"Cannot create JVM string\")".to_string(),
        FFIType::Bytes => {
            "env.byte_array_from_slice(&val).expect(\"Cannot create JVM byte array\")".to_string()
        }
        FFIType::Opaque(..) => {
            "let handle = koffi_runtime::HandleRegistry::global().insert(val);\n\
            handle as jlong"
                .to_string()
        }
        _ => {
            // Serialized type
            "let bytes = postcard::to_allocvec(&val).expect(\"Postcard serialization failed\");\n\
            env.byte_array_from_slice(&bytes).expect(\"Cannot create JVM byte array\")"
                .to_string()
        }
    }
}

#[must_use]
pub fn jni_rust_default_return(ty: &FFIType) -> String {
    match ty {
        FFIType::Bool => "0".to_string(),
        FFIType::I8 | FFIType::U8 => "0".to_string(),
        FFIType::I16 | FFIType::U16 => "0".to_string(),
        FFIType::I32 | FFIType::U32 => "0".to_string(),
        FFIType::I64 | FFIType::U64 => "0".to_string(),
        FFIType::F32 => "0.0".to_string(),
        FFIType::F64 => "0.0".to_string(),
        FFIType::Unit => "()".to_string(),
        FFIType::Opaque(_) => "0".to_string(),
        _ => "std::ptr::null_mut()".to_string(),
    }
}

#[must_use]
pub fn call_args(params: &[ParamInfo]) -> String {
    params
        .iter()
        .map(|p| format!("r#{}", AsSnakeCase(&p.name)))
        .collect::<Vec<_>>()
        .join(", ")
}
