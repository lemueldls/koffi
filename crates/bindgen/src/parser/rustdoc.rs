use std::{
    collections::HashMap,
    fs,
    hash::BuildHasher,
    path::{Path, PathBuf},
};

use koffi_ir::{CrateId, CrateInterface, FFIType, TypeRef};
use rustdoc_types::{Crate, Id, Type};

use crate::{BindgenError, parser::visitor::PartialInterface};

/// Run `cargo rustdoc --output-format json` and return the path to the JSON.
pub fn ensure_json(crate_path: &Path, workspace_root: &Path) -> Result<PathBuf, BindgenError> {
    let output = std::process::Command::new("cargo")
        .args([
            "-Z",
            "unstable-options",
            "rustdoc",
            "--output-format",
            "json",
            "--",
            "--document-private-items",
        ])
        .current_dir(crate_path)
        .output()?;

    if !output.status.success() {
        return Err(BindgenError::RustdocFailed(
            String::from_utf8_lossy(&output.stderr).into_owned(),
        ));
    }

    // Locate the JSON file in target/doc/
    let crate_name = crate_path
        .join("Cargo.toml")
        .pipe(|p| cargo_name_from_toml(&p))?
        .replace('-', "_");
    let json_path = workspace_root
        .join("target/doc")
        .join(format!("{crate_name}.json"));

    Ok(json_path)
}

/// Build a lookup table: rustdoc Id -> fully-qualified path segments.
fn build_path_index(krate: &Crate) -> HashMap<&Id, Vec<String>> {
    let mut index = HashMap::new();
    for (id, paths) in &krate.paths {
        index.insert(id, paths.path.clone());
    }

    index
}

/// Resolve a rustdoc `Type` to our `FFIType`.
pub fn resolve_type<S: BuildHasher>(
    ty: &Type,
    krate: &Crate,
    path_index: &HashMap<&Id, Vec<String>, S>,
    dep_schemas: &[CrateInterface],
    local_ir: &PartialInterface,
) -> Result<FFIType, BindgenError> {
    match ty {
        Type::Primitive(name) => Ok(primitive_to_ffi(name)?),

        Type::ResolvedPath(rp) => {
            let path = path_index
                .get(&rp.id)
                .map(|v| v.as_slice())
                .unwrap_or_default();

            // Is this a known std type?
            if let Some(std_ty) = resolve_std_type(
                path,
                rp.args.as_deref(),
                krate,
                path_index,
                dep_schemas,
                local_ir,
            )? {
                return Ok(std_ty);
            }

            // User type. Determine if opaque or data.
            let type_name = path.last().cloned().unwrap_or_default();
            let module_path: Vec<String> = path[..path.len().saturating_sub(1)].to_vec();
            let crate_name = path.first().cloned().unwrap_or_default();

            // Look up crate version from dep schemas or local
            let version = dep_schemas
                .iter()
                .find(|s| s.crate_name.replace('-', "_") == crate_name)
                .map(|s| s.version.clone())
                .unwrap_or_else(|| local_ir.version.clone());

            let is_opaque = local_ir
                .structs
                .iter()
                .any(|s| s.name == type_name && s.is_opaque)
                || dep_schemas.iter().any(|dep| dep.is_opaque(&type_name));

            let type_ref = TypeRef {
                crate_id: CrateId {
                    name: crate_name,
                    version,
                },
                module_path,
                name: type_name,
                schema_hash: 0, // filled in after full IR is built
            };

            Ok(if is_opaque {
                FFIType::Opaque(type_ref)
            } else {
                FFIType::Data(type_ref)
            })
        }

        Type::BorrowedRef { type_, .. } => {
            // &str -> String, &[u8] -> Bytes, &T -> recurse
            match type_.as_ref() {
                Type::Primitive(p) if p == "str" => Ok(FFIType::String),
                Type::Slice(inner) => {
                    let inner_ty = resolve_type(inner, krate, path_index, dep_schemas, local_ir)?;
                    if inner_ty == FFIType::U8 {
                        Ok(FFIType::Bytes)
                    } else {
                        Err(BindgenError::UnsupportedType("&[T] where T != u8".into()))
                    }
                }
                _ => resolve_type(type_, krate, path_index, dep_schemas, local_ir),
            }
        }

        Type::Tuple(elems) if elems.is_empty() => Ok(FFIType::Unit),

        _ => Err(BindgenError::UnsupportedType(format!("{ty:?}"))),
    }
}

pub fn resolve_types(
    local_ir: PartialInterface,
    _rustdoc_json: &Path,
    _dep_schemas: &[CrateInterface],
) -> Result<CrateInterface, BindgenError> {
    // let rustdoc_str = fs::read_to_string(rustdoc_json)?;
    // let krate: Crate = serde_json::from_str(&rustdoc_str)?;

    Ok(CrateInterface {
        namespace: local_ir.namespace,
        crate_name: local_ir.crate_name,
        version: local_ir.version,
        structs: local_ir.structs,
        enums: local_ir.enums,
        functions: local_ir.functions,
        imports: vec![],
    })
}

