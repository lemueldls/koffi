use std::{
    fs::{self, create_dir_all},
    path::Path,
};

use crate::parser::{CrateInterface, FFIType, ReceiverType};

pub fn generate_rust(
    ir: &CrateInterface,
    out_dir: &Path,
    crate_name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let pkg_name = heck::AsPascalCase(crate_name).to_string();

    let rust_glue_dir = out_dir.join("rust/src");
    create_dir_all(&rust_glue_dir)?;

    let mut jni_src = String::new();
    jni_src.push_str(&format!(
        "// Auto-generated glue for JNI. Do not edit.\n\n\
        use jni::JNIEnv;\n\
        use jni::objects::{{JClass, JString, JByteArray}};\n\
        use jni::sys::{{jint, jlong, jbyte, jshort, jboolean, jfloat, jdouble}};\n\
        use std::sync::Arc;\n\n\
        // Import target crate\n\
        use {}::*;\n\n",
        crate_name.replace('-', "_")
    ));

    for f in &ir.functions {
        let jni_fn_name =
            crate::codegen::kotlin::format_jni_method_name(f.parent_struct.as_ref(), &f.rust_name);
        let pkg_prefix = ir.namespace.replace('.', "_");

        let jni_symbol_name = format!(
            "Java_{pkg_prefix}_{pkg_name}Jni_{}",
            jni_fn_name.replace('_', "_1")
        );

        jni_src.push_str(&format!(
            "#[unsafe(no_mangle)]\n\
            pub unsafe extern \"system\" fn {}(\n\
            \tmut env: JNIEnv,\n\
            \t_class: JClass,\n\
            \t{}\n\
            ) -> {} {{\n",
            jni_symbol_name,
            format_jni_rust_params(&f.params, f.parent_struct.is_some()),
            to_jni_rust_ret_type(&f.ret_ty, ir)
        ));

        // Setup argument conversion
        let mut conv = String::new();
        let mut call_args = Vec::new();

        if let Some(parent) = &f.parent_struct {
            conv.push_str(&format!(
                "\tlet parent_arc = koffi_runtime::HandleRegistry::global().get::<{parent}>(handle_id as u64).expect(\"Invalid handle\");\n"
            ));
            match f.receiver {
                Some(ReceiverType::Ref) => call_args.push("&*parent_arc".to_string()),
                Some(ReceiverType::RefMut) => {
                    conv.push_str(&format!(
                        "\tlet parent_mut_ptr = Arc::as_ptr(&parent_arc) as *mut {parent};\n\
                        \tlet parent_mut = unsafe {{ &mut *parent_mut_ptr }};\n"
                    ));
                    call_args.push("parent_mut".to_string());
                }
                Some(ReceiverType::Owned) | None => {
                    conv.push_str("\tkoffi_runtime::HandleRegistry::global().remove(handle_id as u64);\n\
                        \tlet parent_owned = Arc::try_unwrap(parent_arc).ok().expect(\"Cannot consume handle\");\n");
                    call_args.push("parent_owned".to_string());
                }
            }
        }

        for p in &f.params {
            let r_name = format!("r_{}", p.name);
            match &p.ty {
                FFIType::Bool => {
                    conv.push_str(&format!("\tlet {} = {} != 0;\n", r_name, p.name));
                    call_args.push(r_name);
                }
                FFIType::I8 => {
                    conv.push_str(&format!("\tlet {} = {} as i8;\n", r_name, p.name));
                    call_args.push(r_name);
                }
                FFIType::I16 => {
                    conv.push_str(&format!("\tlet {} = {} as i16;\n", r_name, p.name));
                    call_args.push(r_name);
                }
                FFIType::I32 => {
                    conv.push_str(&format!("\tlet {} = {} as i32;\n", r_name, p.name));
                    call_args.push(r_name);
                }
                FFIType::I64 => {
                    conv.push_str(&format!("\tlet {} = {} as i64;\n", r_name, p.name));
                    call_args.push(r_name);
                }
                FFIType::U8 => {
                    conv.push_str(&format!("\tlet {} = {} as u8;\n", r_name, p.name));
                    call_args.push(r_name);
                }
                FFIType::U16 => {
                    conv.push_str(&format!("\tlet {} = {} as u16;\n", r_name, p.name));
                    call_args.push(r_name);
                }
                FFIType::U32 => {
                    conv.push_str(&format!("\tlet {} = {} as u32;\n", r_name, p.name));
                    call_args.push(r_name);
                }
                FFIType::U64 => {
                    conv.push_str(&format!("\tlet {} = {} as u64;\n", r_name, p.name));
                    call_args.push(r_name);
                }
                FFIType::F32 => {
                    conv.push_str(&format!("\tlet {} = {} as f32;\n", r_name, p.name));
                    call_args.push(r_name);
                }
                FFIType::F64 => {
                    conv.push_str(&format!("\tlet {} = {} as f64;\n", r_name, p.name));
                    call_args.push(r_name);
                }
                FFIType::String => {
                    conv.push_str(&format!(
                        "\tlet {}_jstr: JString = env.get_string(&{}).expect(\"Invalid string\").into();\n\
                        \tlet {} = {}_jstr.to_str().expect(\"Invalid UTF-8\").to_string();\n",
                        p.name, p.name, r_name, p.name
                    ));
                    call_args.push(r_name);
                }
                FFIType::Bytes => {
                    conv.push_str(&format!(
                        "\tlet {} = env.convert_byte_array(&{}).expect(\"Invalid byte array\");\n",
                        r_name, p.name
                    ));
                    call_args.push(format!("&{r_name}"));
                }
                _ => {
                    conv.push_str(&format!(
                        "\tlet {}_bytes = env.convert_byte_array(&{}).expect(\"Invalid serialized bytes\");\n\
                        \tlet {} = postcard::from_bytes(&{}_bytes).expect(\"Postcard deserialization failed\");\n",
                        p.name, p.name, r_name, p.name
                    ));
                    call_args.push(r_name);
                }
            }
        }

        jni_src.push_str(&conv);

        let rust_call_prefix = match &f.parent_struct {
            Some(s) => {
                if f.receiver.is_some() {
                    match f.receiver {
                        Some(ReceiverType::Ref) => format!("parent_arc.{}", f.rust_name),
                        Some(ReceiverType::RefMut) => format!("parent_mut.{}", f.rust_name),
                        Some(ReceiverType::Owned) => format!("parent_owned.{}", f.rust_name),
                        None => unreachable!(),
                    }
                } else {
                    format!("{}::{}", s, f.rust_name)
                }
            }
            None => f.rust_name.clone(),
        };

        jni_src.push_str(&format!(
            "\tlet res = koffi_runtime::catch_panic(|| {{\n\
            \t\t{}({})\n\
            \t}});\n",
            rust_call_prefix,
            call_args.join(", ")
        ));

        jni_src.push_str("\tmatch res {\n");
        jni_src.push_str("\t\tOk(val) => {\n");
        jni_src.push_str(&format!(
            "\t\t\t{}\n",
            format_jni_rust_return_conversion(&f.ret_ty, ir)
        ));
        jni_src.push_str("\t\t}\n");
        jni_src.push_str("\t\tErr(panic_msg) => {\n");
        jni_src.push_str("\t\t\tlet _ = env.throw_new(\"io/koffi/KoffiPanic\", panic_msg);\n");
        jni_src.push_str(&format!(
            "\t\t\t{}\n",
            format_jni_rust_default_return(&f.ret_ty, ir)
        ));
        jni_src.push_str("\t\t}\n");
        jni_src.push_str("\t}\n");
        jni_src.push_str("}\n\n");
    }

    fs::write(rust_glue_dir.join("jni_glue.rs"), jni_src)?;

    // Generate C-ABI function wrappers for iOS (cabi_glue.rs)
    let mut cabi_src = String::new();
    cabi_src.push_str(&format!(
        "// Auto-generated glue for C-ABI / iOS. Do not edit.\n\n\
        use std::ffi::{{c_char, CStr}};\n\
        use std::sync::Arc;\n\
        use koffi_runtime::{{KoffiByteBuf, catch_panic, HandleRegistry}};\n\n\
        // Import target crate\n\
        use {}::*;\n\n",
        crate_name.replace('-', "_")
    ));

    for f in &ir.functions {
        cabi_src.push_str(&format!(
            "#[unsafe(no_mangle)]\n\
            pub unsafe extern \"C\" fn {}(\n\
            \t{}\n\
            ) -> {} {{\n",
            f.rust_name,
            format_cabi_rust_params(&f.params, f.parent_struct.is_some()),
            to_cabi_rust_ret_type(&f.ret_ty, ir)
        ));

        let mut conv = String::new();
        let mut call_args = Vec::new();

        if let Some(parent) = &f.parent_struct {
            conv.push_str(&format!(
                "\tlet parent_arc = HandleRegistry::global().get::<{parent}>(handle_id).expect(\"Invalid handle\");\n"
            ));
            match f.receiver {
                Some(ReceiverType::Ref) => call_args.push("&*parent_arc".to_string()),
                Some(ReceiverType::RefMut) => {
                    conv.push_str(&format!(
                        "\tlet parent_mut_ptr = Arc::as_ptr(&parent_arc) as *mut {parent};\n\
                        \tlet parent_mut = unsafe {{ &mut *parent_mut_ptr }};\n"
                    ));
                    call_args.push("parent_mut".to_string());
                }
                Some(ReceiverType::Owned) | None => {
                    conv.push_str("\tHandleRegistry::global().remove(handle_id);\n\
                        \tlet parent_owned = Arc::try_unwrap(parent_arc).ok().expect(\"Cannot consume handle\");\n");
                    call_args.push("parent_owned".to_string());
                }
            }
        }

        for p in &f.params {
            let r_name = format!("r_{}", p.name);
            match &p.ty {
                FFIType::Bool
                | FFIType::I8
                | FFIType::I16
                | FFIType::I32
                | FFIType::I64
                | FFIType::U8
                | FFIType::U16
                | FFIType::U32
                | FFIType::U64
                | FFIType::F32
                | FFIType::F64 => {
                    call_args.push(p.name.clone());
                }
                FFIType::String => {
                    conv.push_str(&format!(
                        "\tlet {} = CStr::from_ptr({}).to_str().expect(\"Invalid UTF-8\").to_string();\n",
                        r_name, p.name
                    ));
                    call_args.push(r_name);
                }
                FFIType::Bytes => {
                    conv.push_str(&format!(
                        "\tlet {} = std::slice::from_raw_parts({}, {}_len);\n",
                        r_name, p.name, p.name
                    ));
                    call_args.push(r_name);
                }
                _ => {
                    conv.push_str(&format!(
                        "\tlet {}_slice = std::slice::from_raw_parts({}, {}_len);\n\
                        \tlet {} = postcard::from_bytes({}_slice).expect(\"Postcard deserialization failed\");\n",
                        p.name, p.name, p.name, r_name, p.name
                    ));
                    call_args.push(r_name);
                }
            }
        }

        cabi_src.push_str(&conv);

        let rust_call_prefix = match &f.parent_struct {
            Some(s) => {
                if f.receiver.is_some() {
                    match f.receiver {
                        Some(ReceiverType::Ref) => format!("parent_arc.{}", f.rust_name),
                        Some(ReceiverType::RefMut) => format!("parent_mut.{}", f.rust_name),
                        Some(ReceiverType::Owned) => format!("parent_owned.{}", f.rust_name),
                        None => unreachable!(),
                    }
                } else {
                    format!("{}::{}", s, f.rust_name)
                }
            }
            None => f.rust_name.clone(),
        };

        cabi_src.push_str(&format!(
            "\tlet res = catch_panic(|| {{\n\
            \t\t{}({})\n\
            \t}});\n",
            rust_call_prefix,
            call_args.join(", ")
        ));

        cabi_src.push_str("\tmatch res {\n");
        cabi_src.push_str("\t\tOk(val) => {\n");
        cabi_src.push_str(&format!(
            "\t\t\t{}\n",
            format_cabi_rust_return_conversion(&f.ret_ty, ir)
        ));
        cabi_src.push_str("\t\t}\n");
        cabi_src.push_str("\t\tErr(panic_msg) => {\n");
        cabi_src.push_str("\t\t\teprintln!(\"Rust Panic: {panic_msg}\");\n");
        cabi_src.push_str(&format!(
            "\t\t\t{}\n",
            format_cabi_rust_default_return(&f.ret_ty, ir)
        ));
        cabi_src.push_str("\t\t}\n");
        cabi_src.push_str("\t}\n");
        cabi_src.push_str("}\n\n");
    }

    fs::write(rust_glue_dir.join("cabi_glue.rs"), cabi_src)?;

    Ok(())
}

