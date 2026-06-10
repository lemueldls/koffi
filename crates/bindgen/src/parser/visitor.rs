//! Phase 1 parser: syn-based attribute harvesting, namespace resolution,
//! and Rust module-path tracking.
//!
//! ## Two sub-passes
//!
//! **Sub-pass A** (`collect_type_declarations`) is a lightweight traversal that
//! builds a map of every `#[koffi::opaque]` and `#[koffi::data]` type name.
//! This is needed before function signatures can be resolved, because a
//! function may reference a type declared in a different file.
//!
//! **Sub-pass B** (`visit_items`) is the full parse that records structs, enums,
//! and functions into [`ParseContext`], resolving namespaces and module paths
//! as it goes.
//!
//! The resulting `PartialInterface` carries:
//! - `TypeRef`s with empty `crate_id` (filled by Phase 2).
//! - `rust_module_path` on every item (used by codegen for `use` paths and unique C/JNI symbol names).

use std::{
    collections::{HashMap, HashSet},
    fs, io,
    path::{Path, PathBuf},
};

use heck::AsLowerCamelCase;
use koffi_ir::{
    CrateId, EnumInfo, EnumVariantInfo, ExportArgs, FFIType, FieldInfo, FnInfo, ParamInfo,
    ReceiverType, StructInfo, TypeRef,
};

use crate::{
    BindgenError,
    parser::context::{ParseContext, TypeDeclarationMap},
};

/// Output of Phase 1: all collected items with placeholder `TypeRef`s.
#[derive(Debug)]
pub struct PartialInterface {
    pub namespace: String,
    pub crate_name: String,
    pub version: String,
    pub structs: Vec<StructInfo>,
    pub enums: Vec<EnumInfo>,
    pub functions: Vec<FnInfo>,
}

/// Run the full Phase 1 parse against a crate rooted at `crate_path`.
pub fn parse_syn(
    crate_path: &Path,
    crate_namespace: String,
    crate_name: String,
    version: String,
) -> Result<PartialInterface, BindgenError> {
    let src_dir = crate_path.join("src");

    let entry = entry_point(&src_dir).ok_or_else(|| {
        BindgenError::IoError(io::Error::new(
            io::ErrorKind::NotFound,
            format!("No lib.rs or main.rs in {}", src_dir.display()),
        ))
    })?;

    let type_decls = collect_type_declarations(&entry, &src_dir)?;

    let mut ctx = ParseContext::new(crate_namespace.clone(), type_decls);

    {
        let source = fs::read_to_string(&entry)?;
        let file = syn::parse_file(&source)?;

        // A file-level inner attribute `#![koffi::namespace("...")]` on lib.rs
        // is treated as a crate-level override (rare; outer attr on mod preferred).
        if let Some(ns) = file_level_namespace(&file.attrs) {
            ctx.push_namespace(ns);
        }

        ctx.file_stack.push(entry);
        visit_items(&file.items, &mut ctx, &src_dir)?;
        ctx.file_stack.pop();
    }

    Ok(PartialInterface {
        namespace: crate_namespace,
        crate_name,
        version,
        structs: ctx.structs,
        enums: ctx.enums,
        functions: ctx.functions,
    })
}

/// Walk the entire source tree starting from `entry` and collect every type
/// annotated with `#[koffi::opaque]` or `#[koffi::data]`.
///
/// Returns `HashMap<type_name, is_opaque>`.
pub fn collect_type_declarations(
    entry: &Path,
    src_dir: &Path,
) -> Result<TypeDeclarationMap, BindgenError> {
    let mut decls = HashMap::new();
    let mut visited: HashSet<PathBuf> = HashSet::new();
    collect_in_file(entry, src_dir, &mut decls, &mut visited)?;

    Ok(decls)
}

fn collect_in_file(
    file_path: &Path,
    src_dir: &Path,
    decls: &mut TypeDeclarationMap,
    visited: &mut HashSet<PathBuf>,
) -> Result<(), BindgenError> {
    let canonical = file_path
        .canonicalize()
        .unwrap_or_else(|_| file_path.to_path_buf());
    if !visited.insert(canonical) {
        return Ok(()); // cycle guard
    }

    let source = fs::read_to_string(file_path).map_err(|e| {
        BindgenError::IoError(io::Error::new(
            e.kind(),
            format!("{}: {}", file_path.display(), e),
        ))
    })?;
    let file = syn::parse_file(&source)?;

    collect_in_items(&file.items, src_dir, decls, visited)
}

