//! Phase 2 parser: type resolution using `cargo rustdoc --output-format json`.
//!
//! # Why rustdoc JSON
//!
//! Every `TypeRef` produced by Phase 1 (`visitor.rs`) carries only a local type
//! name and an empty `crate_id`. To fill in a complete identity we need a
//! source that understands Rust's full name-resolution semantics.
//!
//! # What this module does
//!
//! 1. Shells out to `cargo rustdoc --output-format json`.
//! 2. Builds lookup indexes over the resulting JSON.
//! 3. For each item from Phase 1, finds the corresponding rustdoc item and
//!    re-resolves its type signatures with fully-qualified [`TypeRef`]s.
//! 4. Preserves `rust_module_path` and `parent_rust_module_path` from Phase 1
//!    (these come from the file system traversal, not from rustdoc).
//! 5. Computes structural schema hashes for all user-defined types.
//! 6. Collects the set of cross-crate type imports needed by the interface.

use std::{
    collections::{HashMap, HashSet},
    fs, io,
    path::{Path, PathBuf},
};

use koffi_ir::{
    CrateId, CrateInterface, EnumInfo, EnumVariantInfo, FFIType, FieldInfo, FnInfo, ParamInfo,
    StructInfo, TypeRef,
};
use rustdoc_types::{
    Attribute, Crate, GenericArg, GenericArgs, Id, Item, ItemEnum, StructKind, Type, VariantKind,
};
use tracing::warn;

use super::visitor::PartialInterface;
use crate::BindgenError;

/// Shell out to `cargo rustdoc --output-format json` and return the path to the
/// resulting JSON file.
pub fn ensure_json(crate_path: &Path) -> Result<PathBuf, BindgenError> {
    let cargo_toml = crate_path.join("Cargo.toml");
    let crate_name = crate_name_from_toml(&cargo_toml)?;
    let crate_ident = crate_name.replace('-', "_");

    let output = std::process::Command::new("cargo")
        .args([
            "+nightly",
            "-Z",
            "unstable-options",
            "rustdoc",
            "--output-format",
            "json",
            "--manifest-path",
            &cargo_toml.to_string_lossy(),
            "--",
            "--document-private-items",
        ])
        .output()?;

    if !output.status.success() {
        return Err(BindgenError::RustdocFailed(
            String::from_utf8_lossy(&output.stderr).into_owned(),
        ));
    }

    find_rustdoc_json(crate_path, &crate_ident)
}

/// Phase 2 main entry point. Takes the `PartialInterface` from Phase 1 and
/// the rustdoc JSON path, and produces a fully resolved [`CrateInterface`].
pub fn resolve_types(
    partial: PartialInterface,
    json_path: &Path,
    pkg_schemas: &[CrateInterface],
) -> Result<CrateInterface, BindgenError> {
    let json = fs::read_to_string(json_path)?;
    let krate: Crate = serde_json::from_str(&json)?;

    let local_decls: HashMap<String, bool> = partial
        .structs
        .iter()
        .map(|s| (s.name.clone(), s.is_opaque))
        .chain(partial.enums.iter().map(|e| (e.name.clone(), false)))
        .collect();

    let resolver = TypeResolver::new(
        &krate,
        partial.crate_name.clone(),
        partial.version.clone(),
        pkg_schemas,
        local_decls,
    );

    let structs: Vec<StructInfo> = partial
        .structs
        .into_iter()
        .map(|s| resolver.resolve_struct(s))
        .collect::<Result<_, _>>()?;

    let enums: Vec<EnumInfo> = partial
        .enums
        .into_iter()
        .map(|e| resolver.resolve_enum(e))
        .collect::<Result<_, _>>()?;

    let functions: Vec<FnInfo> = partial
        .functions
        .into_iter()
        .map(|f| resolver.resolve_fn(f))
        .collect::<Result<_, _>>()?;

    let imports = resolver.collect_imports(&structs, &enums, &functions);

    Ok(CrateInterface {
        namespace: partial.namespace,
        crate_name: partial.crate_name,
        version: partial.version,
        structs,
        enums,
        functions,
        imports,
    })
}