fn format_jni_rust_params(params: &[crate::parser::ParamInfo], has_receiver: bool) -> String {
    let mut list = Vec::new();
    if has_receiver {
        list.push("handle_id: jlong".to_string());
    }

    for p in params {
        let jni_type = match &p.ty {
            FFIType::Bool => "jboolean",
            FFIType::I8 | FFIType::U8 => "jbyte",
            FFIType::I16 | FFIType::U16 => "jshort",
            FFIType::I32 | FFIType::U32 => "jint",
            FFIType::I64 | FFIType::U64 => "jlong",
            FFIType::F32 => "jfloat",
            FFIType::F64 => "jdouble",
            FFIType::String => "JString",
            FFIType::Bytes => "JByteArray",
            _ => "JByteArray", // Serialized types
        };
        list.push(format!("{}: {}", p.name, jni_type));
    }

    list.join(",\n\t")
}

fn to_jni_rust_ret_type(ty: &FFIType, ir: &CrateInterface) -> String {
    match ty {
        FFIType::Bool => "jboolean".to_string(),
        FFIType::I8 | FFIType::U8 => "jbyte".to_string(),
        FFIType::I16 | FFIType::U16 => "jshort".to_string(),
        FFIType::I32 | FFIType::U32 => "jint".to_string(),
        FFIType::I64 | FFIType::U64 => "jlong".to_string(),
        FFIType::F32 => "jfloat".to_string(),
        FFIType::F64 => "jdouble".to_string(),
        FFIType::Unit => "()".to_string(),
        FFIType::String => "sys::jstring".to_string(),
        FFIType::Bytes => "sys::jbyteArray".to_string(),
        FFIType::Custom(name) if is_opaque_type(name, ir) => "jlong".to_string(),
        _ => "sys::jbyteArray".to_string(), // Serialized types
    }
}

