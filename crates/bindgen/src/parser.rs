use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FFIType {
    Bool,
    I8,
    I16,
    I32,
    I64,
    U8,
    U16,
    U32,
    U64,
    F32,
    F64,
    Unit,
    String,
    Bytes, // Vec<u8> or &[u8]
    Option(Box<FFIType>),
    Result(Box<FFIType>, Box<FFIType>),
    Vec(Box<FFIType>),
    Map(Box<FFIType>, Box<FFIType>),
    Custom(String), // User struct or enum
}

impl FFIType {
    pub const fn is_blittable(&self) -> bool {
        matches!(
            self,
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
        )
    }
}

#[derive(Debug, Clone, Default)]
pub struct ExportArgs {
    pub name: Option<String>,
    pub package: Option<String>,
    pub blocking: bool,
    pub deprecated: Option<String>,
}

#[derive(Debug, Clone)]
pub struct FieldInfo {
    pub name: String,
    pub ty: FFIType,
}

#[derive(Debug, Clone)]
pub struct StructInfo {
    pub name: String,
    pub is_opaque: bool,
    pub fields: Vec<FieldInfo>,
}

#[derive(Debug, Clone)]
pub struct EnumVariantInfo {
    pub name: String,
    pub fields: Vec<FieldInfo>,
}

