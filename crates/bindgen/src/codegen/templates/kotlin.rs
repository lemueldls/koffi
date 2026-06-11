use heck::AsLowerCamelCase;
use koffi_ir::{FFIType, FnInfo, ParamInfo};

/// Map [`FFIType`] to its Kotlin commonMain representation.
#[must_use]
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
                kotlin_type(err),
            )
        }
        FFIType::Vec(inner) => format!("List<{}>", kotlin_type(inner)),
        FFIType::Map(k, v) => format!("Map<{}, {}>", kotlin_type(k), kotlin_type(v)),
        FFIType::Set(inner) => format!("Set<{}>", kotlin_type(inner)),
        FFIType::Opaque(r) | FFIType::Data(r) => r.name.clone(),
    }
}

/// Map [`FFIType`] to its JNI wire type (what crosses the JNI boundary).
#[must_use]
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
        _ => "ByteArray".into(),
    }
}

/// Format a parameter list for commonMain (Kotlin types).
#[must_use]
pub fn kotlin_params_common(params: &[ParamInfo]) -> String {
    params
        .iter()
        .map(|p| format!("{}: {}", kotlin_sanitize_ident(&p.name), kotlin_type(&p.ty)))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Format a JNI parameter list (wire types), optionally prepending handleId.
#[must_use]
pub fn kotlin_params_jni(params: &[ParamInfo], has_receiver: bool) -> String {
    let mut args = Vec::new();
    if has_receiver {
        args.push("handleId: Long".to_string());
    }

    for p in params {
        args.push(format!(
            "{}: {}",
            kotlin_sanitize_ident(&p.name),
            kotlin_jni_type(&p.ty)
        ));
    }
    args.join(", ")
}

#[must_use]
pub const fn kotlin_async_kw(is_async: bool) -> &'static str {
    if is_async { "suspend " } else { "" }
}

/// Generate the JNI `external fun` name for a function.
///
/// The name is unique within the `{CrateIdent}Jni` object and is stable
/// across recompilations (deterministic from the function's location).
///
/// Convention:
/// - Free fn at root: `koffi_fn_{rust_name}`
/// - Free fn in module: `koffi_fn_{mod1}_{mod2}_{rust_name}`
/// - Method at root: `koffi_struct_{TypeName}_{rust_name}`
/// - Method in module: `koffi_struct_{TypeName}_{mod1}_{mod2}_{rust_name}`
///
/// The `koffi_fn_` / `koffi_struct_` prefix prevents name collisions between
/// a free function and a method that happen to share a name after module-path
/// expansion.
#[must_use]
pub fn kotlin_jni_method_name(f: &FnInfo) -> String {
    let raw = f.rust_name.trim_start_matches("r#");

    let mod_suffix = if f.rust_module_path.is_empty() {
        String::new()
    } else {
        format!("_{}", f.rust_module_path.join("_"))
    };

    match &f.parent_struct {
        Some(parent) => format!("koffi_struct_{parent}{mod_suffix}_{raw}"),
        None => format!("koffi_fn{mod_suffix}_{raw}"),
    }
}

/// Emit the complete `return` statement for a Kotlin JVM/Android actual
/// function, calling through the `{Pkg}Jni` internal object.
///
/// `jni_name` is the fully-prefixed external fun name (output of
/// [`kotlin_jni_method_name`]). It is used directly with no further prefixing.
#[must_use]
pub fn kotlin_jni_return_expr(
    f: &FnInfo,
    pkg_pascal: &str,
    jni_name: &str,
    has_receiver: bool,
) -> String {
    let mut args = Vec::new();

    if has_receiver {
        args.push("handleId".to_string());
    }

    for p in &f.params {
        let name = kotlin_sanitize_ident(&p.name);
        let serialized = !p.ty.is_blittable() && p.ty != FFIType::String && p.ty != FFIType::Bytes;
        if serialized {
            args.push(format!("rs.koffi.KoffiSerializer.serialize({name})"));
        } else {
            args.push(name);
        }
    }

    let call = format!("{pkg_pascal}Jni.{jni_name}({})", args.join(", "));

    // Wrap in deserialization for non-trivial returns.
    if f.ret_ty.is_blittable()
        || f.ret_ty == FFIType::Unit
        || f.ret_ty == FFIType::String
        || f.ret_ty == FFIType::Bytes
    {
        format!("return {call}")
    } else if let FFIType::Opaque(r) = &f.ret_ty {
        format!("return {}({call})", r.name)
    } else {
        format!("return rs.koffi.KoffiSerializer.deserialize({call})")
    }
}