fn collect_in_items(
    items: &[syn::Item],
    src_dir: &Path,
    decls: &mut TypeDeclarationMap,
    visited: &mut HashSet<PathBuf>,
) -> Result<(), BindgenError> {
    for item in items {
        match item {
            syn::Item::Struct(s) => {
                let is_opaque = has_koffi_attr(&s.attrs, "opaque");
                let is_data = has_koffi_attr(&s.attrs, "data");
                if is_opaque || is_data {
                    decls.insert(s.ident.to_string(), is_opaque);
                }
            }
            syn::Item::Enum(e) => {
                if has_koffi_attr(&e.attrs, "data") {
                    decls.insert(e.ident.to_string(), false);
                }
            }
            syn::Item::Mod(m) => {
                if let Some((_, inner)) = &m.content {
                    collect_in_items(inner, src_dir, decls, visited)?;
                } else {
                    let mod_name = m.ident.to_string();
                    if let Some((child_path, child_src)) = resolve_mod_file(src_dir, &mod_name) {
                        collect_in_file(&child_path, &child_src, decls, visited)?;
                    }
                }
            }
            _ => {}
        }
    }

    Ok(())
}

fn visit_items(
    items: &[syn::Item],
    ctx: &mut ParseContext,
    src_dir: &Path,
) -> Result<(), BindgenError> {
    for item in items {
        match item {
            syn::Item::Mod(m) => visit_mod(m, ctx, src_dir)?,
            syn::Item::Struct(s) => visit_struct(s, ctx)?,
            syn::Item::Enum(e) => visit_enum(e, ctx)?,
            syn::Item::Fn(f) => visit_fn(f, ctx)?,
            syn::Item::Impl(i) => visit_impl(i, ctx)?,
            _ => {}
        }
    }

    Ok(())
}

fn visit_mod(m: &syn::ItemMod, ctx: &mut ParseContext, src_dir: &Path) -> Result<(), BindgenError> {
    let mod_name = m.ident.to_string();

    // An outer `#[koffi::namespace("...")]` on the mod declaration overrides
    // the namespace for the entire subtree.
    let ns_override = attrs_get_namespace(&m.attrs);
    if let Some(ref ns) = ns_override {
        ctx.push_namespace(ns.clone());
    }

    // Always push the module name onto the Rust path stack.
    ctx.push_module(mod_name.clone());

    match &m.content {
        // Inline module: recurse directly.
        Some((_, inner_items)) => {
            visit_items(inner_items, ctx, src_dir)?;
        }

        // File-backed module: find the file and recurse.
        None => {
            if let Some((child_path, child_src_dir)) = resolve_mod_file(src_dir, &mod_name) {
                let source = fs::read_to_string(&child_path).map_err(|e| {
                    BindgenError::IoError(io::Error::new(
                        e.kind(),
                        format!("{}: {}", child_path.display(), e),
                    ))
                })?;
                let file = syn::parse_file(&source)?;

                // A file-level inner namespace attr applies if no outer attr was found
                // on the mod declaration in the parent file.
                let child_ns = if ns_override.is_none() {
                    file_level_namespace(&file.attrs)
                } else {
                    None
                };
                if let Some(ref ns) = child_ns {
                    ctx.push_namespace(ns.clone());
                }

                ctx.file_stack.push(child_path);
                visit_items(&file.items, ctx, &child_src_dir)?;
                ctx.file_stack.pop();

                if child_ns.is_some() {
                    ctx.pop_namespace();
                }
            }
            // Unknown module (external, generated, or cfg-gated). skip silently.
        }
    }

    ctx.pop_module();

    if ns_override.is_some() {
        ctx.pop_namespace();
    }

    Ok(())
}

fn visit_struct(s: &syn::ItemStruct, ctx: &mut ParseContext) -> Result<(), BindgenError> {
    let is_opaque = has_koffi_attr(&s.attrs, "opaque");
    let is_data = has_koffi_attr(&s.attrs, "data");

    if !is_opaque && !is_data {
        return Ok(());
    }

    let name = s.ident.to_string();
    let namespace = ctx.current_namespace().to_string();
    let rust_module_path = ctx.current_module_path();
    let doc = extract_doc(&s.attrs);

    // Opaque structs expose no fields to the Kotlin side.
    // Data structs expose all fields that serde will serialize.
    let fields = if is_data {
        parse_struct_fields(&s.fields, &ctx.type_decls)?
    } else {
        Vec::new()
    };

    ctx.structs.push(StructInfo {
        name,
        is_opaque,
        fields,
        namespace,
        rust_module_path,
        doc,
    });

    Ok(())
}

