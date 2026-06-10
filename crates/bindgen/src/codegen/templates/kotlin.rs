use heck::{AsLowerCamelCase, AsPascalCase, AsSnakeCase};
use koffi_ir::{FFIType, FnInfo, ParamInfo};

use crate::codegen::templates::util::needs_buf_read;

/// Map [`FFIType`] to its Kotlin commonMain representation.
pub fn kotlin_type(ty: &FFIType) -> String {
    match ty {
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
        FFIType::Option(inner) => format!("{}?", kotlin_type(inner)),
        FFIType::Result(ok, err) => {
            format!(
                "rs.koffi.KoffiResult<{}, {}>",
                kotlin_type(ok),
                kotlin_type(err)
            )
        }
        FFIType::Vec(inner) => format!("List<{}>", kotlin_type(inner)),
        FFIType::Map(k, v) => format!("Map<{}, {}>", kotlin_type(k), kotlin_type(v)),
        FFIType::Set(inner) => format!("Set<{}>", kotlin_type(inner)),
        FFIType::Opaque(r) | FFIType::Data(r) => r.name.clone(),
    }
}

/// Map [`FFIType`] to its JNI wire type (what crosses the JNI boundary).
/// Complex types become `ByteArray` (postcard-serialized).
/// Opaque types become Long (handle ID).
pub fn kotlin_jni_type(ty: &FFIType) -> String {
    match ty {
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
    }
}

/// Format a parameter list for commonMain (Kotlin types).
pub fn kotlin_params_common(params: &[ParamInfo]) -> String {
    params
        .iter()
        .map(|p| format!("{}: {}", kotlin_sanitize_ident(&p.name), kotlin_type(&p.ty)))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Format a JNI parameter list (wire types), optionally prepending handleId.
pub fn kotlin_params_jni(params: &[ParamInfo]) -> String {
    let args: Vec<String> = params
        .iter()
        .map(|p| {
            format!(
                "{}: {}",
                kotlin_sanitize_ident(&p.name),
                kotlin_jni_type(&p.ty)
            )
        })
        .collect();

    args.join(", ")
}

pub const fn kotlin_async_kw(is_async: &bool) -> &'static str {
    if *is_async { "suspend " } else { "" }
}

/// Generate the JNI method name used in the Kotlin JNI object.
pub fn kotlin_jni_method_name(parent: &Option<String>, rust_name: &str) -> String {
    let name = rust_name.trim_start_matches("r#");

    match parent {
        Some(s) => format!("koffi_struct_{s}_{name}"),
        None => format!("koffi_fn_{name}"),
    }
}

/// Emit the JNI call expression for a function return, including
/// postcard deserialization for non-blittable return types.
pub fn kotlin_jni_return_expr(
    params: &[ParamInfo],
    pkg: &str,
    jni_name: &str,
    ret: &FFIType,
    has_receiver: bool,
) -> String {
    let mut args = Vec::new();
    if has_receiver {
        args.push("handleId".into());
    }

    for p in params {
        let name = kotlin_sanitize_ident(&p.name);
        if p.ty.is_blittable() || p.ty == FFIType::String || p.ty == FFIType::Bytes {
            args.push(name);
        } else {
            args.push(format!("rs.koffi.KoffiSerializer.serialize({name})"));
        }
    }

    let name = jni_name.trim_start_matches("r#");
    let call = format!("{pkg}Jni.{name}({})", args.join(", "));

    if ret.is_blittable()
        || *ret == FFIType::Unit
        || *ret == FFIType::String
        || *ret == FFIType::Bytes
    {
        format!("return {call}")
    } else {
        format!("return rs.koffi.KoffiSerializer.deserialize({call})")
    }
}

#[must_use]
pub fn kotlin_sanitize_ident(ident: &str) -> String {
    static KOTLIN_KEYWORDS: [&str; 27] = [
        "as",
        "break",
        "class",
        "continue",
        "do",
        "else",
        "false",
        "for",
        "fun",
        "if",
        "in",
        "interface",
        "is",
        "null",
        "object",
        "package",
        "return",
        "super",
        "this",
        "throw",
        "true",
        "try",
        "typealias",
        "val",
        "var",
        "when",
        "while",
    ];

    if KOTLIN_KEYWORDS.contains(&ident) {
        format!("`{}`", AsLowerCamelCase(ident))
    } else {
        AsLowerCamelCase(ident).to_string()
    }
}

