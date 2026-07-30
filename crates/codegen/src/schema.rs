use std::collections::BTreeMap;

use heck::ToLowerCamelCase;
use koffi_core::FnShapeRef;

use crate::layout::{StructLayoutInfo, compute_struct_layout};

#[derive(Debug, Clone)]
pub struct Schema {
    pub crate_name: String,
    pub functions: Vec<SchemaFn>,
    pub structs: Vec<SchemaStruct>,
}

#[derive(Debug, Clone)]
pub struct SchemaFn {
    pub rust_name: String,
    pub kotlin_name: String,
    pub module_path: Option<String>,
    pub receiver: Option<SchemaTypeRef>,
    pub params: Vec<SchemaParam>,
    pub return_type: SchemaTypeRef,
}

#[derive(Debug, Clone)]
pub struct SchemaParam {
    pub name: String,
    pub ty: SchemaTypeRef,
    pub is_receiver: bool,
}

#[derive(Debug, Clone)]
pub struct SchemaStruct {
    pub name: String,
    pub module_path: Option<String>,
    pub fields: Vec<SchemaField>,
    pub layout: StructLayoutInfo,
}

#[derive(Debug, Clone)]
pub struct SchemaField {
    pub name: String,
    pub ty: SchemaTypeRef,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SchemaTypeRef {
    Scalar(ScalarKind),
    Struct {
        name: String,
        module_path: Option<String>,
    },
}

impl SchemaTypeRef {
    #[must_use]
    pub fn abi_ident_infix(&self) -> String {
        match self {
            SchemaTypeRef::Struct { name, module_path } => {
                let mod_infix = module_path.as_deref().unwrap_or("").replace("::", "_");
                format!("{mod_infix}_{name}")
            }
            SchemaTypeRef::Scalar(k) => k.rust_type_name().to_string(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScalarKind {
    Bool,
    U8,
    U16,
    U32,
    U64,
    I8,
    I16,
    I32,
    I64,
    F32,
    F64,
}

pub fn build_schema(crate_name: String, fn_entries: &[FnShapeRef]) -> anyhow::Result<Schema> {
    let mut structs: BTreeMap<(Option<String>, String), SchemaStruct> = BTreeMap::new();
    let mut functions = Vec::new();

    for entry in fn_entries {
        let rust_name = entry.name.to_owned();
        let kotlin_name = entry.name.trim_start_matches("r#").to_lower_camel_case();
        let module_path = entry.module_path.map(|p| p.to_owned());

        let receiver = entry
            .receiver
            .map(|ty| convert_shape(ty.shape(), &mut structs))
            .transpose()?;

        let mut params = entry
            .params
            .iter()
            .map(|p| {
                Ok(SchemaParam {
                    name: p.name.to_string(),
                    ty: convert_shape(p.param_type.shape(), &mut structs)?,
                    is_receiver: false,
                })
            })
            .collect::<anyhow::Result<Vec<_>>>()?;

        // See the doc comment on `SchemaFn::receiver`: it isn't its own
        // field on FnShapeRef, it's positional, always params[0] when
        // receiver is Some. Marking it here, once, is what lets every
        // consumer (both templates) branch on `p.is_receiver` instead of
        // each having to know and re-derive that convention itself.
        if receiver.is_some()
            && let Some(first) = params.first_mut()
        {
            first.is_receiver = true;
        }

        let return_type = convert_shape(entry.return_type.shape(), &mut structs)?;

        functions.push(SchemaFn {
            rust_name,
            kotlin_name,
            module_path,
            receiver,
            params,
            return_type,
        });
    }

    Ok(Schema {
        crate_name,
        functions,
        structs: structs.into_values().collect(),
    })
}

fn convert_shape(
    shape: &'static facet::Shape,
    structs: &mut BTreeMap<(Option<String>, String), SchemaStruct>,
) -> anyhow::Result<SchemaTypeRef> {
    match shape.def {
        facet::Def::Scalar => Ok(SchemaTypeRef::Scalar(scalar_kind_of(shape)?)),
        facet::Def::Undefined => {
            match shape.ty {
                facet::Type::User(facet::UserType::Struct(s)) => {
                    let module_path = shape.module_path.map(|p| p.to_owned());
                    let name = shape.effective_name().to_string();
                    let key = (module_path.clone(), name.clone());

                    if !structs.contains_key(&key) {
                        let fields = s
                            .fields
                            .iter()
                            .map(|f| {
                                let field_shape = f.shape();
                                Ok(SchemaField {
                                    name: f.effective_name().to_string(),
                                    ty: convert_shape(field_shape, structs)?,
                                })
                            })
                            .collect::<anyhow::Result<Vec<_>>>()?;

                        // Safe to compute the layout right away: every field's
                        // own SchemaTypeRef, including any nested struct's, is
                        // already fully resolved (inserted into `structs`) by
                        // the recursive convert_shape calls just above, before
                        // this struct's own entry goes in below. A true cycle
                        // (X containing Y containing X by value) can't reach
                        // this code in the first place, Rust won't compile an
                        // infinitely-sized value type.
                        let layout = compute_struct_layout(&fields, structs)?;
                        structs.insert(key.clone(), SchemaStruct {
                            name,
                            module_path,
                            fields,
                            layout,
                        });
                    }

                    Ok(SchemaTypeRef::Struct {
                        name: key.1,
                        module_path: key.0,
                    })
                }
                _ => {
                    anyhow::bail!(
                        "koffi M0 only supports plain structs and scalars, got: {shape:?}"
                    )
                }
            }
        }
        _ => anyhow::bail!("koffi M0 only supports plain structs and scalars: {shape:?}"),
    }
}

fn scalar_kind_of(shape: &'static facet::Shape) -> anyhow::Result<ScalarKind> {
    match shape.effective_name() {
        "bool" => Ok(ScalarKind::Bool),
        "u8" => Ok(ScalarKind::U8),
        "u16" => Ok(ScalarKind::U16),
        "u32" => Ok(ScalarKind::U32),
        "u64" => Ok(ScalarKind::U64),
        "i8" => Ok(ScalarKind::I8),
        "i16" => Ok(ScalarKind::I16),
        "i32" => Ok(ScalarKind::I32),
        "i64" => Ok(ScalarKind::I64),
        "f32" => Ok(ScalarKind::F32),
        "f64" => Ok(ScalarKind::F64),
        other => anyhow::bail!("koffi M0 doesn't recognize scalar `{other}`"),
    }
}