fn format_jni_rust_return_conversion(ty: &FFIType, ir: &CrateInterface) -> String {
    match ty {
        FFIType::Bool => "val as jboolean".to_string(),
        FFIType::I8 | FFIType::U8 => "val as jbyte".to_string(),
        FFIType::I16 | FFIType::U16 => "val as jshort".to_string(),
        FFIType::I32 | FFIType::U32 => "val as jint".to_string(),
        FFIType::I64 | FFIType::U64 => "val as jlong".to_string(),
        FFIType::F32 => "val as jfloat".to_string(),
        FFIType::F64 => "val as jdouble".to_string(),
        FFIType::Unit => "()".to_string(),
        FFIType::String => {
            "let jstr = env.new_string(val).expect(\"Cannot create JVM string\");\n\
            \t\t\tjstr.into_raw()"
                .to_string()
        }
        FFIType::Bytes => {
            "let jarr = env.byte_array_from_slice(&val).expect(\"Cannot create JVM byte array\");\n\
            \t\t\tjarr.into_raw()"
                .to_string()
        }
        FFIType::Custom(name) if is_opaque_type(name, ir) => {
            "let handle = koffi_runtime::HandleRegistry::global().insert(val);\n\
                \t\t\thandle as jlong"
                .to_string()
        }
        _ => {
            // Serialized type
            "let bytes = postcard::to_allocvec(&val).expect(\"Postcard serialization failed\");\n\
            \t\t\tlet jarr = env.byte_array_from_slice(&bytes).expect(\"Cannot create JVM byte array\");\n\
            \t\t\tjarr.into_raw()"
                .to_string()
        }
    }
}