fn parse_struct_fields(
    fields: &syn::Fields,
    type_decls: &TypeDeclarationMap,
) -> Result<Vec<FieldInfo>, BindgenError> {
    let mut result = Vec::new();
    match fields {
        syn::Fields::Named(named) => {
            for field in &named.named {
                let skip_serde = has_serde_skip(&field.attrs);
                let name = field
                    .ident
                    .as_ref()
                    .map(|id| id.to_string())
                    .unwrap_or_default();
                let ty = parse_type(&field.ty, type_decls)?;
                result.push(FieldInfo {
                    name,
                    ty,
                    skip_serde,
                });
            }
        }
        syn::Fields::Unnamed(unnamed) => {
            for (idx, field) in unnamed.unnamed.iter().enumerate() {
                let skip_serde = has_serde_skip(&field.attrs);
                let ty = parse_type(&field.ty, type_decls)?;
                result.push(FieldInfo {
                    name: idx.to_string(),
                    ty,
                    skip_serde,
                });
            }
        }
        syn::Fields::Unit => {}
    }
    Ok(result)
}

fn visit_enum(e: &syn::ItemEnum, ctx: &mut ParseContext) -> Result<(), BindgenError> {
    if !has_koffi_attr(&e.attrs, "data") {
        return Ok(());
    }

    let name = e.ident.to_string();
    let namespace = ctx.current_namespace().to_string();
    let rust_module_path = ctx.current_module_path();
    let doc = extract_doc(&e.attrs);
    let mut variants = Vec::new();

    for variant in &e.variants {
        let v_name = variant.ident.to_string();
        let v_doc = extract_doc(&variant.attrs);
        let v_fields = match &variant.fields {
            syn::Fields::Named(named) => {
                named
                    .named
                    .iter()
                    .map(|f| {
                        let name = f.ident.as_ref().map(|i| i.to_string()).unwrap_or_default();
                        let ty = parse_type(&f.ty, &ctx.type_decls)?;
                        let skip = has_serde_skip(&f.attrs);
                        Ok(FieldInfo {
                            name,
                            ty,
                            skip_serde: skip,
                        })
                    })
                    .collect::<Result<Vec<_>, BindgenError>>()?
            }
            syn::Fields::Unnamed(unnamed) => {
                unnamed
                    .unnamed
                    .iter()
                    .enumerate()
                    .map(|(i, f)| {
                        let ty = parse_type(&f.ty, &ctx.type_decls)?;
                        let skip = has_serde_skip(&f.attrs);
                        Ok(FieldInfo {
                            name: format!("field{i}"),
                            ty,
                            skip_serde: skip,
                        })
                    })
                    .collect::<Result<Vec<_>, BindgenError>>()?
            }
            syn::Fields::Unit => Vec::new(),
        };
        variants.push(EnumVariantInfo {
            name: v_name,
            fields: v_fields,
            doc: v_doc,
        });
    }

    ctx.enums.push(EnumInfo {
        name,
        variants,
        namespace,
        rust_module_path,
        doc,
    });

    Ok(())
}

fn visit_fn(f: &syn::ItemFn, ctx: &mut ParseContext) -> Result<(), BindgenError> {
    let attr = match get_koffi_attr(&f.attrs, "export") {
        Some(a) => a,
        None => return Ok(()),
    };

    // Only public free functions.
    if !matches!(f.vis, syn::Visibility::Public(_)) {
        return Ok(());
    }

    let args = parse_export_args(&attr);
    let rust_name = f.sig.ident.to_string();
    let raw_name = rust_name.trim_start_matches("r#");
    let name = args
        .name
        .clone()
        .unwrap_or_else(|| AsLowerCamelCase(raw_name).to_string());
    let namespace = args
        .package
        .clone()
        .unwrap_or_else(|| ctx.current_namespace().to_string());
    let doc = extract_doc(&f.attrs);
    let is_async = f.sig.asyncness.is_some();
    let rust_module_path = ctx.current_module_path();

    let mut params = Vec::new();
    for input in &f.sig.inputs {
        if let syn::FnArg::Typed(pat_type) = input {
            let param_name = pat_ident(&pat_type.pat).unwrap_or_else(|| "arg".into());
            let param_ty = parse_type(&pat_type.ty, &ctx.type_decls)?;
            params.push(ParamInfo {
                name: param_name,
                ty: param_ty,
            });
        }
    }

    let ret_ty = match &f.sig.output {
        syn::ReturnType::Default => FFIType::Unit,
        syn::ReturnType::Type(_, ty) => parse_type(ty, &ctx.type_decls)?,
    };

    ctx.functions.push(FnInfo {
        name,
        rust_name,
        is_async,
        params,
        ret_ty,
        receiver: None,
        parent_struct: None,
        namespace,
        rust_module_path,
        parent_rust_module_path: Vec::new(),
        doc,
        args,
    });

    Ok(())
}

