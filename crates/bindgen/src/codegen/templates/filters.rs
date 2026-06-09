use koffi_ir::{FFIType, FnInfo, ParamInfo, TypeRef};

use crate::codegen::templates::util;

#[askama::filter_fn]
pub fn kotlin_type(ty: &FFIType, _env: &dyn askama::Values) -> askama::Result<String> {
    util::kotlin_type(ty)
}

#[askama::filter_fn]
pub fn jni_type(ty: &FFIType, _env: &dyn askama::Values) -> askama::Result<String> {
    util::jni_type(ty)
}

#[askama::filter_fn]
pub fn jni_rust_type(ty: &FFIType, _env: &dyn askama::Values) -> askama::Result<String> {
    util::jni_rust_type(ty)
}

#[askama::filter_fn]
pub fn c_type(ty: &FFIType, _env: &dyn askama::Values) -> askama::Result<String> {
    util::c_type(ty)
}

#[askama::filter_fn]
pub fn camel_case(s: &str, _env: &dyn askama::Values) -> askama::Result<String> {
    util::camel_case(s)
}

#[askama::filter_fn]
pub fn pascal_case(s: &str, _env: &dyn askama::Values) -> askama::Result<String> {
    util::pascal_case(s)
}

#[askama::filter_fn]
pub fn snake_case(s: &str, _env: &dyn askama::Values) -> askama::Result<String> {
    util::snake_case(s)
}

#[askama::filter_fn]
pub fn jni_symbol(
    namespace: &str,
    _env: &dyn askama::Values,
    pkg_pascal: &str,
    method: &str,
) -> askama::Result<String> {
    util::jni_symbol(namespace, pkg_pascal, method)
}

#[askama::filter_fn]
pub fn params_common(
    params: &[koffi_ir::ParamInfo],
    _env: &dyn askama::Values,
) -> askama::Result<String> {
    util::params_common(params)
}

#[askama::filter_fn]
pub fn params_jni(
    params: &[koffi_ir::ParamInfo],
    _env: &dyn askama::Values,
) -> askama::Result<String> {
    util::params_jni(params)
}

#[askama::filter_fn]
pub fn params_jni_rust(
    params: &[koffi_ir::ParamInfo],
    _env: &dyn askama::Values,
    has_receiver: bool,
) -> askama::Result<String> {
    util::params_jni_rust(params, has_receiver)
}

#[askama::filter_fn]
pub fn async_kw(is_async: &bool, _env: &dyn askama::Values) -> askama::Result<&'static str> {
    util::async_kw(is_async)
}

#[askama::filter_fn]
pub fn jni_method_name(
    parent: &Option<String>,
    _env: &dyn askama::Values,
    rust_name: &str,
) -> askama::Result<String> {
    util::jni_method_name(parent, rust_name)
}

#[askama::filter_fn]
pub fn jni_return_expr(
    function: &FnInfo,
    _env: &dyn askama::Values,
    pkg_pascal: &str,
    jni_name: &str,
    has_receiver: bool,
) -> askama::Result<String> {
    util::jni_return_expr(
        pkg_pascal,
        jni_name,
        &function.params,
        &function.ret_ty,
        has_receiver,
    )
}

#[askama::filter_fn]
pub fn jni_return_conversion(ty: &FFIType, _env: &dyn askama::Values) -> askama::Result<String> {
    Ok(util::jni_rust_return_conversion(ty))
}

#[askama::filter_fn]
pub fn jni_default_return(ret: &FFIType, _env: &dyn askama::Values) -> askama::Result<String> {
    Ok(util::jni_rust_default_return(ret))
}

#[askama::filter_fn]
pub fn call_args(params: &[ParamInfo], _env: &dyn askama::Values) -> askama::Result<String> {
    Ok(util::call_args(params))
}

#[askama::filter_fn]
pub fn qualified_name(ty: &TypeRef, _env: &dyn askama::Values) -> askama::Result<String> {
    Ok(ty.qualified_name())
}