fn resolve_std_type<S: BuildHasher>(
    path: &[String],
    args: Option<&rustdoc_types::GenericArgs>,
    krate: &Crate,
    path_index: &HashMap<&Id, Vec<String>, S>,
    dep_schemas: &[CrateInterface],
    local_ir: &PartialInterface,
) -> Result<Option<FFIType>, BindgenError> {
    // Normalise path to "crate::module::Name" form for matching
    let full = path.join("::");

    match full.as_str() {
        "std::string::String" | "alloc::string::String" => return Ok(Some(FFIType::String)),
        "std::vec::Vec" | "alloc::vec::Vec" => {
            let inner = single_generic(args, krate, path_index, dep_schemas, local_ir)?;

            return Ok(Some(if inner == FFIType::U8 {
                FFIType::Bytes
            } else {
                FFIType::Vec(Box::new(inner))
            }));
        }
        "std::option::Option" | "core::option::Option" => {
            let inner = single_generic(args, krate, path_index, dep_schemas, local_ir)?;
            return Ok(Some(FFIType::Option(Box::new(inner))));
        }
        "std::result::Result" | "core::result::Result" => {
            let (ok, err) = two_generics(args, krate, path_index, dep_schemas, local_ir)?;
            return Ok(Some(FFIType::Result(Box::new(ok), Box::new(err))));
        }
        "std::collections::HashMap" | "std::collections::BTreeMap" => {
            let (k, v) = two_generics(args, krate, path_index, dep_schemas, local_ir)?;
            return Ok(Some(FFIType::Map(Box::new(k), Box::new(v))));
        }
        "std::collections::HashSet" | "std::collections::BTreeSet" => {
            let inner = single_generic(args, krate, path_index, dep_schemas, local_ir)?;
            return Ok(Some(FFIType::Set(Box::new(inner))));
        }
        _ => {}
    }

    Ok(None)
}

fn primitive_to_ffi(name: &str) -> Result<FFIType, BindgenError> {
    Ok(match name {
        "bool" => FFIType::Bool,
        "i8" => FFIType::I8,
        "i16" => FFIType::I16,
        "i32" => FFIType::I32,
        "i64" => FFIType::I64,
        "u8" => FFIType::U8,
        "u16" => FFIType::U16,
        "u32" => FFIType::U32,
        "u64" => FFIType::U64,
        "f32" => FFIType::F32,
        "f64" => FFIType::F64,
        "()" => FFIType::Unit,
        other => return Err(BindgenError::UnsupportedType(other.into())),
    })
}

fn single_generic<S: BuildHasher>(
    args: Option<&rustdoc_types::GenericArgs>,
    krate: &Crate,
    path_index: &HashMap<&Id, Vec<String>, S>,
    dep_schemas: &[CrateInterface],
    local_ir: &PartialInterface,
) -> Result<FFIType, BindgenError> {
    if let Some(generic_args) = args
        && let rustdoc_types::GenericArgs::AngleBracketed { args, .. } = generic_args
        && args.len() == 1
        && let rustdoc_types::GenericArg::Type(ty) = &args[0]
    {
        return resolve_type(ty, krate, path_index, dep_schemas, local_ir);
    }

    Err(BindgenError::UnsupportedType(
        "Expected exactly one generic argument".into(),
    ))
}

fn two_generics<S: BuildHasher>(
    args: Option<&rustdoc_types::GenericArgs>,
    krate: &Crate,
    path_index: &HashMap<&Id, Vec<String>, S>,
    dep_schemas: &[CrateInterface],
    local_ir: &PartialInterface,
) -> Result<(FFIType, FFIType), BindgenError> {
    if let Some(generic_args) = args
        && let rustdoc_types::GenericArgs::AngleBracketed { args, .. } = generic_args
        && args.len() == 2
        && let (rustdoc_types::GenericArg::Type(ty1), rustdoc_types::GenericArg::Type(ty2)) =
            (&args[0], &args[1])
    {
        let resolved1 = resolve_type(ty1, krate, path_index, dep_schemas, local_ir)?;
        let resolved2 = resolve_type(ty2, krate, path_index, dep_schemas, local_ir)?;

        return Ok((resolved1, resolved2));
    }

    Err(BindgenError::UnsupportedType(
        "Expected exactly two generic arguments".into(),
    ))
}

fn cargo_name_from_toml(path: &Path) -> Result<String, BindgenError> {
    let content = fs::read_to_string(path)?;
    let value: toml::Value = toml::from_str(&content)?;
    if let Some(name) = value
        .get("package")
        .and_then(|pkg| pkg.get("name"))
        .and_then(|n| n.as_str())
    {
        Ok(name.to_string())
    } else {
        Err(BindgenError::CargoTomlError(
            "Missing [package] name in Cargo.toml".into(),
        ))
    }
}

// Extension trait for ergonomic method chaining in pipe()
trait Pipe: Sized {
    fn pipe<F, R>(self, f: F) -> R
    where F: FnOnce(Self) -> R {
        f(self)
    }
}

impl<T> Pipe for T {}