/// Walk up from `crate_path` looking for `target/doc/{crate_ident}.json`.
///
/// Cargo places the rustdoc JSON in the *workspace* `target/` even when the
/// command is run inside a member crate, so we must walk upward.
fn find_rustdoc_json(crate_path: &Path, crate_ident: &str) -> Result<PathBuf, BindgenError> {
    let filename = format!("{crate_ident}.json");
    let mut dir = crate_path.to_path_buf();

    loop {
        let candidate = dir.join("target").join("doc").join(&filename);
        if candidate.exists() {
            return Ok(candidate);
        }
        match dir.parent() {
            Some(p) => dir = p.to_path_buf(),
            None => break,
        }
    }

    Err(BindgenError::RustdocFailed(format!(
        "Could not find {filename} in any target/doc directory above {}. \
         Run `cargo rustdoc --output-format json` manually first.",
        crate_path.display()
    )))
}

struct TypeResolver<'a> {
    krate: &'a Crate,
    crate_name: String,
    crate_ident: String,
    crate_version: String,
    pkg_schemas: &'a [CrateInterface],
    local_decls: HashMap<String, bool>,
    /// item name -> list of item Ids.
    name_to_ids: HashMap<String, Vec<Id>>,
    /// item Id -> (full path segments, `crate_id_u32`).
    path_index: HashMap<Id, (Vec<String>, u32)>,
}

impl<'a> TypeResolver<'a> {
    fn new(
        krate: &'a Crate,
        crate_name: String,
        crate_version: String,
        pkg_schemas: &'a [CrateInterface],
        local_decls: HashMap<String, bool>,
    ) -> Self {
        let crate_ident = crate_name.replace('-', "_");

        let mut name_to_ids: HashMap<String, Vec<Id>> = HashMap::new();
        for (id, item) in &krate.index {
            if let Some(name) = &item.name {
                name_to_ids.entry(name.clone()).or_default().push(*id);
            }
        }

        let path_index: HashMap<Id, (Vec<String>, u32)> = krate
            .paths
            .iter()
            .map(|(id, summary)| (*id, (summary.path.clone(), summary.crate_id)))
            .collect();

        Self {
            krate,
            crate_name,
            crate_ident,
            crate_version,
            pkg_schemas,
            local_decls,
            name_to_ids,
            path_index,
        }
    }

    fn resolve_struct(&self, s: StructInfo) -> Result<StructInfo, BindgenError> {
        if s.is_opaque {
            return Ok(s); // no fields to re-resolve
        }

        if let Some(item) = self.find_struct_item(&s.name) {
            match self.resolve_struct_fields(item) {
                Ok(fields) => Ok(StructInfo { fields, ..s }),
                Err(e) => {
                    warn!(
                        "could not re-resolve fields of `{}` from rustdoc: {e}",
                        s.name
                    );

                    let fields = s
                        .fields
                        .into_iter()
                        .map(|f| {
                            Ok(FieldInfo {
                                ty: self.resolve_ffi_type(f.ty)?,
                                ..f
                            })
                        })
                        .collect::<Result<_, BindgenError>>()?;

                    Ok(StructInfo { fields, ..s })
                }
            }
        } else {
            let fields = s
                .fields
                .into_iter()
                .map(|f| {
                    Ok(FieldInfo {
                        ty: self.resolve_ffi_type(f.ty)?,
                        ..f
                    })
                })
                .collect::<Result<_, BindgenError>>()?;

            Ok(StructInfo { fields, ..s })
        }
    }

    fn resolve_struct_fields(&self, item: &Item) -> Result<Vec<FieldInfo>, BindgenError> {
        let struct_ = match &item.inner {
            ItemEnum::Struct(s) => s,
            _ => return Err(BindgenError::UnsupportedType("Expected Struct item".into())),
        };

        match &struct_.kind {
            StructKind::Plain { fields, .. } => {
                fields
                    .iter()
                    .filter_map(|fid| {
                        let field_item = self.krate.index.get(fid)?;
                        let ty = match &field_item.inner {
                            ItemEnum::StructField(t) => t,
                            _ => return None,
                        };
                        let name = field_item.name.clone().unwrap_or_default();
                        let skip_serde = field_item
                            .attrs
                            .iter()
                            .any(|a| matches!(a, Attribute::Other(a) if a.contains("serde(skip")));

                        Some(self.resolve_rustdoc_type(ty).map(|ty| {
                            FieldInfo {
                                name,
                                ty,
                                skip_serde,
                            }
                        }))
                    })
                    .collect()
            }

            StructKind::Tuple(fields) => {
                fields
                    .iter()
                    .enumerate()
                    .filter_map(|(i, maybe_fid)| {
                        let fid = maybe_fid.as_ref()?;
                        let field_item = self.krate.index.get(fid)?;
                        let ty = match &field_item.inner {
                            ItemEnum::StructField(t) => t,
                            _ => return None,
                        };

                        Some(self.resolve_rustdoc_type(ty).map(|ty| {
                            FieldInfo {
                                name: i.to_string(),
                                ty,
                                skip_serde: false,
                            }
                        }))
                    })
                    .collect()
            }

            StructKind::Unit => Ok(Vec::new()),
        }
    }