/// Emit a complete `return` statement for a Kotlin/Native actual function that
/// calls a C-ABI symbol via cinterop.
///
/// The generated code assumes the surrounding function is annotated with
/// `@OptIn(ExperimentalForeignApi::class)` and is wrapped in the appropriate
/// `memScoped {}` when heap-allocated values are involved.
#[must_use]
pub fn kotlin_native_return_expr(f: &FnInfo, c_sym: &str) -> String {
    let mut rust_call_args: Vec<String> = Vec::new();

    // If this is a method, the first C arg is the handle ID.
    if f.parent_struct.is_some() {
        rust_call_args.push("handleId.toULong()".into());
    }

    for p in &f.params {
        let name = AsLowerCamelCase(&p.name).to_string();
        match &p.ty {
            FFIType::String => {
                // Pass as pointer + length pair inside withUtf8Bytes lambda.
                // We collect these separately and wrap everything in memScoped.
                rust_call_args.push(format!("{name}Ptr"));
                rust_call_args.push(format!("{name}Len"));
            }
            FFIType::Bytes => {
                rust_call_args.push(format!("{name}Ptr"));
                rust_call_args.push(format!("{name}Len"));
            }
            FFIType::Data(_) => {
                // Serialized. Pass as pointer + length.
                rust_call_args.push(format!("{name}Ptr"));
                rust_call_args.push(format!("{name}Len"));
            }
            _ if p.ty.is_blittable() => {
                rust_call_args.push(name);
            }
            FFIType::Opaque(_) => {
                // Opaque params are handle IDs passed as ULong.
                rust_call_args.push(format!("{name}.handleId.toULong()"));
            }
            _ => rust_call_args.push(name),
        }
    }

    // Build per-param setup lines for string/bytes/data params that need
    // pinning or serialization.
    let mut setup_lines: Vec<String> = Vec::new();
    for p in &f.params {
        let name = AsLowerCamelCase(&p.name).to_string();
        match &p.ty {
            FFIType::String => {
                setup_lines.push(format!("val {name}Bytes = {name}.encodeToByteArray()"));
                setup_lines.push(format!("val {name}Pinned = {name}Bytes.pin()"));
                setup_lines.push(format!(
                    "val {name}Ptr = {name}Pinned.addressOf(0).reinterpret<UByteVar>()"
                ));
                setup_lines.push(format!("val {name}Len = {name}Bytes.size.toULong()"));
            }
            FFIType::Bytes => {
                setup_lines.push(format!("val {name}Pinned = {name}.pin()"));
                setup_lines.push(format!(
                    "val {name}Ptr = {name}Pinned.addressOf(0).reinterpret<UByteVar>()"
                ));
                setup_lines.push(format!("val {name}Len = {name}.size.toULong()"));
            }
            FFIType::Data(r) => {
                let hash = r.schema_hash;
                setup_lines.push(format!(
                    "val {name}Bytes = KoffiSerializer.serialize({name}, 0x{hash:016x}_uL) {{ write{name}(it) }}"
                ));
                setup_lines.push(format!("val {name}Pinned = {name}Bytes.pin()"));
                setup_lines.push(format!(
                    "val {name}Ptr = {name}Pinned.addressOf(0).reinterpret<UByteVar>()"
                ));
                setup_lines.push(format!("val {name}Len = {name}Bytes.size.toULong()"));
            }
            _ => {}
        }
    }

    // Collect pinned objects that need to be unpinned.
    let pinned_names: Vec<String> = f
        .params
        .iter()
        .filter(|p| matches!(p.ty, FFIType::String | FFIType::Bytes | FFIType::Data(_)))
        .map(|p| format!("{}Pinned", AsLowerCamelCase(&p.name)))
        .collect();

    let call = format!("{c_sym}({})", rust_call_args.join(", "));

    // Build the full return expression.
    let needs_scope = !setup_lines.is_empty() || needs_buf_read(&f.ret_ty);

    if !needs_scope {
        // Simple case: no setup, no heap reads.
        let ret = kotlin_simple_return_convert(&f.ret_ty, &call);
        return format!("return {ret}");
    }

    // Complex case: use a try/finally block for pinned memory + buf reads.
    let mut lines = Vec::new();
    lines.extend(setup_lines);
    lines.push(format!("val __result = {call}"));

    // Unpin in finally.
    let unpin_stmts: Vec<String> = pinned_names
        .iter()
        .map(|n| format!("{n}.unpin()"))
        .collect();

    let convert = kotlin_buf_return_convert(&f.ret_ty, "__result");

    if unpin_stmts.is_empty() {
        lines.push(format!("return {convert}"));
        format!("run {{\n        {}\n    }}", lines.join("\n        "))
    } else {
        let body = lines.join("\n        ");
        let unpins = unpin_stmts.join("\n        ");
        format!(
            "try {{\n        {body}\n        return {convert}\n    }} finally {{\n        {unpins}\n    }}"
        )
    }
}

fn kotlin_simple_return_convert(ty: &FFIType, call: &str) -> String {
    match ty {
        FFIType::Unit => format!("{call}"),
        FFIType::Bool => format!("{call}"),
        FFIType::I8 => format!("{call}.toByte()"),
        FFIType::U8 => format!("{call}.toUByte()"),
        FFIType::I16 => format!("{call}.toShort()"),
        FFIType::U16 => format!("{call}.toUShort()"),
        FFIType::I32 => format!("{call}.toInt()"),
        FFIType::U32 => format!("{call}.toUInt()"),
        FFIType::I64 => format!("{call}.toLong()"),
        FFIType::U64 => format!("{call}.toULong()"),
        FFIType::F32 => format!("{call}.toFloat()"),
        FFIType::F64 => format!("{call}.toDouble()"),
        FFIType::Opaque(r) => format!("{}({call}.toLong())", r.name),
        _ => call.to_string(),
    }
}

fn kotlin_buf_return_convert(ty: &FFIType, result_var: &str) -> String {
    match ty {
        FFIType::String => {
            format!(
                "run {{ val bytes = readAndFreeByteBuf({result_var}); bytes.decodeToString() }}"
            )
        }
        FFIType::Bytes => {
            format!("readAndFreeByteBuf({result_var})")
        }
        FFIType::Data(r) => {
            let hash = r.schema_hash;
            let name = &r.name;
            format!(
                "KoffiSerializer.deserialize(readAndFreeByteBuf({result_var}), 0x{hash:016x}_uL) {{ read{name}() }}"
            )
        }
        _ => kotlin_simple_return_convert(ty, result_var),
    }
}