fn format_jni_rust_default_return(ty: &FFIType, ir: &CrateInterface) -> String {
    match ty {
        FFIType::Bool => "0".to_string(),
        FFIType::I8 | FFIType::U8 => "0".to_string(),
        FFIType::I16 | FFIType::U16 => "0".to_string(),
        FFIType::I32 | FFIType::U32 => "0".to_string(),
        FFIType::I64 | FFIType::U64 => "0".to_string(),
        FFIType::F32 => "0.0".to_string(),
        FFIType::F64 => "0.0".to_string(),
        FFIType::Unit => "()".to_string(),
        FFIType::Custom(name) if is_opaque_type(name, ir) => "0".to_string(),
        _ => "std::ptr::null_mut()".to_string(),
    }
}

fn is_opaque_type(name: &str, ir: &CrateInterface) -> bool {
    ir.structs.iter().any(|s| s.name == name && s.is_opaque)
}

fn format_cabi_rust_params(params: &[crate::parser::ParamInfo], has_receiver: bool) -> String {
    let mut list = Vec::new();
    if has_receiver {
        list.push("handle_id: u64".to_string());
    }

    for p in params {
        let c_type = match &p.ty {
            FFIType::Bool => "bool",
            FFIType::I8 => "i8",
            FFIType::I16 => "i16",
            FFIType::I32 => "i32",
            FFIType::I64 => "i64",
            FFIType::U8 => "u8",
            FFIType::U16 => "u16",
            FFIType::U32 => "u32",
            FFIType::U64 => "u64",
            FFIType::F32 => "f32",
            FFIType::F64 => "f64",
            FFIType::String => "*const c_char",
            FFIType::Bytes => "*const u8",
            _ => "*const u8",
        };
        list.push(format!("{}: {}", p.name, c_type));
        if p.ty == FFIType::Bytes
            || matches!(
                p.ty,
                FFIType::Option(_)
                    | FFIType::Result(_, _)
                    | FFIType::Vec(_)
                    | FFIType::Map(_, _)
                    | FFIType::Custom(_)
            )
        {
            list.push(format!("{}_len: usize", p.name));
        }
    }

    list.join(",\n\t")
}