    fn resolve_enum(&self, e: EnumInfo) -> Result<EnumInfo, BindgenError> {
        if let Some(item) = self.find_enum_item(&e.name) {
            match self.resolve_enum_variants(item) {
                Ok(variants) => Ok(EnumInfo { variants, ..e }),
                Err(err) => {
                    warn!("could not re-resolve variants of `{}`: {err}", e.name);
                    let variants = e
                        .variants
                        .into_iter()
                        .map(|v| self.resolve_enum_variant_types(v))
                        .collect::<Result<_, _>>()?;

                    Ok(EnumInfo { variants, ..e })
                }
            }
        } else {
            let variants = e
                .variants
                .into_iter()
                .map(|v| self.resolve_enum_variant_types(v))
                .collect::<Result<_, _>>()?;

            Ok(EnumInfo { variants, ..e })
        }
    }

    fn resolve_enum_variants(&self, item: &Item) -> Result<Vec<EnumVariantInfo>, BindgenError> {
        let enum_ = match &item.inner {
            ItemEnum::Enum(e) => e,
            _ => return Err(BindgenError::UnsupportedType("Expected Enum item".into())),
        };

        enum_
            .variants
            .iter()
            .filter_map(|vid| {
                let variant_item = self.krate.index.get(vid)?;
                let variant = match &variant_item.inner {
                    ItemEnum::Variant(v) => v,
                    _ => return None,
                };
                let name = variant_item.name.clone().unwrap_or_default();
                let doc = variant_item
                    .docs
                    .as_ref()
                    .map(|d| d.lines().map(|s| s.to_owned()).collect())
                    .unwrap_or_default();

                let fields_result = match &variant.kind {
                    VariantKind::Plain => Ok(Vec::new()),

                    VariantKind::Tuple(fields) => {
                        fields
                            .iter()
                            .enumerate()
                            .filter_map(|(i, maybe_fid)| {
                                let fid = maybe_fid.as_ref()?;
                                let fi = self.krate.index.get(fid)?;
                                let ty = match &fi.inner {
                                    ItemEnum::StructField(t) => t,
                                    _ => return None,
                                };

                                Some(self.resolve_rustdoc_type(ty).map(|ty| {
                                    FieldInfo {
                                        name: format!("field{i}"),
                                        ty,
                                        skip_serde: false,
                                    }
                                }))
                            })
                            .collect()
                    }

                    VariantKind::Struct { fields, .. } => {
                        fields
                            .iter()
                            .filter_map(|fid| {
                                let fi = self.krate.index.get(fid)?;
                                let ty = match &fi.inner {
                                    ItemEnum::StructField(t) => t,
                                    _ => return None,
                                };
                                let field_name = fi.name.clone().unwrap_or_default();
                                let skip = fi.attrs.iter().any(
                                |a| matches!(a, Attribute::Other(a) if a.contains("serde(skip")),
                            );

                                Some(self.resolve_rustdoc_type(ty).map(|ty| {
                                    FieldInfo {
                                        name: field_name,
                                        ty,
                                        skip_serde: skip,
                                    }
                                }))
                            })
                            .collect()
                    }
                };

                Some(fields_result.map(|fields| EnumVariantInfo { name, fields, doc }))
            })
            .collect()
    }