fn visit_impl(impl_item: &syn::ItemImpl, ctx: &mut ParseContext) -> Result<(), BindgenError> {
    // Only impl blocks explicitly annotated with `#[koffi::export]`.
    let impl_attr = match get_koffi_attr(&impl_item.attrs, "export") {
        Some(a) => a,
        None => return Ok(()),
    };

    // Trait impls (impl Foo for Bar) are not supported.
    if impl_item.trait_.is_some() {
        return Ok(());
    }

    let impl_args = parse_export_args(&impl_attr);

    let parent_name = match &*impl_item.self_ty {
        syn::Type::Path(tp) => tp.path.segments.last().map(|s| s.ident.to_string()),
        _ => None,
    };
    let parent_name = match parent_name {
        Some(n) => n,
        None => return Ok(()),
    };

    // The module path where the impl block (and typically the type) lives.
    let impl_module_path = ctx.current_module_path();

    for member in &impl_item.items {
        let method = match member {
            syn::ImplItem::Fn(m) => m,
            _ => continue,
        };

        // Only public methods.
        if !matches!(method.vis, syn::Visibility::Public(_)) {
            continue;
        }

        let rust_name = method.sig.ident.to_string();
        let raw_name = rust_name.trim_start_matches("r#");
        let doc = extract_doc(&method.attrs);
        let is_async = method.sig.asyncness.is_some();

        let mut params = Vec::new();
        let mut receiver = None;

        for input in &method.sig.inputs {
            match input {
                syn::FnArg::Receiver(rec) => {
                    receiver = Some(detect_receiver(rec));
                }
                syn::FnArg::Typed(pat_type) => {
                    let param_name = pat_ident(&pat_type.pat).unwrap_or_else(|| "arg".into());
                    let mut param_ty = parse_type(&pat_type.ty, &ctx.type_decls)?;
                    // Replace `Self` type references with the concrete parent name.
                    param_ty = replace_self(param_ty, &parent_name, &ctx.type_decls);
                    params.push(ParamInfo {
                        name: param_name,
                        ty: param_ty,
                    });
                }
            }
        }

        let mut ret_ty = match &method.sig.output {
            syn::ReturnType::Default => FFIType::Unit,
            syn::ReturnType::Type(_, ty) => parse_type(ty, &ctx.type_decls)?,
        };
        ret_ty = replace_self(ret_ty, &parent_name, &ctx.type_decls);

        // Per-method `#[koffi::export(...)]` can override name/package.
        let method_args = get_koffi_attr(&method.attrs, "export")
            .map(|a| parse_export_args(&a))
            .unwrap_or_else(|| impl_args.clone());

        let effective_name = method_args
            .name
            .clone()
            .unwrap_or_else(|| AsLowerCamelCase(raw_name).to_string());
        let effective_ns = method_args.package.clone().unwrap_or_else(|| {
            impl_args
                .package
                .clone()
                .unwrap_or_else(|| ctx.current_namespace().to_string())
        });

        ctx.functions.push(FnInfo {
            name: effective_name,
            rust_name,
            is_async,
            params,
            ret_ty,
            receiver,
            parent_struct: Some(parent_name.clone()),
            namespace: effective_ns,
            rust_module_path: impl_module_path.clone(),
            parent_rust_module_path: impl_module_path.clone(),
            doc,
            args: method_args,
        });
    }

    Ok(())
}