fn to_cabi_rust_ret_type(ty: &FFIType, ir: &CrateInterface) -> String {
    match ty {
        FFIType::Bool => "bool".to_string(),
        FFIType::I8 => "i8".to_string(),
        FFIType::I16 => "i16".to_string(),
        FFIType::I32 => "i32".to_string(),
        FFIType::I64 => "i64".to_string(),
        FFIType::U8 => "u8".to_string(),
        FFIType::U16 => "u16".to_string(),
        FFIType::U32 => "u32".to_string(),
        FFIType::U64 => "u64".to_string(),
        FFIType::F32 => "f32".to_string(),
        FFIType::F64 => "f64".to_string(),
        FFIType::Unit => "()".to_string(),
        FFIType::String => "*mut c_char".to_string(),
        FFIType::Custom(name) if is_opaque_type(name, ir) => "u64".to_string(),
        _ => "KoffiByteBuf".to_string(),
    }
}

fn format_cabi_rust_return_conversion(ty: &FFIType, ir: &CrateInterface) -> String {
    match ty {
        FFIType::Bool
        | FFIType::I8
        | FFIType::I16
        | FFIType::I32
        | FFIType::I64
        | FFIType::U8
        | FFIType::U16
        | FFIType::U32
        | FFIType::U64
        | FFIType::F32
        | FFIType::F64
        | FFIType::Unit => "val".to_string(),
        FFIType::String => {
            "let c_str = std::ffi::CString::new(val).unwrap();\n\
            \t\t\tc_str.into_raw()"
                .to_string()
        }
        FFIType::Custom(name) if is_opaque_type(name, ir) => {
            "let id = HandleRegistry::global().insert(val);\n\
            \t\t\tid"
                .to_string()
        }
        _ => {
            "let bytes = postcard::to_allocvec(&val).unwrap();\n\
            \t\t\tKoffiByteBuf::new(bytes)"
                .to_string()
        }
    }
}

fn format_cabi_rust_default_return(ty: &FFIType, ir: &CrateInterface) -> String {
    match ty {
        FFIType::Bool => "false".to_string(),
        FFIType::I8
        | FFIType::U8
        | FFIType::I16
        | FFIType::U16
        | FFIType::I32
        | FFIType::U32
        | FFIType::I64
        | FFIType::U64 => "0".to_string(),
        FFIType::F32 | FFIType::F64 => "0.0".to_string(),
        FFIType::Unit => "()".to_string(),
        FFIType::String => "std::ptr::null_mut()".to_string(),
        FFIType::Custom(name) if is_opaque_type(name, ir) => "0".to_string(),
        _ => "KoffiByteBuf { ptr: std::ptr::null_mut(), len: 0, cap: 0 }".to_string(),
    }
}