    fn resolve_enum_variant_types(
        &self,
        v: EnumVariantInfo,
    ) -> Result<EnumVariantInfo, BindgenError> {
        let fields = v
            .fields
            .into_iter()
            .map(|f| {
                Ok(FieldInfo {
                    ty: self.resolve_ffi_type(f.ty)?,
                    ..f
                })
            })
            .collect::<Result<_, BindgenError>>()?;

        Ok(EnumVariantInfo { fields, ..v })
    }

    fn resolve_fn(&self, f: FnInfo) -> Result<FnInfo, BindgenError> {
        // Phase 1 sets rust_module_path / parent_rust_module_path from the
        // file-system traversal. We preserve those fields here; only the
        // *type signatures* (params and ret_ty) are updated from rustdoc.
        let item = match &f.parent_struct {
            Some(parent) => self.find_method_item(parent, &f.rust_name),
            None => self.find_function_item(&f.rust_name),
        };

        match item {
            Some(fn_item) => {
                match self.resolve_fn_decl(fn_item, &f) {
                    Ok((params, ret_ty)) => {
                        Ok(FnInfo {
                            params,
                            ret_ty,
                            ..f
                        })
                    }
                    Err(e) => {
                        warn!("could not re-resolve signature of `{}`: {e}", f.rust_name);

                        self.resolve_fn_types_only(f)
                    }
                }
            }
            None => self.resolve_fn_types_only(f),
        }
    }

    fn resolve_fn_decl(
        &self,
        item: &Item,
        original: &FnInfo,
    ) -> Result<(Vec<ParamInfo>, FFIType), BindgenError> {
        let func = match &item.inner {
            ItemEnum::Function(f) => f,
            _ => {
                return Err(BindgenError::UnsupportedType(
                    "Expected Function item".into(),
                ));
            }
        };

        // `sig.inputs` includes `("self", ...)` for methods. skip it.
        let mut params = Vec::new();
        for (param_name, param_type) in &func.sig.inputs {
            if param_name == "self" {
                continue;
            }

            let ty = self.resolve_rustdoc_type(param_type)?;
            let by_ref = matches!(param_type, Type::BorrowedRef { .. });

            params.push(ParamInfo {
                name: param_name.clone(),
                ty,
                by_ref,
            });
        }

        // If the count mismatches Phase 1, align by index (edge case: macro wrappers).
        if params.len() != original.params.len() {
            params = original
                .params
                .iter()
                .enumerate()
                .map(|(i, orig_p)| {
                    let param_type = func
                        .sig
                        .inputs
                        .get(i + usize::from(original.receiver.is_some()))
                        .and_then(|(n, t)| if n == "self" { None } else { Some(t) });

                    let ty = param_type
                        .and_then(|t| self.resolve_rustdoc_type(t).ok())
                        .unwrap_or_else(|| {
                            self.resolve_ffi_type(orig_p.ty.clone())
                                .unwrap_or_else(|_| orig_p.ty.clone())
                        });

                    let by_ref =
                        param_type.map_or(orig_p.by_ref, |t| matches!(t, Type::BorrowedRef { .. }));

                    ParamInfo {
                        name: orig_p.name.clone(),
                        ty,
                        by_ref,
                    }
                })
                .collect();
        }

        let ret_ty = match &func.sig.output {
            Some(t) => {
                match t {
                    Type::Generic(name) if name == "Self" => {
                        self.resolve_ffi_type(original.ret_ty.clone())?
                    }
                    other => self.resolve_rustdoc_type(other)?,
                }
            }
            None => FFIType::Unit,
        };

        Ok((params, ret_ty))
    }

    fn resolve_fn_types_only(&self, f: FnInfo) -> Result<FnInfo, BindgenError> {
        let params = f
            .params
            .into_iter()
            .map(|p| {
                Ok(ParamInfo {
                    ty: self.resolve_ffi_type(p.ty)?,
                    ..p
                })
            })
            .collect::<Result<_, BindgenError>>()?;
        let ret_ty = self.resolve_ffi_type(f.ret_ty)?;

        Ok(FnInfo {
            params,
            ret_ty,
            ..f
        })
    }