#[derive(Debug, Clone)]
pub struct EnumInfo {
    pub name: String,
    pub variants: Vec<EnumVariantInfo>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReceiverType {
    Ref,    // &self
    RefMut, // &mut self
    Owned,  // self
}

#[derive(Debug, Clone)]
pub struct ParamInfo {
    pub name: String,
    pub ty: FFIType,
}

#[derive(Debug, Clone)]
pub struct FnInfo {
    pub name: String,
    pub rust_name: String,
    pub is_async: bool,
    pub params: Vec<ParamInfo>,
    pub ret_ty: FFIType,
    pub receiver: Option<ReceiverType>,
    pub parent_struct: Option<String>,
    pub args: ExportArgs,
}

pub struct CrateInterface {
    pub namespace: String,
    pub structs: Vec<StructInfo>,
    pub enums: Vec<EnumInfo>,
    pub functions: Vec<FnInfo>,
}

pub fn has_koffi_attr(attrs: &[syn::Attribute], name: &str) -> bool {
    attrs.iter().any(|attr| {
        let is_match = |ident: &syn::Ident| {
            let s = ident.to_string();
            s == name || s == format!("r#{name}")
        };

        if attr.path().segments.len() == 1 {
            is_match(&attr.path().segments[0].ident)
        } else if attr.path().segments.len() == 2 {
            attr.path().segments[0].ident == "koffi" && is_match(&attr.path().segments[1].ident)
        } else {
            false
        }
    })
}

pub fn get_koffi_attr(attrs: &[syn::Attribute], name: &str) -> Option<syn::Attribute> {
    attrs
        .iter()
        .find(|attr| {
            let is_match = |ident: &syn::Ident| {
                let s = ident.to_string();
                s == name || s == format!("r#{name}")
            };
            if attr.path().segments.len() == 1 {
                is_match(&attr.path().segments[0].ident)
            } else if attr.path().segments.len() == 2 {
                attr.path().segments[0].ident == "koffi" && is_match(&attr.path().segments[1].ident)
            } else {
                false
            }
        })
        .cloned()
}

pub fn parse_export_args(attr: &syn::Attribute) -> ExportArgs {
    let mut args = ExportArgs::default();
    if let syn::Meta::List(meta_list) = &attr.meta {
        let _ = meta_list.parse_nested_meta(|meta| {
            if meta.path.is_ident("name") {
                let value = meta.value()?;
                let s: syn::LitStr = value.parse()?;
                args.name = Some(s.value());
            } else if meta.path.is_ident("package") {
                let value = meta.value()?;
                let s: syn::LitStr = value.parse()?;
                args.package = Some(s.value());
            } else if meta.path.is_ident("blocking") {
                args.blocking = true;
            } else if meta.path.is_ident("deprecated") {
                let value = meta.value()?;
                let s: syn::LitStr = value.parse()?;
                args.deprecated = Some(s.value());
            }
            Ok(())
        });
    }

    args
}

pub fn parse_type(ty: &syn::Type) -> Result<FFIType, String> {
    match ty {
        syn::Type::Path(type_path) => {
            let segment = type_path.path.segments.last().ok_or("Empty type path")?;
            let name = segment.ident.to_string();

            match name.as_str() {
                "bool" => Ok(FFIType::Bool),
                "i8" => Ok(FFIType::I8),
                "i16" => Ok(FFIType::I16),
                "i32" => Ok(FFIType::I32),
                "i64" => Ok(FFIType::I64),
                "u8" => Ok(FFIType::U8),
                "u16" => Ok(FFIType::U16),
                "u32" => Ok(FFIType::U32),
                "u64" => Ok(FFIType::U64),
                "f32" => Ok(FFIType::F32),
                "f64" => Ok(FFIType::F64),
                "String" => Ok(FFIType::String),
                "Option" => {
                    let generic_arg = get_single_generic_arg(segment)?;
                    let inner = parse_type(generic_arg)?;

                    Ok(FFIType::Option(Box::new(inner)))
                }
                "Result" => {
                    let (arg1, arg2) = get_two_generic_args(segment)?;
                    let inner1 = parse_type(arg1)?;
                    let inner2 = parse_type(arg2)?;

                    Ok(FFIType::Result(Box::new(inner1), Box::new(inner2)))
                }
                "Vec" => {
                    let generic_arg = get_single_generic_arg(segment)?;
                    let inner = parse_type(generic_arg)?;

                    if inner == FFIType::U8 {
                        Ok(FFIType::Bytes)
                    } else {
                        Ok(FFIType::Vec(Box::new(inner)))
                    }
                }
                "HashMap" => {
                    let (arg1, arg2) = get_two_generic_args(segment)?;
                    let inner1 = parse_type(arg1)?;
                    let inner2 = parse_type(arg2)?;

                    Ok(FFIType::Map(Box::new(inner1), Box::new(inner2)))
                }
                _ => Ok(FFIType::Custom(name)),
            }
        }
        syn::Type::Reference(type_ref) => {
            match &*type_ref.elem {
                syn::Type::Path(type_path) => {
                    let segment = type_path.path.segments.last().ok_or("Empty type path")?;
                    if segment.ident == "str" {
                        return Ok(FFIType::String);
                    }
                }
                syn::Type::Slice(type_slice) => {
                    let inner = parse_type(&type_slice.elem)?;
                    if inner == FFIType::U8 {
                        return Ok(FFIType::Bytes);
                    }
                }
                _ => {}
            }
            parse_type(&type_ref.elem)
        }
        syn::Type::Tuple(type_tuple) => {
            if type_tuple.elems.is_empty() {
                Ok(FFIType::Unit)
            } else {
                Err("Tuples not yet fully supported".to_string())
            }
        }
        _ => Err(format!("Unsupported type: {ty:?}")),
    }
}

fn get_single_generic_arg(segment: &syn::PathSegment) -> Result<&syn::Type, String> {
    if let syn::PathArguments::AngleBracketed(args) = &segment.arguments
        && args.args.len() == 1
        && let syn::GenericArgument::Type(ty) = &args.args[0]
    {
        return Ok(ty);
    }

    Err(format!("Expected 1 generic argument for {}", segment.ident))
}

fn get_two_generic_args(segment: &syn::PathSegment) -> Result<(&syn::Type, &syn::Type), String> {
    if let syn::PathArguments::AngleBracketed(args) = &segment.arguments
        && args.args.len() == 2
        && let (syn::GenericArgument::Type(ty1), syn::GenericArgument::Type(ty2)) =
            (&args.args[0], &args.args[1])
    {
        return Ok((ty1, ty2));
    }

    Err(format!(
        "Expected 2 generic arguments for {}",
        segment.ident
    ))
}

fn find_rs_files(dir: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    if dir.is_dir() {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                files.extend(find_rs_files(&path)?);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                files.push(path);
            }
        }
    }

    Ok(files)
}

