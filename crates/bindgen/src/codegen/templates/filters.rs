#![allow(clippy::inline_always)]

use koffi_ir::{FFIType, FnInfo, ParamInfo};

use crate::codegen::templates::{c, kotlin, rust, util};

#[askama::filter_fn]
pub fn camel_case(s: &str, _env: &dyn askama::Values) -> askama::Result<String> {
    Ok(util::camel_case(s))
}

#[askama::filter_fn]
pub fn pascal_case(s: &str, _env: &dyn askama::Values) -> askama::Result<String> {
    Ok(util::pascal_case(s))
}

#[askama::filter_fn]
pub fn snake_case(s: &str, _env: &dyn askama::Values) -> askama::Result<String> {
    Ok(util::snake_case(s))
}

#[askama::filter_fn]
pub fn schema_hash_hex(ty: &FFIType, _env: &dyn askama::Values) -> askama::Result<String> {
    Ok(util::schema_hash_hex(ty))
}

#[askama::filter_fn]
pub fn kotlin_type(ty: &FFIType, _env: &dyn askama::Values) -> askama::Result<String> {
    Ok(kotlin::kotlin_type(ty))
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
) -> askama::Result<String> {
    Ok(kotlin::kotlin_params_jni(params))
}

#[askama::filter_fn]
pub const fn kotlin_async_kw(
    is_async: &bool,
    _env: &dyn askama::Values,
) -> askama::Result<&'static str> {
    Ok(kotlin::kotlin_async_kw(is_async))
}

#[askama::filter_fn]
pub fn kotlin_jni_method_name(
    parent: &Option<String>,
    _env: &dyn askama::Values,
    rust_name: &str,
) -> askama::Result<String> {
    Ok(kotlin::kotlin_jni_method_name(parent, rust_name))
}

#[askama::filter_fn]
pub fn kotlin_jni_return_expr(
    params: &[ParamInfo],
    _env: &dyn askama::Values,
    pkg: &str,
    jni_name: &str,
    ret: &FFIType,
    has_receiver: bool,
) -> askama::Result<String> {
    Ok(kotlin::kotlin_jni_return_expr(
        params,
        pkg,
        jni_name,
        ret,
        has_receiver,
    ))
}

#[askama::filter_fn]
pub fn kotlin_sanitize_ident(ident: &str, _env: &dyn askama::Values) -> askama::Result<String> {
    Ok(kotlin::kotlin_sanitize_ident(ident))
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
pub fn rust_jni_type(ty: &FFIType, _env: &dyn askama::Values) -> askama::Result<String> {
    Ok(rust::rust_jni_type(ty))
}

#[askama::filter_fn]
pub fn rust_params_jni(
    params: &[ParamInfo],
    _env: &dyn askama::Values,
    has_receiver: bool,
    namespace: &str,
) -> askama::Result<String> {
    Ok(rust::rust_params_jni(params, has_receiver, namespace))
}

#[askama::filter_fn]
pub fn rust_prefix_ident(
    ident: &str,
    _env: &dyn askama::Values,
    namespace: &str,
) -> askama::Result<String> {
    Ok(rust::rust_prefix_ident(ident, namespace))
}

#[askama::filter_fn]
pub fn rust_prefix_param(
    param: &str,
    _env: &dyn askama::Values,
    namespace: &str,
) -> askama::Result<String> {
    Ok(rust::rust_prefix_param(param, namespace))
}

#[askama::filter_fn]
pub fn rust_prefix_native(
    native: &str,
    _env: &dyn askama::Values,
    namespace: &str,
) -> askama::Result<String> {
    Ok(rust::rust_prefix_native(native, namespace))
}

#[askama::filter_fn]
pub fn rust_call_args(
    params: &[ParamInfo],
    _env: &dyn askama::Values,
    namespace: &str,
) -> askama::Result<String> {
    Ok(rust::rust_call_args(params, namespace))
}

#[askama::filter_fn]
pub fn rust_c_native_type(ty: &FFIType, _env: &dyn askama::Values) -> askama::Result<String> {
    Ok(rust::rust_c_native_type(ty))
}

#[askama::filter_fn]
pub fn rust_c_native_return_type(
    ty: &FFIType,
    _env: &dyn askama::Values,
) -> askama::Result<String> {
    Ok(rust::rust_c_native_return_type(ty))
}

#[askama::filter_fn]
pub fn c_type(ty: &FFIType, _env: &dyn askama::Values) -> askama::Result<String> {
    Ok(c::c_type(ty))
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
pub fn c_return_type(ty: &FFIType, _env: &dyn askama::Values) -> askama::Result<String> {
    Ok(c::c_return_type(ty))
}