    fn resolve_rustdoc_type(&self, ty: &Type) -> Result<FFIType, BindgenError> {
        match ty {
            Type::Primitive(name) => primitive_to_ffi(name),

            Type::ResolvedPath(path) => self.resolve_path(path),

            Type::BorrowedRef { type_, .. } => {
                match type_.as_ref() {
                    Type::Primitive(s) if s == "str" => Ok(FFIType::String),
                    Type::Slice(inner) => {
                        match self.resolve_rustdoc_type(inner)? {
                            FFIType::U8 => Ok(FFIType::Bytes),
                            other => {
                                Err(BindgenError::UnsupportedType(format!(
                                    "&[{other:?}] is not supported; only &[u8] maps to Bytes",
                                )))
                            }
                        }
                    }
                    other => self.resolve_rustdoc_type(other),
                }
            }

            Type::Tuple(elems) if elems.is_empty() => Ok(FFIType::Unit),

            Type::Tuple(_) => {
                Err(BindgenError::UnsupportedType(
                    "Non-unit tuples are not supported across the FFI boundary".into(),
                ))
            }

            Type::Slice(inner) => {
                match self.resolve_rustdoc_type(inner)? {
                    FFIType::U8 => Ok(FFIType::Bytes),
                    _ => {
                        Err(BindgenError::UnsupportedType(
                            "Bare slices are not supported; use &[u8] or Vec<u8>".into(),
                        ))
                    }
                }
            }

            Type::Generic(name) => {
                Err(BindgenError::UnsupportedType(format!(
                    "Generic parameter `{name}` in koffi-exported signature (monomorphise first)",
                )))
            }

            other => {
                Err(BindgenError::UnsupportedType(format!(
                    "Unsupported rustdoc type variant: {other:?}",
                )))
            }
        }
    }

    fn resolve_path(&self, path: &rustdoc_types::Path) -> Result<FFIType, BindgenError> {
        if let Some((segments, crate_id_u32)) = self.path_index.get(&path.id) {
            let full = segments.join("::");

            match full.as_str() {
                "std::string::String" | "alloc::string::String" => {
                    return Ok(FFIType::String);
                }
                "std::vec::Vec" | "alloc::vec::Vec" => {
                    let inner = self.single_generic_arg(path)?;
                    return Ok(if inner == FFIType::U8 {
                        FFIType::Bytes
                    } else {
                        FFIType::Vec(Box::new(inner))
                    });
                }
                "std::option::Option" | "core::option::Option" => {
                    let inner = self.single_generic_arg(path)?;
                    return Ok(FFIType::Option(Box::new(inner)));
                }
                "std::result::Result" | "core::result::Result" => {
                    let (ok, err) = self.two_generic_args(path)?;
                    return Ok(FFIType::Result(Box::new(ok), Box::new(err)));
                }
                "std::collections::HashMap"
                | "std::collections::BTreeMap"
                | "alloc::collections::BTreeMap" => {
                    let (k, v) = self.two_generic_args(path)?;
                    return Ok(FFIType::Map(Box::new(k), Box::new(v)));
                }
                "std::collections::HashSet"
                | "std::collections::BTreeSet"
                | "alloc::collections::BTreeSet" => {
                    let inner = self.single_generic_arg(path)?;
                    return Ok(FFIType::Set(Box::new(inner)));
                }
                _ => {}
            }

            let type_name = segments
                .last()
                .cloned()
                .unwrap_or_else(|| path.path.clone());
            let module_path = if segments.len() > 2 {
                segments[1..segments.len() - 1].to_vec()
            } else {
                Vec::new()
            };

            let (crate_pkg_name, version) = self.crate_pkg_for_id(*crate_id_u32);
            let is_local = *crate_id_u32 == 0;
            let is_opaque = if is_local {
                self.local_decls.get(&type_name).copied().unwrap_or(false)
            } else {
                self.dep_opaque(&crate_pkg_name, &type_name)
            };

            let type_ref = TypeRef {
                crate_id: CrateId {
                    name: crate_pkg_name,
                    version,
                },
                module_path,
                name: type_name,
            };

            return Ok(if is_opaque {
                FFIType::Opaque(type_ref)
            } else {
                FFIType::Data(type_ref)
            });
        }

        // Id not in the path index. fall back to name-based lookup.
        self.resolve_path_by_name(&path.path, path)
    }