pub fn parse_crate(
    crate_path: &Path,
    default_package: Option<String>,
) -> Result<CrateInterface, Box<dyn std::error::Error>> {
    let src_dir = crate_path.join("src");
    let rs_files = find_rs_files(&src_dir)?;

    let mut namespace = default_package.unwrap_or_else(|| "generated".to_string());
    let mut structs = Vec::new();
    let mut enums = Vec::new();
    let mut functions = Vec::new();

    for file_path in rs_files {
        let content = fs::read_to_string(&file_path)?;
        let file = syn::parse_file(&content)?;

        // Check file level namespace attribute
        for attr in &file.attrs {
            if has_koffi_attr(std::slice::from_ref(attr), "namespace")
                && let syn::Meta::List(meta_list) = &attr.meta
                && let Ok(s) = meta_list.parse_args::<syn::LitStr>()
            {
                namespace = s.value();
            }
        }

        for item in file.items {
            match item {
                syn::Item::Struct(item_struct) => {
                    println!(
                        "Struct {} attributes: {:?}",
                        item_struct.ident,
                        item_struct
                            .attrs
                            .iter()
                            .map(|a| {
                                a.path()
                                    .segments
                                    .iter()
                                    .map(|s| s.ident.to_string())
                                    .collect::<Vec<_>>()
                            })
                            .collect::<Vec<_>>()
                    );
                    let is_opaque = has_koffi_attr(&item_struct.attrs, "opaque");
                    let is_type = has_koffi_attr(&item_struct.attrs, "type");
                    if is_opaque || is_type {
                        let name = item_struct.ident.to_string();
                        let mut fields = Vec::new();
                        if is_type {
                            // Extract fields for serialization
                            for field in &item_struct.fields {
                                let f_name = field
                                    .ident
                                    .as_ref()
                                    .map_or_else(String::new, |id| id.to_string());
                                let f_ty = parse_type(&field.ty)?;
                                fields.push(FieldInfo {
                                    name: f_name,
                                    ty: f_ty,
                                });
                            }
                        }
                        structs.push(StructInfo {
                            name,
                            is_opaque,
                            fields,
                        });
                    }
                }
                syn::Item::Enum(item_enum) => {
                    if has_koffi_attr(&item_enum.attrs, "type") {
                        let name = item_enum.ident.to_string();
                        let mut variants = Vec::new();
                        for variant in &item_enum.variants {
                            let v_name = variant.ident.to_string();
                            let mut fields = Vec::new();
                            for (idx, field) in variant.fields.iter().enumerate() {
                                let f_name = field
                                    .ident
                                    .as_ref()
                                    .map_or_else(|| format!("field{idx}"), |id| id.to_string());
                                let f_ty = parse_type(&field.ty)?;
                                fields.push(FieldInfo {
                                    name: f_name,
                                    ty: f_ty,
                                });
                            }
                            variants.push(EnumVariantInfo {
                                name: v_name,
                                fields,
                            });
                        }
                        enums.push(EnumInfo { name, variants });
                    }
                }
                syn::Item::Fn(item_fn) => {
                    if let Some(attr) = get_koffi_attr(&item_fn.attrs, "export") {
                        let rust_name = item_fn.sig.ident.to_string();
                        let args = parse_export_args(&attr);
                        let name = args.name.clone().unwrap_or_else(|| {
                            // Convert snake_case to camelCase
                            heck::AsLowerCamelCase(&rust_name).to_string()
                        });
                        let is_async = item_fn.sig.asyncness.is_some();
                        let mut params = Vec::new();
                        for arg in &item_fn.sig.inputs {
                            if let syn::FnArg::Typed(pat_type) = arg {
                                let p_name = if let syn::Pat::Ident(pat_ident) = &*pat_type.pat {
                                    pat_ident.ident.to_string()
                                } else {
                                    "arg".to_string()
                                };
                                let p_ty = parse_type(&pat_type.ty)?;
                                params.push(ParamInfo {
                                    name: p_name,
                                    ty: p_ty,
                                });
                            }
                        }
                        let ret_ty = match &item_fn.sig.output {
                            syn::ReturnType::Default => FFIType::Unit,
                            syn::ReturnType::Type(_, ty) => parse_type(ty)?,
                        };
                        functions.push(FnInfo {
                            name,
                            rust_name,
                            is_async,
                            params,
                            ret_ty,
                            receiver: None,
                            parent_struct: None,
                            args,
                        });
                    }
                }
                syn::Item::Impl(item_impl) => {
                    // Check if the impl block itself is annotated with #[koffi::export]
                    if let Some(attr) = get_koffi_attr(&item_impl.attrs, "export") {
                        let args = parse_export_args(&attr);
                        if let syn::Type::Path(type_path) = &*item_impl.self_ty {
                            let parent_name =
                                type_path.path.segments.last().unwrap().ident.to_string();
                            for impl_item in &item_impl.items {
                                if let syn::ImplItem::Fn(impl_fn) = impl_item
                                    && matches!(impl_fn.vis, syn::Visibility::Public(_))
                                {
                                    // Only public impl methods
                                    let rust_name = impl_fn.sig.ident.to_string();
                                    let name = heck::AsLowerCamelCase(&rust_name).to_string();
                                    let is_async = impl_fn.sig.asyncness.is_some();
                                    let mut params = Vec::new();
                                    let mut receiver = None;

                                    for arg in &impl_fn.sig.inputs {
                                        match arg {
                                            syn::FnArg::Receiver(rec) => {
                                                if rec.reference.is_some() {
                                                    if rec.mutability.is_some() {
                                                        receiver = Some(ReceiverType::RefMut);
                                                    } else {
                                                        receiver = Some(ReceiverType::Ref);
                                                    }
                                                } else {
                                                    receiver = Some(ReceiverType::Owned);
                                                }
                                            }
                                            syn::FnArg::Typed(pat_type) => {
                                                let p_name = if let syn::Pat::Ident(pat_ident) =
                                                    &*pat_type.pat
                                                {
                                                    pat_ident.ident.to_string()
                                                } else {
                                                    "arg".to_string()
                                                };
                                                let mut p_ty = parse_type(&pat_type.ty)?;
                                                if let FFIType::Custom(name) = &p_ty
                                                    && name == "Self"
                                                {
                                                    p_ty = FFIType::Custom(parent_name.clone());
                                                }
                                                params.push(ParamInfo {
                                                    name: p_name,
                                                    ty: p_ty,
                                                });
                                            }
                                        }
                                    }
                                    let mut ret_ty = match &impl_fn.sig.output {
                                        syn::ReturnType::Default => FFIType::Unit,
                                        syn::ReturnType::Type(_, ty) => parse_type(ty)?,
                                    };
                                    if let FFIType::Custom(name) = &ret_ty
                                        && name == "Self"
                                    {
                                        ret_ty = FFIType::Custom(parent_name.clone());
                                    }
                                    functions.push(FnInfo {
                                        name,
                                        rust_name,
                                        is_async,
                                        params,
                                        ret_ty,
                                        receiver,
                                        parent_struct: Some(parent_name.clone()),
                                        args: args.clone(),
                                    });
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }

    Ok(CrateInterface {
        namespace,
        structs,
        enums,
        functions,
    })
}
