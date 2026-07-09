#![allow(clippy::inline_always)]

use koffi_ir::{EnumInfo, FFIType, FnInfo, ParamInfo, StructInfo};

use crate::codegen::templates::{c, kotlin, rust};

#[askama::filter_fn]
pub fn camel_case(s: &str, _env: &dyn askama::Values) -> askama::Result<String> {
    Ok(heck::AsLowerCamelCase(s).to_string())
}

#[askama::filter_fn]
pub fn pascal_case(s: &str, _env: &dyn askama::Values) -> askama::Result<String> {
    Ok(heck::AsPascalCase(s).to_string())
}

#[askama::filter_fn]
pub fn snake_case(s: &str, _env: &dyn askama::Values) -> askama::Result<String> {
    Ok(heck::AsSnakeCase(s).to_string())
}

#[askama::filter_fn]
pub fn kotlin_type(ty: &FFIType, _env: &dyn askama::Values) -> askama::Result<String> {
    Ok(kotlin::kotlin_type(ty))
}

#[askama::filter_fn]
pub fn kotlin_writer_expr(
    ty: &FFIType,
    _env: &dyn askama::Values,
    val: &str,
) -> askama::Result<String> {
    Ok(kotlin::kotlin_writer_expr(ty, val))
}

#[askama::filter_fn]
pub fn kotlin_reader_expr(ty: &FFIType, _env: &dyn askama::Values) -> askama::Result<String> {
    Ok(kotlin::kotlin_reader_expr(ty))
}

#[askama::filter_fn]
pub fn kotlin_jni_type(ty: &FFIType, _env: &dyn askama::Values) -> askama::Result<String> {
    Ok(kotlin::kotlin_jni_type(ty))
}

#[askama::filter_fn]
pub fn kotlin_params_common(
    params: &[ParamInfo],
    _env: &dyn askama::Values,
) -> askama::Result<String> {
    Ok(kotlin::kotlin_params_common(params))
}

#[askama::filter_fn]
pub fn kotlin_params_jni(
    params: &[ParamInfo],
    _env: &dyn askama::Values,
    has_receiver: &bool,
) -> askama::Result<String> {
    Ok(kotlin::kotlin_params_jni(params, *has_receiver))
}

#[askama::filter_fn]
pub fn kotlin_async_kw(is_async: &bool, _env: &dyn askama::Values) -> askama::Result<&'static str> {
    Ok(kotlin::kotlin_async_kw(*is_async))
}

#[askama::filter_fn]
pub fn kotlin_jni_return_expr(
    f: &FnInfo,
    _env: &dyn askama::Values,
    pkg_pascal: &str,
    jni_name: &str,
) -> askama::Result<String> {
    Ok(kotlin::kotlin_jni_return_expr(f, pkg_pascal, jni_name))
}

#[askama::filter_fn]
pub fn kotlin_sanitize_ident(ident: &str, _env: &dyn askama::Values) -> askama::Result<String> {
    Ok(kotlin::kotlin_sanitize_ident(ident))
}

#[askama::filter_fn]
pub fn kotlin_jni_method_name(f: &FnInfo, _env: &dyn askama::Values) -> askama::Result<String> {
    Ok(kotlin::kotlin_jni_method_name(f))
}

#[askama::filter_fn]
pub fn kotlin_native_return_expr(
    f: &FnInfo,
    _env: &dyn askama::Values,
    c_sym: &str,
) -> askama::Result<String> {
    Ok(kotlin::kotlin_native_return_expr(f, c_sym))
}

#[askama::filter_fn]
pub fn kotlin_params_wasm_external(
    params: &[ParamInfo],
    _env: &dyn askama::Values,
) -> askama::Result<String> {
    Ok(kotlin::kotlin_params_wasm_external(params))
}

#[askama::filter_fn]
pub fn kotlin_wasm_external_type(
    ty: &FFIType,
    _env: &dyn askama::Values,
) -> askama::Result<String> {
    Ok(kotlin::kotlin_wasm_external_type(ty))
}

#[askama::filter_fn]
pub fn kotlin_wasm_return_expr(
    f: &FnInfo,
    _env: &dyn askama::Values,
    js_name: &str,
) -> askama::Result<String> {
    Ok(kotlin::kotlin_wasm_return_expr(f, js_name))
}