/// Convert a [`syn::Type`] to [`FFIType`].
///
/// Custom (user-defined) types are looked up in `type_decls`:
/// - known opaque types  -> `FFIType::Opaque(placeholder_ref)`
/// - known data types    -> `FFIType::Data(placeholder_ref)`
/// - unknown types       -> `FFIType::Data(placeholder_ref)` (Phase 2 will correct)
pub fn parse_type(
    ty: &syn::Type,
    type_decls: &TypeDeclarationMap,
) -> Result<FFIType, BindgenError> {
    match ty {
        syn::Type::Tuple(t) if t.elems.is_empty() => Ok(FFIType::Unit),
        syn::Type::Tuple(_) => {
            Err(BindgenError::UnsupportedType(
                "Non-unit tuples are not supported across the FFI boundary".into(),
            ))
        }
        syn::Type::Reference(r) => parse_reference(r, type_decls),
        syn::Type::Slice(s) => {
            let inner = parse_type(&s.elem, type_decls)?;
            if inner == FFIType::U8 {
                Ok(FFIType::Bytes)
            } else {
                Err(BindgenError::UnsupportedType(
                    "Bare slices are not supported; use &[u8] or Vec<T>".into(),
                ))
            }
        }
        syn::Type::Path(tp) => {
            let segment = tp
                .path
                .segments
                .last()
                .ok_or_else(|| BindgenError::UnsupportedType("Empty type path".into()))?;

            parse_type_segment(&segment.ident.to_string(), &segment.arguments, type_decls)
        }
        syn::Type::Infer(_) => {
            Err(BindgenError::UnsupportedType(
                "Inferred types (`_`) are not supported in koffi signatures".into(),
            ))
        }
        other => {
            Err(BindgenError::UnsupportedType(format!(
                "Unsupported type: {}",
                quote::quote!(#other),
            )))
        }
    }
}

fn parse_reference(
    r: &syn::TypeReference,
    type_decls: &TypeDeclarationMap,
) -> Result<FFIType, BindgenError> {
    match &*r.elem {
        // &str -> String
        syn::Type::Path(tp) if path_last_str(tp) == "str" => Ok(FFIType::String),

        // &[u8] -> Bytes
        syn::Type::Slice(s) => {
            let inner = parse_type(&s.elem, type_decls)?;
            if inner == FFIType::U8 {
                Ok(FFIType::Bytes)
            } else {
                Err(BindgenError::UnsupportedType(
                    "&[T] is only supported for T = u8".into(),
                ))
            }
        }

        // &T pass through; handles &Self, &Struct, etc.
        other => parse_type(other, type_decls),
    }
}

fn parse_type_segment(
    name: &str,
    args: &syn::PathArguments,
    type_decls: &TypeDeclarationMap,
) -> Result<FFIType, BindgenError> {
    match name {
        "bool" => return Ok(FFIType::Bool),
        "i8" => return Ok(FFIType::I8),
        "i16" => return Ok(FFIType::I16),
        "i32" => return Ok(FFIType::I32),
        "i64" => return Ok(FFIType::I64),
        "isize" => return Ok(FFIType::I64), // always i64 at the FFI boundary
        "u8" => return Ok(FFIType::U8),
        "u16" => return Ok(FFIType::U16),
        "u32" => return Ok(FFIType::U32),
        "u64" => return Ok(FFIType::U64),
        "usize" => return Ok(FFIType::U64), // always u64 at the FFI boundary
        "f32" => return Ok(FFIType::F32),
        "f64" => return Ok(FFIType::F64),
        "String" => return Ok(FFIType::String),
        "str" => return Ok(FFIType::String),
        _ => {}
    }

    let angle_args = match args {
        syn::PathArguments::AngleBracketed(a) => Some(a),
        _ => None,
    };

    match name {
        "Option" => {
            let inner = single_angle_type(angle_args, "Option", type_decls)?;
            return Ok(FFIType::Option(Box::new(inner)));
        }
        "Result" => {
            let (ok, err) = two_angle_types(angle_args, "Result", type_decls)?;
            return Ok(FFIType::Result(Box::new(ok), Box::new(err)));
        }
        "Vec" => {
            let inner = single_angle_type(angle_args, "Vec", type_decls)?;

            return Ok(if inner == FFIType::U8 {
                FFIType::Bytes
            } else {
                FFIType::Vec(Box::new(inner))
            });
        }
        "HashMap" | "BTreeMap" => {
            let (k, v) = two_angle_types(angle_args, name, type_decls)?;
            return Ok(FFIType::Map(Box::new(k), Box::new(v)));
        }
        "HashSet" | "BTreeSet" => {
            let inner = single_angle_type(angle_args, name, type_decls)?;
            return Ok(FFIType::Set(Box::new(inner)));
        }
        _ => {}
    }

    // `Self` is a special case: replaced later by `replace_self`.
    let is_opaque = *type_decls.get(name).unwrap_or(&false);
    let type_ref = placeholder_type_ref(name.to_string());

    Ok(if is_opaque {
        FFIType::Opaque(type_ref)
    } else {
        FFIType::Data(type_ref)
    })
}

fn single_angle_type(
    args: Option<&syn::AngleBracketedGenericArguments>,
    parent: &str,
    type_decls: &TypeDeclarationMap,
) -> Result<FFIType, BindgenError> {
    let args = args.ok_or_else(|| {
        BindgenError::UnsupportedType(format!("{parent} requires a type argument"))
    })?;

    let ty_args = collect_type_args(args);
    if ty_args.len() != 1 {
        return Err(BindgenError::UnsupportedType(format!(
            "{parent} expects exactly 1 type argument, got {}",
            ty_args.len(),
        )));
    }

    parse_type(ty_args[0], type_decls)
}

fn two_angle_types(
    args: Option<&syn::AngleBracketedGenericArguments>,
    parent: &str,
    type_decls: &TypeDeclarationMap,
) -> Result<(FFIType, FFIType), BindgenError> {
    let args = args.ok_or_else(|| {
        BindgenError::UnsupportedType(format!("{parent} requires type arguments"))
    })?;

    let ty_args = collect_type_args(args);
    if ty_args.len() < 2 {
        return Err(BindgenError::UnsupportedType(format!(
            "{parent} expects at least 2 type arguments, got {}",
            ty_args.len(),
        )));
    }

    let first = parse_type(ty_args[0], type_decls)?;
    let second = parse_type(ty_args[1], type_decls)?;

    Ok((first, second))
}

fn collect_type_args(args: &syn::AngleBracketedGenericArguments) -> Vec<&syn::Type> {
    args.args
        .iter()
        .filter_map(|arg| {
            if let syn::GenericArgument::Type(ty) = arg {
                Some(ty)
            } else {
                None
            }
        })
        .collect()
}

fn replace_self(ty: FFIType, parent: &str, type_decls: &TypeDeclarationMap) -> FFIType {
    match ty {
        FFIType::Data(ref r) | FFIType::Opaque(ref r) if r.name == "Self" => {
            let is_opaque = *type_decls.get(parent).unwrap_or(&false);
            let new_ref = placeholder_type_ref(parent.to_string());

            if is_opaque {
                FFIType::Opaque(new_ref)
            } else {
                FFIType::Data(new_ref)
            }
        }
        FFIType::Option(inner) => {
            FFIType::Option(Box::new(replace_self(*inner, parent, type_decls)))
        }
        FFIType::Result(ok, err) => {
            FFIType::Result(
                Box::new(replace_self(*ok, parent, type_decls)),
                Box::new(replace_self(*err, parent, type_decls)),
            )
        }
        FFIType::Vec(inner) => FFIType::Vec(Box::new(replace_self(*inner, parent, type_decls))),
        FFIType::Map(k, v) => {
            FFIType::Map(
                Box::new(replace_self(*k, parent, type_decls)),
                Box::new(replace_self(*v, parent, type_decls)),
            )
        }
        FFIType::Set(inner) => FFIType::Set(Box::new(replace_self(*inner, parent, type_decls))),
        other => other,
    }
}

#[must_use]
const fn detect_receiver(rec: &syn::Receiver) -> ReceiverType {
    if rec.reference.is_some() {
        if rec.mutability.is_some() {
            ReceiverType::RefMut
        } else {
            ReceiverType::Ref
        }
    } else {
        ReceiverType::Owned
    }
}

#[must_use]
pub fn has_koffi_attr(attrs: &[syn::Attribute], name: &str) -> bool {
    get_koffi_attr(attrs, name).is_some()
}

#[must_use]
pub fn get_koffi_attr(attrs: &[syn::Attribute], name: &str) -> Option<syn::Attribute> {
    attrs
        .iter()
        .find(|attr| {
            let segs = &attr.path().segments;
            match segs.len() {
                1 => segs[0].ident == name || segs[0].ident == format!("r#{name}"),
                2 => {
                    segs[0].ident == "koffi"
                        && (segs[1].ident == name || segs[1].ident == format!("r#{name}"))
                }
                _ => false,
            }
        })
        .cloned()
}

#[must_use]
pub fn parse_export_args(attr: &syn::Attribute) -> ExportArgs {
    let mut args = ExportArgs::default();
    let list = match &attr.meta {
        syn::Meta::List(l) => l,
        _ => return args,
    };

    let _ = list.parse_nested_meta(|meta| {
        if meta.path.is_ident("name") {
            if let Ok(s) = meta.value()?.parse::<syn::LitStr>() {
                args.name = Some(s.value());
            }
        } else if meta.path.is_ident("package") {
            if let Ok(s) = meta.value()?.parse::<syn::LitStr>() {
                args.package = Some(s.value());
            }
        } else if meta.path.is_ident("blocking") {
            args.blocking = true;
        } else if meta.path.is_ident("deprecated")
            && let Ok(s) = meta.value()?.parse::<syn::LitStr>()
        {
            args.deprecated = Some(s.value());
        }

        Ok(())
    });

    args
}

#[must_use]
pub fn attrs_get_namespace(attrs: &[syn::Attribute]) -> Option<String> {
    attrs.iter().find_map(|attr| {
        let segs = &attr.path().segments;
        let is_ns = match segs.len() {
            1 => segs[0].ident == "namespace",
            2 => segs[0].ident == "koffi" && segs[1].ident == "namespace",
            _ => false,
        };

        if !is_ns {
            return None;
        }

        attr.parse_args::<syn::LitStr>().ok().map(|s| s.value())
    })
}

#[must_use]
pub fn file_level_namespace(attrs: &[syn::Attribute]) -> Option<String> {
    attrs.iter().find_map(|attr| {
        if !matches!(attr.style, syn::AttrStyle::Inner(_)) {
            return None;
        }

        attrs_get_namespace(std::slice::from_ref(attr))
    })
}

#[must_use]
pub fn extract_doc(attrs: &[syn::Attribute]) -> Vec<String> {
    let lines: Vec<String> = attrs
        .iter()
        .filter_map(|attr| {
            if !attr.path().is_ident("doc") {
                return None;
            }

            if let syn::Meta::NameValue(nv) = &attr.meta
                && let syn::Expr::Lit(el) = &nv.value
                && let syn::Lit::Str(s) = &el.lit
            {
                return Some(s.value());
            }

            None
        })
        .collect();

    if lines.is_empty() { Vec::new() } else { lines }
}

fn has_serde_skip(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|attr| {
        if !attr.path().is_ident("serde") {
            return false;
        }

        let mut found = false;
        let _ = attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("skip") || meta.path.is_ident("skip_serializing") {
                found = true;
            }

            Ok(())
        });

        found
    })
}

