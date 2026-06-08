use std::{fs, path::Path};

use crate::parser::{CrateInterface, FFIType};

pub fn generate_kotlin(
    ir: &CrateInterface,
    out_dir: &Path,
    crate_name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let pkg_name = heck::AsPascalCase(crate_name).to_string();
    let pkg_file = format!("{pkg_name}.kt");

    // Generate commonMain files
    let common_dir = out_dir.join("src");
    fs::create_dir_all(&common_dir)?;

    let mut common_src = String::new();
    common_src.push_str(&format!("package {}\n\n", ir.namespace));

    // Opaque structs as expect classes
    for s in &ir.structs {
        if s.is_opaque {
            common_src.push_str(&format!(
                "expect class {} : rs.koffi.KoffiHandleBase {{\n",
                s.name
            ));
            common_src.push_str("    constructor(handleId: Long)\n");
            // Add methods for this opaque struct
            for f in &ir.functions {
                if f.parent_struct.as_deref() == Some(&s.name) {
                    common_src.push_str(&format!(
                        "    {}fun {}({}): {}\n",
                        if f.is_async { "suspend " } else { "" },
                        f.name,
                        format_params_common(&f.params),
                        to_kotlin_type(&f.ret_ty)
                    ));
                }
            }
            common_src.push_str("}\n\n");
        } else {
            // Transparent structs as data classes
            common_src.push_str(&format!("data class {}(\n", s.name));
            for field in &s.fields {
                common_src.push_str(&format!(
                    "    val {}: {},\n",
                    heck::AsLowerCamelCase(&field.name),
                    to_kotlin_type(&field.ty)
                ));
            }
            common_src.push_str(")\n\n");
        }
    }

    // Enums
    for e in &ir.enums {
        common_src.push_str(&format!(
            "sealed class {} : rs.koffi.KoffiError() {{\n",
            e.name
        ));
        for variant in &e.variants {
            if variant.fields.is_empty() {
                common_src.push_str(&format!("    object {} : {}()\n", variant.name, e.name));
            } else {
                common_src.push_str(&format!("    data class {}(\n", variant.name));
                for field in &variant.fields {
                    common_src.push_str(&format!(
                        "        val {}: {},\n",
                        heck::AsLowerCamelCase(&field.name),
                        to_kotlin_type(&field.ty)
                    ));
                }
                common_src.push_str(&format!("    ) : {}()\n", e.name));
            }
        }
        common_src.push_str("}\n\n");
    }

    // Free functions
    for f in &ir.functions {
        if f.parent_struct.is_none() {
            common_src.push_str(&format!(
                "expect {}fun {}({}): {}\n\n",
                if f.is_async { "suspend " } else { "" },
                f.name,
                format_params_common(&f.params),
                to_kotlin_type(&f.ret_ty)
            ));
        }
    }

    fs::write(common_dir.join(&pkg_file), common_src)?;

    // Generate JVM JNI actuals
    let jvm_dir = out_dir.join("src@jvm");
    fs::create_dir_all(&jvm_dir)?;

    let mut jni_src = String::new();
    jni_src.push_str(&format!("package {}\n\n", ir.namespace));

    // JNI Native interface declarations
    jni_src.push_str(&format!("internal object {pkg_name}Jni {{\n"));
    jni_src.push_str("    init {\n");
    jni_src.push_str("        // Library loading handled by KoffiLoader\n");
    jni_src.push_str("    }\n\n");
    for f in &ir.functions {
        let jni_fn_name = format_jni_method_name(f.parent_struct.as_ref(), &f.rust_name);
        jni_src.push_str(&format!(
            "    @JvmStatic\n    external fun {}({}): {}\n\n",
            jni_fn_name,
            format_params_jni(&f.params, f.parent_struct.is_some()),
            to_kotlin_jni_type(&f.ret_ty)
        ));
    }
    jni_src.push_str("}\n\n");

    // Actual class implementations for JVM
    for s in &ir.structs {
        if s.is_opaque {
            jni_src.push_str(&format!(
                "actual class {} actual constructor(handleId: Long) : rs.koffi.KoffiHandleBase(handleId) {{\n",
                s.name
            ));
            for f in &ir.functions {
                if f.parent_struct.as_deref() == Some(&s.name) {
                    let jni_fn_name =
                        format_jni_method_name(f.parent_struct.as_ref(), &f.rust_name);
                    jni_src.push_str(&format!(
                        "    actual {}fun {}({}): {} {{\n",
                        if f.is_async { "suspend " } else { "" },
                        f.name,
                        format_params_common(&f.params),
                        to_kotlin_type(&f.ret_ty)
                    ));
                    // Check if closed
                    jni_src.push_str(
                        "        if (isClosed) throw rs.koffi.KoffiHandleClosedException()\n",
                    );
                    // Implement method call
                    jni_src.push_str(&format!(
                        "        {}\n",
                        format_jni_call(&pkg_name, &jni_fn_name, &f.params, &f.ret_ty, true)
                    ));
                    jni_src.push_str("    }\n\n");
                }
            }
            jni_src.push_str("}\n\n");
        }
    }

    // Free functions actuals
    for f in &ir.functions {
        if f.parent_struct.is_none() {
            let jni_fn_name = format_jni_method_name(f.parent_struct.as_ref(), &f.rust_name);
            jni_src.push_str(&format!(
                "actual {}fun {}({}): {} {{\n",
                if f.is_async { "suspend " } else { "" },
                f.name,
                format_params_common(&f.params),
                to_kotlin_type(&f.ret_ty)
            ));
            jni_src.push_str(&format!(
                "    {}\n",
                format_jni_call(&pkg_name, &jni_fn_name, &f.params, &f.ret_ty, false)
            ));
            jni_src.push_str("}\n\n");
        }
    }

    fs::write(jvm_dir.join(&pkg_file), &jni_src)?;

    // Generate Android JNI actuals
    let android_dir = out_dir.join("src@android");
    fs::create_dir_all(&android_dir)?;
    fs::write(android_dir.join(&pkg_file), &jni_src)?;

    // Generate native cinterop actuals
    let native_dir = out_dir.join("src@native");
    fs::create_dir_all(&native_dir)?;

    let mut native_src = String::new();
    native_src.push_str(&format!("package {}\n\n", ir.namespace));
    native_src.push_str("import kotlinx.cinterop.*\n\n");

    for s in &ir.structs {
        if s.is_opaque {
            native_src.push_str(&format!(
                "actual class {} actual constructor(handleId: Long) : rs.koffi.KoffiHandleBase(handleId) {{\n",
                s.name
            ));
            for f in &ir.functions {
                if f.parent_struct.as_deref() == Some(&s.name) {
                    native_src.push_str(&format!(
                        "    actual {}fun {}({}): {} {{\n",
                        if f.is_async { "suspend " } else { "" },
                        f.name,
                        format_params_common(&f.params),
                        to_kotlin_type(&f.ret_ty)
                    ));
                    native_src.push_str(
                        "        if (isClosed) throw rs.koffi.KoffiHandleClosedException()\n",
                    );
                    native_src.push_str(&format!(
                        "        {}\n",
                        format_c_call(&f.rust_name, &f.params, &f.ret_ty, true)
                    ));
                    native_src.push_str("    }\n\n");
                }
            }
            native_src.push_str("}\n\n");
        }
    }

    // Free functions
    for f in &ir.functions {
        if f.parent_struct.is_none() {
            native_src.push_str(&format!(
                "actual {}fun {}({}): {} {{\n",
                if f.is_async { "suspend " } else { "" },
                f.name,
                format_params_common(&f.params),
                to_kotlin_type(&f.ret_ty)
            ));
            native_src.push_str(&format!(
                "    {}\n",
                format_c_call(&f.rust_name, &f.params, &f.ret_ty, false)
            ));
            native_src.push_str("}\n\n");
        }
    }

    fs::write(native_dir.join(&pkg_file), native_src)?;

    Ok(())
}