#[askama::filter_fn]
pub fn c_abi_symbol(
    f: &FnInfo,
    _env: &dyn askama::Values,
    crate_ident: &str,
) -> askama::Result<String> {
    Ok(c::c_abi_symbol(f, crate_ident))
}

#[askama::filter_fn]
pub fn c_type(ty: &FFIType, _env: &dyn askama::Values) -> askama::Result<String> {
    Ok(c::c_type(ty))
}

#[askama::filter_fn]
pub fn c_return_type(ty: &FFIType, _env: &dyn askama::Values) -> askama::Result<String> {
    Ok(c::c_return_type(ty))
}

#[askama::filter_fn]
pub fn rust_primitive_type(ty: &FFIType, _env: &dyn askama::Values) -> askama::Result<String> {
    Ok(rust::rust_primitive_type(ty))
}

#[askama::filter_fn]
pub fn rust_jni_type(ty: &FFIType, _env: &dyn askama::Values) -> askama::Result<String> {
    Ok(rust::rust_jni_type(ty))
}

#[askama::filter_fn]
pub fn rust_params_jni(
    params: &[ParamInfo],
    _env: &dyn askama::Values,
    has_receiver: &bool,
) -> askama::Result<String> {
    Ok(rust::rust_params_jni(params, *has_receiver))
}

#[askama::filter_fn]
pub fn rust_cabi_type(ty: &FFIType, _env: &dyn askama::Values) -> askama::Result<String> {
    Ok(rust::rust_cabi_type(ty))
}

#[askama::filter_fn]
pub fn rust_cabi_return_type(ty: &FFIType, _env: &dyn askama::Values) -> askama::Result<String> {
    Ok(rust::rust_cabi_return_type(ty))
}

#[askama::filter_fn]
pub fn rust_params_wasm(params: &[ParamInfo], _env: &dyn askama::Values) -> askama::Result<String> {
    Ok(rust::rust_params_wasm(params))
}

#[askama::filter_fn]
pub fn rust_wasm_return_type(ty: &FFIType, _env: &dyn askama::Values) -> askama::Result<String> {
    Ok(rust::rust_wasm_return_type(ty))
}

#[askama::filter_fn]
pub fn wasm_rust_fn_ident(
    f: &FnInfo,
    _env: &dyn askama::Values,
    crate_ident: &str,
) -> askama::Result<String> {
    Ok(rust::wasm_rust_fn_ident(f, crate_ident))
}

#[askama::filter_fn]
pub fn rust_fn_use_path(f: &FnInfo, _env: &dyn askama::Values) -> askama::Result<String> {
    Ok(rust::rust_fn_use_path(f))
}

#[askama::filter_fn]
pub fn rust_fn_alias(f: &FnInfo, _env: &dyn askama::Values) -> askama::Result<String> {
    Ok(rust::rust_fn_alias(f))
}

#[askama::filter_fn]
pub fn rust_struct_use_path(s: &StructInfo, _env: &dyn askama::Values) -> askama::Result<String> {
    Ok(rust::rust_struct_use_path(s))
}

#[askama::filter_fn]
pub fn rust_struct_alias(s: &StructInfo, _env: &dyn askama::Values) -> askama::Result<String> {
    Ok(rust::rust_struct_alias(s))
}

#[askama::filter_fn]
pub fn rust_enum_use_path(e: &EnumInfo, _env: &dyn askama::Values) -> askama::Result<String> {
    Ok(rust::rust_enum_use_path(e))
}

#[askama::filter_fn]
pub fn rust_enum_alias(e: &EnumInfo, _env: &dyn askama::Values) -> askama::Result<String> {
    Ok(rust::rust_enum_alias(e))
}

#[askama::filter_fn]
pub fn rust_parent_use_path(f: &FnInfo, _env: &dyn askama::Values) -> askama::Result<String> {
    Ok(rust::rust_parent_use_path(f))
}

#[askama::filter_fn]
pub fn rust_use_path_with_parent(f: &FnInfo, _env: &dyn askama::Values) -> askama::Result<String> {
    Ok(rust::rust_use_path_with_parent(f))
}

#[askama::filter_fn]
pub fn rust_parent_alias(f: &FnInfo, _env: &dyn askama::Values) -> askama::Result<String> {
    Ok(rust::rust_parent_alias(f))
}

#[askama::filter_fn]
pub fn rust_call_args(params: &[ParamInfo], _env: &dyn askama::Values) -> askama::Result<String> {
    Ok(rust::rust_call_args(params))
}