    fn resolve_path_by_name(
        &self,
        name: &str,
        path: &rustdoc_types::Path,
    ) -> Result<FFIType, BindgenError> {
        if let Some(&is_opaque) = self.local_decls.get(name) {
            let type_ref = TypeRef {
                crate_id: CrateId {
                    name: self.crate_name.clone(),
                    version: self.crate_version.clone(),
                },
                module_path: Vec::new(),
                name: name.to_string(),
            };
            return Ok(if is_opaque {
                FFIType::Opaque(type_ref)
            } else {
                FFIType::Data(type_ref)
            });
        }

        for dep in self.pkg_schemas {
            if let Some(ty) = find_in_dep(dep, name) {
                return Ok(ty);
            }
        }

        Err(BindgenError::UnsupportedType(format!(
            "Could not resolve type `{name}` (id={:?}); \
             ensure it is annotated with #[koffi::data] or #[koffi::opaque] \
             and its crate's schema.json is available",
            path.id
        )))
    }

    fn single_generic_arg(&self, path: &rustdoc_types::Path) -> Result<FFIType, BindgenError> {
        let args = path.args.as_deref().ok_or_else(|| {
            BindgenError::UnsupportedType(format!("`{}` requires a type argument", path.path))
        })?;
        let type_args = angle_type_args(args);
        if type_args.len() != 1 {
            return Err(BindgenError::UnsupportedType(format!(
                "`{}` expects 1 type argument, got {}",
                path.path,
                type_args.len(),
            )));
        }
        self.resolve_rustdoc_type(type_args[0])
    }

    fn two_generic_args(
        &self,
        path: &rustdoc_types::Path,
    ) -> Result<(FFIType, FFIType), BindgenError> {
        let args = path.args.as_deref().ok_or_else(|| {
            BindgenError::UnsupportedType(format!("`{}` requires type arguments", path.path))
        })?;

        let type_args = angle_type_args(args);
        if type_args.len() < 2 {
            return Err(BindgenError::UnsupportedType(format!(
                "`{}` expects at least 2 type arguments, got {}",
                path.path,
                type_args.len(),
            )));
        }

        let first = self.resolve_rustdoc_type(type_args[0])?;
        let second = self.resolve_rustdoc_type(type_args[1])?;

        Ok((first, second))
    }

    /// Walk an `FFIType` from Phase 1 and fill in any empty `crate_id` fields.
    fn resolve_ffi_type(&self, ty: FFIType) -> Result<FFIType, BindgenError> {
        match ty {
            FFIType::Data(ref tr) if tr.crate_id.name.is_empty() => {
                Ok(FFIType::Data(self.resolve_placeholder_ref(tr)))
            }
            FFIType::Opaque(ref tr) if tr.crate_id.name.is_empty() => {
                Ok(FFIType::Opaque(self.resolve_placeholder_ref(tr)))
            }
            FFIType::Option(inner) => Ok(FFIType::Option(Box::new(self.resolve_ffi_type(*inner)?))),
            FFIType::Result(ok, err) => {
                Ok(FFIType::Result(
                    Box::new(self.resolve_ffi_type(*ok)?),
                    Box::new(self.resolve_ffi_type(*err)?),
                ))
            }
            FFIType::Vec(inner) => Ok(FFIType::Vec(Box::new(self.resolve_ffi_type(*inner)?))),
            FFIType::Map(k, v) => {
                Ok(FFIType::Map(
                    Box::new(self.resolve_ffi_type(*k)?),
                    Box::new(self.resolve_ffi_type(*v)?),
                ))
            }
            FFIType::Set(inner) => Ok(FFIType::Set(Box::new(self.resolve_ffi_type(*inner)?))),
            other => Ok(other),
        }
    }

    fn resolve_placeholder_ref(&self, placeholder: &TypeRef) -> TypeRef {
        let name = &placeholder.name;

        if let Some(found) = self.find_type_in_path_index(name) {
            return found;
        }

        for dep in self.pkg_schemas {
            if let Some(tr) = find_type_ref_in_dep(dep, name) {
                return tr;
            }
        }

        if self.local_decls.contains_key(name) {
            return TypeRef {
                crate_id: CrateId {
                    name: self.crate_name.clone(),
                    version: self.crate_version.clone(),
                },
                module_path: Vec::new(),
                name: name.clone(),
            };
        }

        warn!("could not resolve TypeRef for `{name}`");

        placeholder.clone()
    }