#[must_use]
pub fn resolve_mod_file(src_dir: &Path, name: &str) -> Option<(PathBuf, PathBuf)> {
    let flat = src_dir.join(format!("{name}.rs"));
    if flat.exists() {
        return Some((flat, src_dir.to_path_buf()));
    }

    let dir_mod = src_dir.join(name).join("mod.rs");
    if dir_mod.exists() {
        return Some((dir_mod, src_dir.join(name)));
    }

    None
}

fn entry_point(src_dir: &Path) -> Option<PathBuf> {
    let lib = src_dir.join("lib.rs");
    let main = src_dir.join("main.rs");

    if lib.exists() {
        Some(lib)
    } else if main.exists() {
        Some(main)
    } else {
        None
    }
}

/// Return the last path segment as an owned `String`.
fn path_last_str(tp: &syn::TypePath) -> String {
    tp.path
        .segments
        .last()
        .map(|s| s.ident.to_string())
        .unwrap_or_default()
}

fn pat_ident(pat: &syn::Pat) -> Option<String> {
    if let syn::Pat::Ident(pi) = pat {
        Some(pi.ident.to_string())
    } else {
        None
    }
}

/// Create a placeholder `TypeRef` with empty `crate_id` and zero `schema_hash`.
/// Phase 2 replaces these with fully-qualified refs.
#[must_use]
pub const fn placeholder_type_ref(name: String) -> TypeRef {
    TypeRef {
        crate_id: CrateId {
            name: String::new(),
            version: String::new(),
        },
        module_path: Vec::new(),
        name,
        schema_hash: 0,
    }
}