fn to_kotlin_type(ty: &FFIType) -> String {
    match ty {
        FFIType::Bool => "Boolean".to_string(),
        FFIType::I8 => "Byte".to_string(),
        FFIType::I16 => "Short".to_string(),
        FFIType::I32 => "Int".to_string(),
        FFIType::I64 => "Long".to_string(),
        FFIType::U8 => "UByte".to_string(),
        FFIType::U16 => "UShort".to_string(),
        FFIType::U32 => "UInt".to_string(),
        FFIType::U64 => "ULong".to_string(),
        FFIType::F32 => "Float".to_string(),
        FFIType::F64 => "Double".to_string(),
        FFIType::Unit => "Unit".to_string(),
        FFIType::String => "String".to_string(),
        FFIType::Bytes => "ByteArray".to_string(),
        FFIType::Option(inner) => format!("{}?", to_kotlin_type(inner)),
        FFIType::Result(ok, err) => {
            format!(
                "rs.koffi.KoffiResult<{}, {}>",
                to_kotlin_type(ok),
                to_kotlin_type(err)
            )
        }
        FFIType::Vec(inner) => format!("List<{}>", to_kotlin_type(inner)),
        FFIType::Map(k, v) => format!("Map<{}, {}>", to_kotlin_type(k), to_kotlin_type(v)),
        FFIType::Custom(name) => name.clone(),
    }
}

fn to_kotlin_jni_type(ty: &FFIType) -> String {
    match ty {
        FFIType::Bool => "Boolean".to_string(),
        FFIType::I8 | FFIType::U8 => "Byte".to_string(),
        FFIType::I16 | FFIType::U16 => "Short".to_string(),
        FFIType::I32 | FFIType::U32 => "Int".to_string(),
        FFIType::I64 | FFIType::U64 => "Long".to_string(),
        FFIType::F32 => "Float".to_string(),
        FFIType::F64 => "Double".to_string(),
        FFIType::Unit => "Unit".to_string(),
        FFIType::String => "String".to_string(),
        FFIType::Bytes => "ByteArray".to_string(),
        // Complex types are serialized to byte arrays on JVM/JNI
        FFIType::Option(_)
        | FFIType::Result(..)
        | FFIType::Vec(_)
        | FFIType::Map(..)
        | FFIType::Custom(_) => "ByteArray".to_string(),
    }
}