    fn find_type_in_path_index(&self, name: &str) -> Option<TypeRef> {
        let ids = self.name_to_ids.get(name)?;

        let mut best: Option<(&Vec<String>, u32)> = None;
        for id in ids {
            if let Some((path, crate_id)) = self.path_index.get(id) {
                if *crate_id == 0 {
                    best = Some((path, 0));
                    break;
                }
                if best.is_none() {
                    best = Some((path, *crate_id));
                }
            }
        }

        let (path, crate_id_u32) = best?;
        let type_name = path.last().cloned().unwrap_or_else(|| name.to_string());
        let module_path = if path.len() > 2 {
            path[1..path.len() - 1].to_vec()
        } else {
            Vec::new()
        };
        let (crate_pkg, version) = self.crate_pkg_for_id(crate_id_u32);

        Some(TypeRef {
            crate_id: CrateId {
                name: crate_pkg,
                version,
            },
            module_path,
            name: type_name,
        })
    }

    fn find_function_item(&self, rust_name: &str) -> Option<&Item> {
        let ids = self.name_to_ids.get(rust_name)?;
        let mut fallback = None;

        for id in ids {
            if let Some(item) = self.krate.index.get(id)
                && matches!(item.inner, ItemEnum::Function(_))
            {
                if item.crate_id == 0 {
                    return Some(item);
                }

                if fallback.is_none() {
                    fallback = Some(item);
                }
            }
        }

        fallback
    }

    fn find_struct_item(&self, name: &str) -> Option<&Item> {
        let ids = self.name_to_ids.get(name)?;
        ids.iter().find_map(|id| {
            let item = self.krate.index.get(id)?;

            if matches!(item.inner, ItemEnum::Struct(_)) && item.crate_id == 0 {
                Some(item)
            } else {
                None
            }
        })
    }

    fn find_enum_item(&self, name: &str) -> Option<&Item> {
        let ids = self.name_to_ids.get(name)?;
        ids.iter().find_map(|id| {
            let item = self.krate.index.get(id)?;

            if matches!(item.inner, ItemEnum::Enum(_)) && item.crate_id == 0 {
                Some(item)
            } else {
                None
            }
        })
    }

    /// Find an inherent method on `parent_struct` by walking its impl list.
    ///
    /// Algorithm:
    ///   1. Find the struct item for `parent_struct`.
    ///   2. Walk its `impls` list.
    ///   3. Skip trait impls.
    ///   4. In each inherent impl, search for an item named `method_name`.
    fn find_method_item(&self, parent_struct: &str, method_name: &str) -> Option<&Item> {
        let struct_item = self.find_struct_item(parent_struct)?;
        let struct_ = match &struct_item.inner {
            ItemEnum::Struct(s) => s,
            _ => return None,
        };

        for impl_id in &struct_.impls {
            let impl_item = self.krate.index.get(impl_id)?;
            let impl_ = match &impl_item.inner {
                ItemEnum::Impl(i) => i,
                _ => continue,
            };

            if impl_.trait_.is_some() {
                continue; // skip trait impls
            }

            for method_id in &impl_.items {
                if let Some(method_item) = self.krate.index.get(method_id)
                    && method_item.name.as_deref() == Some(method_name)
                    && matches!(method_item.inner, ItemEnum::Function(_))
                {
                    return Some(method_item);
                }
            }
        }

        None
    }

    fn crate_pkg_for_id(&self, crate_id_u32: u32) -> (String, String) {
        if crate_id_u32 == 0 {
            return (self.crate_name.clone(), self.crate_version.clone());
        }

        let ext_name = self
            .krate
            .external_crates
            .get(&crate_id_u32)
            .map(|ec| ec.name.clone())
            .unwrap_or_default();

        let version = self
            .pkg_schemas
            .iter()
            .find(|d| d.crate_name.replace('-', "_") == ext_name)
            .map(|d| d.version.clone())
            .unwrap_or_default();

        let crate_pkg = self
            .pkg_schemas
            .iter()
            .find(|d| d.crate_name.replace('-', "_") == ext_name)
            .map(|d| d.crate_name.clone())
            .unwrap_or(ext_name);

        (crate_pkg, version)
    }