/// Escape Kotlin hard keywords with backticks.
#[must_use]
pub fn kotlin_sanitize_ident(ident: &str) -> String {
    const KOTLIN_KEYWORDS: &[&str] = &[
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

    // Strip Rust raw-identifier prefix first.
    let ident = ident.trim_start_matches("r#");
    let camel = AsLowerCamelCase(ident).to_string();

    if KOTLIN_KEYWORDS.contains(&camel.as_str()) {
        format!("`{camel}`")
    } else {
        camel
    }
}

/// Emit a complete `return` statement for a Kotlin/Native actual function that
/// calls a C-ABI symbol via cinterop.
///
/// The generated code assumes the surrounding function is annotated with
/// `@OptIn(ExperimentalForeignApi::class)` and is wrapped in the appropriate
/// `try/finally` when pinned memory is involved.
#[must_use]
pub fn kotlin_native_return_expr(f: &FnInfo, c_sym: &str) -> String {
    let mut call_args: Vec<String> = Vec::new();

    // Instance methods: pass handle ID as first C arg.
    if f.receiver.is_some() {
        call_args.push("handleId.toULong()".into());
    }

    for p in &f.params {
        let name = AsLowerCamelCase(&p.name).to_string();
        match &p.ty {
            FFIType::String | FFIType::Bytes | FFIType::Data(_) => {
                call_args.push(format!("{name}Ptr"));
                call_args.push(format!("{name}Len"));
            }
            _ if p.ty.is_blittable() => call_args.push(name),
            FFIType::Opaque(_) => call_args.push(format!("{name}.handleId.toULong()")),
            _ => call_args.push(name),
        }
    }

    // Per-param setup: pinning + serialization for string/bytes/data params.
    let mut setup: Vec<String> = Vec::new();
    let mut pinned: Vec<String> = Vec::new();

    for p in &f.params {
        let name = AsLowerCamelCase(&p.name).to_string();
        match &p.ty {
            FFIType::String => {
                setup.push(format!("val {name}Bytes = {name}.encodeToByteArray()"));
                setup.push(format!("val {name}Pinned = {name}Bytes.pin()"));
                setup.push(format!(
                    "val {name}Ptr = {name}Pinned.addressOf(0).reinterpret<UByteVar>()"
                ));
                setup.push(format!("val {name}Len = {name}Bytes.size.toULong()"));
                pinned.push(format!("{name}Pinned"));
            }
            FFIType::Bytes => {
                setup.push(format!("val {name}Pinned = {name}.pin()"));
                setup.push(format!(
                    "val {name}Ptr = {name}Pinned.addressOf(0).reinterpret<UByteVar>()"
                ));
                setup.push(format!("val {name}Len = {name}.size.toULong()"));
                pinned.push(format!("{name}Pinned"));
            }
            FFIType::Data(r) => {
                let hash = r.schema_hash;
                let type_name = &r.name;
                setup.push(format!(
                    "val {name}Bytes = KoffiSerializer.serialize({name}, 0x{hash:016x}_uL) {{ write{type_name}(it) }}"
                ));
                setup.push(format!("val {name}Pinned = {name}Bytes.pin()"));
                setup.push(format!(
                    "val {name}Ptr = {name}Pinned.addressOf(0).reinterpret<UByteVar>()"
                ));
                setup.push(format!("val {name}Len = {name}Bytes.size.toULong()"));
                pinned.push(format!("{name}Pinned"));
            }
            _ => {}
        }
    }

    let call = format!("{c_sym}({})", call_args.join(", "));
    let needs_scope = !setup.is_empty() || needs_buf_read(&f.ret_ty);

    if !needs_scope {
        let ret = simple_convert(&f.ret_ty, &call);
        return format!("return {ret}");
    }

    let mut body = setup;
    body.push(format!("val __result = {call}"));

    let convert = buf_convert(&f.ret_ty, "__result");
    let unpin_stmts: Vec<String> = pinned.iter().map(|n| format!("{n}.unpin()")).collect();

    if unpin_stmts.is_empty() {
        body.push(format!("return {convert}"));

        format!("run {{\n        {}\n    }}", body.join("\n        "))
    } else {
        let b = body.join("\n        ");
        let u = unpin_stmts.join("\n        ");

        format!(
            "try {{\n        {b}\n        return {convert}\n    }} finally {{\n        {u}\n    }}"
        )
    }
}

const fn needs_buf_read(ty: &FFIType) -> bool {
    matches!(ty, FFIType::String | FFIType::Bytes | FFIType::Data(_))
}

fn simple_convert(ty: &FFIType, call: &str) -> String {
    match ty {
        FFIType::Unit => call.to_string(),
        FFIType::Bool => call.to_string(),
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

fn buf_convert(ty: &FFIType, var: &str) -> String {
    match ty {
        FFIType::String => {
            format!("run {{ val bytes = readAndFreeByteBuf({var}); bytes.decodeToString() }}")
        }
        FFIType::Bytes => format!("readAndFreeByteBuf({var})"),
        FFIType::Data(r) => {
            let hash = r.schema_hash;
            let type_name = &r.name;

            format!(
                "KoffiSerializer.deserialize(readAndFreeByteBuf({var}), 0x{hash:016x}_uL) {{ read{type_name}() }}"
            )
        }
        _ => simple_convert(ty, var),
    }
}