fn format_params_common(params: &[crate::parser::ParamInfo]) -> String {
    params
        .iter()
        .map(|p| {
            format!(
                "{}: {}",
                heck::AsLowerCamelCase(&p.name),
                to_kotlin_type(&p.ty)
            )
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn format_params_jni(params: &[crate::parser::ParamInfo], has_receiver: bool) -> String {
    let mut list = Vec::new();
    if has_receiver {
        list.push("handleId: Long".to_string());
    }

    for p in params {
        list.push(format!(
            "{}: {}",
            heck::AsLowerCamelCase(&p.name),
            to_kotlin_jni_type(&p.ty)
        ));
    }

    list.join(", ")
}

pub fn format_jni_method_name(parent_struct: Option<&String>, rust_name: &str) -> String {
    match parent_struct {
        Some(s) => format!("koffi_struct_{s}_{rust_name}"),
        None => format!("koffi_fn_{rust_name}"),
    }
}

fn format_jni_call(
    pkg_name: &str,
    jni_fn_name: &str,
    params: &[crate::parser::ParamInfo],
    ret_ty: &FFIType,
    has_receiver: bool,
) -> String {
    let mut args = Vec::new();
    if has_receiver {
        args.push("handleId".to_string());
    }

    for p in params {
        let name = heck::AsLowerCamelCase(&p.name).to_string();
        if p.ty.is_blittable() || p.ty == FFIType::String || p.ty == FFIType::Bytes {
            args.push(name);
        } else {
            // Serialize using postcard placeholder (we will generate postcard helper functions in Kotlin)
            args.push(format!("rs.koffi.PostcardSerializer.serialize({name})"));
        }
    }

    let call = format!("{pkg_name}Jni.{jni_fn_name}({})", args.join(", "));

    if ret_ty.is_blittable()
        || *ret_ty == FFIType::Unit
        || *ret_ty == FFIType::String
        || *ret_ty == FFIType::Bytes
    {
        format!("return {call}")
    } else {
        // Deserialize returned serialized byte array
        format!(
            "val resBytes = {call}\n        return rs.koffi.PostcardSerializer.deserialize(resBytes)"
        )
    }
}

fn format_c_call(
    rust_name: &str,
    params: &[crate::parser::ParamInfo],
    ret_ty: &FFIType,
    has_receiver: bool,
) -> String {
    // For iOS, Kotlin/Native uses cinterop mappings.
    // Opaque handles pass `handleId` directly.
    // Strings get pinned, or passed as CStrings.
    // Serialized types get serialized to ByteArray, pinned, and passed as pointer/len.
    // Let's implement a clean invocation.
    let mut prep = String::new();
    let mut args = Vec::new();

    if has_receiver {
        args.push("handleId".to_string());
    }

    for p in params {
        let name = heck::AsLowerCamelCase(&p.name).to_string();
        if p.ty.is_blittable() {
            args.push(name);
        } else if p.ty == FFIType::String {
            // Pass string via memScoped to CString
            prep.push_str(&format!("        val {name}Ptr = {name}.cstr\n"));
            args.push(format!("{name}Ptr"));
        } else if p.ty == FFIType::Bytes {
            // Pass ByteArray pointer and length
            args.push(format!("{name}.refTo(0)"));
            args.push(format!("{name}.size"));
        } else {
            // Serialized type
            prep.push_str(&format!(
                "        val {name}Bytes = rs.koffi.PostcardSerializer.serialize({name})\n"
            ));
            args.push(format!("{name}Bytes.refTo(0)"));
            args.push(format!("{name}Bytes.size"));
        }
    }

    let c_fn_call = format!("{}({})", rust_name, args.join(", "));
    let mut body = String::new();

    if !prep.is_empty() {
        body.push_str("memScoped {\n");
        body.push_str(&prep);
        body.push_str("            ");
    }

    if ret_ty.is_blittable() || *ret_ty == FFIType::Unit {
        body.push_str(&format!("val res = {c_fn_call}\n"));
        if *ret_ty != FFIType::Unit {
            body.push_str("            return res\n");
        }
    } else if *ret_ty == FFIType::String {
        body.push_str(&format!("val resPtr = {c_fn_call}\n"));
        body.push_str("            return resPtr?.toKString() ?: \"\"\n");
    } else {
        // Returns a KoffiByteBuf
        body.push_str(&format!("val resBuf = {c_fn_call}\n"));
        body.push_str("            val bytes = ByteArray(resBuf.len.toInt())\n");
        body.push_str("            if (resBuf.len > 0u) {\n");
        body.push_str("                bytes.usePinned { pinned ->\n");
        body.push_str("                    memcpy(pinned.addressOf(0), resBuf.ptr, resBuf.len)\n");
        body.push_str("                }\n");
        body.push_str("                koffi_free_byte_buf(resBuf)\n");
        body.push_str("            }\n");
        body.push_str("            return rs.koffi.PostcardSerializer.deserialize(bytes)\n");
    }

    if !prep.is_empty() {
        body.push_str("        }");
    }

    body
}