    fn dep_opaque(&self, crate_name: &str, type_name: &str) -> bool {
        self.pkg_schemas
            .iter()
            .find(|d| d.crate_name == crate_name || d.crate_name.replace('-', "_") == crate_name)
            .map(|d| d.is_opaque(type_name))
            .unwrap_or(false)
    }

    pub fn collect_imports(
        &self,
        structs: &[StructInfo],
        enums: &[EnumInfo],
        functions: &[FnInfo],
    ) -> Vec<TypeRef> {
        let mut seen = HashSet::new();
        let mut imports = Vec::new();

        let mut collect = |ty: &FFIType| {
            for type_ref in ty.collect_type_refs() {
                if type_ref.crate_id.name.is_empty() {
                    continue;
                }
                if type_ref.crate_id.name == self.crate_name {
                    continue;
                }
                if type_ref.crate_id.name.replace('-', "_") == self.crate_ident {
                    continue;
                }

                let key = type_ref.qualified_name();
                if seen.insert(key) {
                    imports.push(type_ref.clone());
                }
            }
        };

        for s in structs {
            for f in &s.fields {
                collect(&f.ty);
            }
        }

        for e in enums {
            for v in &e.variants {
                for f in &v.fields {
                    collect(&f.ty);
                }
            }
        }

        for fn_info in functions {
            for p in &fn_info.params {
                collect(&p.ty);
            }
            collect(&fn_info.ret_ty);
        }

        imports
    }
}

fn find_in_dep(dep: &CrateInterface, name: &str) -> Option<FFIType> {
    let type_ref = find_type_ref_in_dep(dep, name)?;
    let is_opaque = dep.is_opaque(name);

    Some(if is_opaque {
        FFIType::Opaque(type_ref)
    } else {
        FFIType::Data(type_ref)
    })
}

fn find_type_ref_in_dep(dep: &CrateInterface, name: &str) -> Option<TypeRef> {
    let in_structs = dep.structs.iter().any(|s| s.name == name);
    let in_enums = dep.enums.iter().any(|e| e.name == name);
    if !in_structs && !in_enums {
        return None;
    }

    Some(TypeRef {
        crate_id: CrateId {
            name: dep.crate_name.clone(),
            version: dep.version.clone(),
        },
        module_path: Vec::new(),
        name: name.to_string(),
    })
}

fn primitive_to_ffi(name: &str) -> Result<FFIType, BindgenError> {
    Ok(match name {
        "bool" => FFIType::Bool,
        "i8" => FFIType::I8,
        "i16" => FFIType::I16,
        "i32" => FFIType::I32,
        "i64" => FFIType::I64,
        "isize" => FFIType::I64,
        "u8" => FFIType::U8,
        "u16" => FFIType::U16,
        "u32" => FFIType::U32,
        "u64" => FFIType::U64,
        "usize" => FFIType::U64,
        "f32" => FFIType::F32,
        "f64" => FFIType::F64,
        "()" => FFIType::Unit,
        "str" => FFIType::String,
        other => {
            return Err(BindgenError::UnsupportedType(format!(
                "Unknown primitive: `{other}`"
            )));
        }
    })
}

fn angle_type_args(args: &GenericArgs) -> Vec<&Type> {
    match args {
        GenericArgs::AngleBracketed { args, .. } => {
            args.iter()
                .filter_map(|a| {
                    if let GenericArg::Type(t) = a {
                        Some(t)
                    } else {
                        None
                    }
                })
                .collect()
        }
        GenericArgs::Parenthesized { inputs, .. } => inputs.iter().collect(),
        GenericArgs::ReturnTypeNotation => Vec::new(),
    }
}

pub fn crate_name_from_toml(cargo_toml: &Path) -> Result<String, BindgenError> {
    let content = fs::read_to_string(cargo_toml).map_err(|e| {
        BindgenError::IoError(io::Error::new(
            e.kind(),
            format!("{}: {}", cargo_toml.display(), e),
        ))
    })?;
    let value: toml::Value = toml::from_str(&content)
        .map_err(|e| BindgenError::UnsupportedType(format!("Invalid Cargo.toml: {e}")))?;

    value
        .get("package")
        .and_then(|p| p.get("name"))
        .and_then(|n| n.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| {
            BindgenError::UnsupportedType(format!(
                "Missing [package].name in {}",
                cargo_toml.display()
            ))
        })
}
