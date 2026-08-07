use std::collections::BTreeMap;

use heck::ToLowerCamelCase;
use koffi_core::FnShapeRef;

use crate::layout::{
    FieldPlacement, StructLayoutInfo, compute_enum_layout, compute_struct_layout,
    compute_wrapper_layout,
};

#[derive(Debug, Clone)]
pub struct Schema {
    pub crate_name: String,
    pub functions: Vec<SchemaFn>,
    pub structs: Vec<SchemaStruct>,
    pub enums: Vec<SchemaEnum>,
    pub wrappers: Vec<SchemaWrapper>,
    pub opaques: Vec<SchemaOpaque>,
}

#[derive(Debug, Clone)]
pub struct SchemaFn {
    pub rust_name: String,
    pub kotlin_name: String,
    pub module_path: Option<String>,
    pub parent: Option<SchemaTypeRef>,
    pub params: Vec<SchemaParam>,
    pub return_type: SchemaTypeRef,
}

impl SchemaFn {
    /// Does this fn take a `self`/`&self`/`&mut self` receiver? Asks the
    /// params directly; a `parent`-having fn can easily have no receiver
    /// (`Payload::new(data: u16) -> Self`).
    #[must_use]
    pub fn has_receiver(&self) -> bool {
        self.params.iter().any(|p| p.is_receiver)
    }

    /// True for a `parent`-having, receiver-less fn whose return type is
    /// its own `parent`: `Payload::new(..) -> Self`-shaped. The Kotlin
    /// generator treats this as a constructor, not an ordinary companion fn.
    #[must_use]
    pub fn is_constructor(&self) -> bool {
        !self.has_receiver() && self.parent.as_ref().is_some_and(|p| *p == self.return_type)
    }

    /// True for a `parent`-having fn that's neither an instance method nor
    /// a constructor (`Payload::describe_format() -> u32`-shaped). Lands in
    /// the Kotlin `companion object` as an ordinary function.
    #[must_use]
    pub fn is_companion_function(&self) -> bool {
        self.parent.is_some() && !self.has_receiver() && !self.is_constructor()
    }
}

#[derive(Debug, Clone)]
pub struct SchemaParam {
    pub name: String,
    pub ty: SchemaTypeRef,
    pub is_receiver: bool,
    /// True for a `&mut self` receiver. Only meaningful for receivers; the
    /// macro always stores `false` for real params.
    pub is_mut_receiver: bool,
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
    /// True when the field carries `#[facet(proxy = X)]`: `ty` is the
    /// proxy's wire type and the generated From impls go through the user's
    /// `TryFrom` pair instead of a plain `.into()`.
    pub is_proxy: bool,
    /// For proxy fields: the real field type's `(name, module_path)`, so
    /// generated conversions can name the `TryFrom` target explicitly.
    /// rustc can't infer it through the `.into()`/`TryFrom` chain
    /// otherwise. `None` for plain fields.
    pub real_ty: Option<(String, Option<String>)>,
}

/// A `#[facet(opaque)]` type.
///
/// Its shape keeps the real layout but exposes no fields; koffi marshals
/// values as pointer-sized handles, so the whole type is represented by one
/// `__koffi_opaque_*` wire struct holding an address.
#[derive(Debug, Clone)]
pub struct SchemaOpaque {
    pub name: String,
    pub module_path: Option<String>,
}

/// A reflected enum with its discriminant scalar kind, whether any
/// variant carries a payload, and the variant list. Data-carrying
/// enums need a primitive repr: rustc rejects `#[repr(C)]` (E0732).
#[derive(Debug, Clone)]
pub struct SchemaEnum {
    pub name: String,
    pub module_path: Option<String>,
    pub discriminant: ScalarKind,
    pub has_data: bool,
    pub variants: Vec<SchemaEnumVariant>,
    pub layout: StructLayoutInfo,
}

#[derive(Debug, Clone)]
pub struct SchemaEnumVariant {
    pub name: String,
    pub discriminant: i64,
    /// Payload fields with absolute offsets into the enum's memory layout
    /// (the discriminant occupies offset 0); empty for unit variants.
    pub placements: Vec<FieldPlacement>,
    /// True for struct variants (`Variant { f: T }`), false for tuple
    /// variants (`Variant(T)`). Unit variants have empty placements; Rust
    /// distinguishes tuple and struct variants even when both carry a
    /// single field, and the generated glue must construct them the same
    /// way.
    pub is_struct_variant: bool,
}

/// An `Option<T>` or `Result<T, E>` used anywhere in the schema.
///
/// Each distinct instantiation gets one wire type: a `#[repr(C)]` struct
/// with a `u8` discriminant (`0` = None/Err, `1` = Some/Ok) and a payload
/// union holding the inner wire type(s), mirroring the data-enum wire format.
#[derive(Debug, Clone)]
pub struct SchemaWrapper {
    pub kind: WrapperKind,
    pub unique_ident: String,
    pub members: Vec<WrapperMember>,
    pub layout: StructLayoutInfo,
}

impl SchemaWrapper {
    /// Payload union wire ident: `__koffi_option_u32_payload`. Follows the
    /// `__koffi_enum_*_payload` convention so the C header reads the same.
    #[must_use]
    pub fn union_ident(&self) -> String {
        format!("{}_payload", self.unique_ident)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WrapperKind {
    Option,
    Result,
}

/// One payload union member of a wrapper: `some` for Option, `ok`/`err`
/// for Result, with the member's (already converted) type.
#[derive(Debug, Clone)]
pub struct WrapperMember {
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
    Enum {
        name: String,
        module_path: Option<String>,
        discriminant: ScalarKind,
        has_data: bool,
    },
    Option {
        inner: Box<SchemaTypeRef>,
    },
    Result {
        ok: Box<SchemaTypeRef>,
        err: Box<SchemaTypeRef>,
    },
    Opaque {
        name: String,
        module_path: Option<String>,
    },
}

impl SchemaTypeRef {
    #[must_use]
    pub fn abi_ident_infix(&self) -> String {
        match self {
            SchemaTypeRef::Struct { name, module_path }
            | SchemaTypeRef::Enum {
                name, module_path, ..
            }
            | SchemaTypeRef::Opaque { name, module_path } => {
                let mod_infix = module_path.as_deref().unwrap_or("").replace("::", "_");
                format!("{mod_infix}_{name}")
            }
            SchemaTypeRef::Scalar(k) => k.rust_type_name().to_string(),
            // Wrappers are only ever fn params/returns/fields, never an
            // `impl` block's Self type, so they can't be a `parent`.
            SchemaTypeRef::Option { .. } | SchemaTypeRef::Result { .. } => {
                unreachable!("an Option/Result can't be an impl-block parent")
            }
        }
    }

    #[must_use]
    pub fn same_struct(&self, s: &SchemaStruct) -> bool {
        matches!(self, SchemaTypeRef::Struct { name, module_path }
            if name == &s.name && module_path == &s.module_path)
    }

    #[must_use]
    pub fn same_enum(&self, e: &SchemaEnum) -> bool {
        matches!(self, SchemaTypeRef::Enum { name, module_path, .. }
            if name == &e.name && module_path == &e.module_path)
    }

    /// True for a data-carrying enum: the FFI layer marshals it through a
    /// memory segment (with a per-enum `toFfm`/`fromFfm` pair), while a
    /// fieldless enum crosses the ABI as its plain discriminant scalar.
    #[must_use]
    pub const fn has_data(&self) -> bool {
        matches!(self, SchemaTypeRef::Enum { has_data: true, .. })
    }

    /// True for an opaque handle type. These cross the ABI as addresses,
    /// never through the struct/union wire types.
    #[must_use]
    pub const fn is_opaque(&self) -> bool {
        matches!(self, SchemaTypeRef::Opaque { .. })
    }

    #[must_use]
    pub fn same_opaque(&self, o: &SchemaOpaque) -> bool {
        matches!(self, SchemaTypeRef::Opaque { name, module_path }
            if name == &o.name && module_path == &o.module_path)
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
    let mut enums: BTreeMap<(Option<String>, String), SchemaEnum> = BTreeMap::new();
    let mut wrappers: BTreeMap<String, SchemaWrapper> = BTreeMap::new();
    let mut opaques: Vec<SchemaOpaque> = Vec::new();
    let mut functions = Vec::new();

    for entry in fn_entries {
        let rust_name = entry.name.to_owned();
        let kotlin_name = entry.name.trim_start_matches("r#").to_lower_camel_case();
        let module_path = entry.module_path.map(|p| p.to_owned());

        let parent = entry
            .parent
            .map(|ty| {
                convert_shape(
                    ty.shape(),
                    &mut structs,
                    &mut enums,
                    &mut wrappers,
                    &mut opaques,
                )
            })
            .transpose()?;

        // `is_receiver` comes straight from the macro; it can't be
        // reconstructed from `parent` (`Payload::new` has a `parent` and
        // zero receiver params).
        let params = entry
            .params
            .iter()
            .map(|p| {
                Ok(SchemaParam {
                    name: p.name.to_string(),
                    ty: convert_shape(
                        p.param_type.shape(),
                        &mut structs,
                        &mut enums,
                        &mut wrappers,
                        &mut opaques,
                    )?,
                    is_receiver: p.is_receiver,
                    is_mut_receiver: p.is_mut_receiver,
                })
            })
            .collect::<anyhow::Result<Vec<_>>>()?;

        let return_type = convert_shape(
            entry.return_type.shape(),
            &mut structs,
            &mut enums,
            &mut wrappers,
            &mut opaques,
        )?;

        functions.push(SchemaFn {
            rust_name,
            kotlin_name,
            module_path,
            parent,
            params,
            return_type,
        });
    }

    Ok(Schema {
        crate_name,
        functions,
        structs: structs.into_values().collect(),
        enums: enums.into_values().collect(),
        wrappers: wrappers.into_values().collect(),
        opaques,
    })
}

fn convert_shape(
    shape: &'static facet::Shape,
    structs: &mut BTreeMap<(Option<String>, String), SchemaStruct>,
    enums: &mut BTreeMap<(Option<String>, String), SchemaEnum>,
    wrappers: &mut BTreeMap<String, SchemaWrapper>,
    opaques: &mut Vec<SchemaOpaque>,
) -> anyhow::Result<SchemaTypeRef> {
    if shape.proxy.is_some() {
        anyhow::bail!(
            "koffi: container-level #[facet(proxy = X)] on `{}` is not supported yet; \
             put #[facet(proxy = X)] on individual fields instead",
            shape.effective_name()
        );
    }

    match shape.def {
        facet::Def::Scalar => Ok(SchemaTypeRef::Scalar(scalar_kind_of(shape)?)),
        facet::Def::Option(def) => {
            let inner = convert_shape(def.t, structs, enums, wrappers, opaques)?;

            if matches!(inner, SchemaTypeRef::Option { .. }) {
                anyhow::bail!(
                    "koffi: unsupported Option<Option<T>>: double nullability has no \
                     Kotlin equivalent"
                );
            }

            let ident = format!("__koffi_option_{}", inner.unique_ident());
            if !wrappers.contains_key(&ident) {
                // The inner is converted first, so its structs/enums/wrappers
                // are registered before this wrapper's layout is computed.
                let members = vec![WrapperMember {
                    name: "some".to_string(),
                    ty: inner.clone(),
                }];
                let layout = compute_wrapper_layout(&members, structs, enums, wrappers)?;
                wrappers.insert(ident.clone(), SchemaWrapper {
                    kind: WrapperKind::Option,
                    unique_ident: ident,
                    members,
                    layout,
                });
            }

            Ok(SchemaTypeRef::Option {
                inner: Box::new(inner),
            })
        }
        facet::Def::Result(def) => {
            let ok = convert_shape(def.t, structs, enums, wrappers, opaques)?;
            let err = convert_shape(def.e, structs, enums, wrappers, opaques)?;

            let ident = format!(
                "__koffi_result_{}_{}",
                ok.unique_ident(),
                err.unique_ident()
            );
            if !wrappers.contains_key(&ident) {
                let members = vec![
                    WrapperMember {
                        name: "ok".to_string(),
                        ty: ok.clone(),
                    },
                    WrapperMember {
                        name: "err".to_string(),
                        ty: err.clone(),
                    },
                ];
                let layout = compute_wrapper_layout(&members, structs, enums, wrappers)?;
                wrappers.insert(ident.clone(), SchemaWrapper {
                    kind: WrapperKind::Result,
                    unique_ident: ident,
                    members,
                    layout,
                });
            }

            Ok(SchemaTypeRef::Result {
                ok: Box::new(ok),
                err: Box::new(err),
            })
        }
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
                                // A `#[facet(proxy = X)]` field is wired as X
                                // (the proxy shape) and converted through the
                                // user's TryFrom pair in the generated glue.
                                let field_shape = f.proxy_shape().unwrap_or_else(|| f.shape());
                                Ok(SchemaField {
                                    name: f.effective_name().to_string(),
                                    ty: convert_shape(
                                        field_shape,
                                        structs,
                                        enums,
                                        wrappers,
                                        opaques,
                                    )?,
                                    is_proxy: f.proxy_shape().is_some(),
                                    real_ty: f.proxy_shape().map(|_| {
                                        let real = f.shape();
                                        (
                                            real.effective_name().to_string(),
                                            real.module_path.map(|p| p.to_owned()),
                                        )
                                    }),
                                })
                            })
                            .collect::<anyhow::Result<Vec<_>>>()?;

                        // Layout is safe to compute here: every field's
                        // SchemaTypeRef is already in `structs` (inserted by
                        // the recursive convert_shape calls above), and a
                        // value cycle (X containing Y containing X) can't
                        // compile in Rust in the first place.
                        let layout = compute_struct_layout(&fields, structs, enums, wrappers)?;
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
                facet::Type::User(facet::UserType::Enum(e)) => {
                    let module_path = shape.module_path.map(|p| p.to_owned());
                    let name = shape.effective_name().to_string();
                    let key = (module_path.clone(), name.clone());

                    if !enums.contains_key(&key) {
                        let discriminant = scalar_kind_of_enum_repr(e.enum_repr)?;
                        let has_data = e
                            .variants
                            .iter()
                            .any(|v| v.data.kind != facet::StructKind::Unit);

                        // FFI-safety rides on the discriminant layout alone:
                        // `#[repr(C)]` (implicit-discriminant data enums) and
                        // `#[repr(i32)]`-style data enums (rustc forces
                        // explicit discriminants there) both get a fixed-size
                        // EnumRepr, while a default-repr enum reports
                        // `EnumRepr::Rust` and is rejected above as having no
                        // stable ABI.
                        let variants = e
                            .variants
                            .iter()
                            .map(|v| {
                                let variant = SchemaEnumVariant {
                                    name: v.name.to_string(),
                                    discriminant: v.discriminant.ok_or_else(|| {
                                        anyhow::anyhow!(
                                            "koffi: can't reflect the discriminant of `{name}`: {shape:?}"
                                        )
                                    })?,
                                    is_struct_variant: v.data.kind == facet::StructKind::Struct,
                                    placements: project_variant_fields(
                                        v,
                                        structs,
                                        enums,
                                        wrappers,
                                        opaques,
                                    )?,
                                };
                                Ok(variant)
                            })
                            .collect::<anyhow::Result<Vec<_>>>()?;

                        let layout =
                            compute_enum_layout(discriminant, &variants, structs, enums, wrappers)?;
                        enums.insert(key.clone(), SchemaEnum {
                            name: name.clone(),
                            module_path,
                            discriminant,
                            has_data,
                            variants,
                            layout,
                        });
                    }

                    let e = enums
                        .get(&key)
                        .ok_or_else(|| anyhow::anyhow!("koffi: enum `{name}` not registered"))?;
                    Ok(SchemaTypeRef::Enum {
                        name: e.name.clone(),
                        module_path: e.module_path.clone(),
                        discriminant: e.discriminant,
                        has_data: e.has_data,
                    })
                }
                facet::Type::User(facet::UserType::Opaque) => {
                    let module_path = shape.module_path.map(|p| p.to_owned());
                    let name = shape.effective_name().to_string();

                    // Opaque values cross the ABI as handles; anything else
                    // would need real layout knowledge the type refuses to
                    // expose. 8/8 is pointer size on every supported host.
                    let layout = shape.layout.sized_layout().map_err(|_| {
                        anyhow::anyhow!(
                            "koffi: unsized opaque types can't cross the FFI boundary (`{name}`)"
                        )
                    })?;
                    if layout.size() != 8 || layout.align() != 8 {
                        anyhow::bail!(
                            "koffi: opaque type `{name}` is not pointer-sized \
                             (size {} align {}); only pointer-sized opaque types can \
                             cross the FFI boundary as handles",
                            layout.size(),
                            layout.align()
                        );
                    }

                    if !opaques
                        .iter()
                        .any(|o| o.name == name && o.module_path == module_path)
                    {
                        opaques.push(SchemaOpaque {
                            name: name.clone(),
                            module_path: module_path.clone(),
                        });
                    }

                    Ok(SchemaTypeRef::Opaque { name, module_path })
                }
                _ => {
                    anyhow::bail!(
                        "koffi: only supports plain structs, fieldless #[repr(C)]/primitive-repr \
                         enums, opaque types, and scalars, got: {shape:?}"
                    )
                }
            }
        }
        _ => {
            anyhow::bail!(
                "koffi: only supports plain structs, fieldless #[repr(C)]/primitive-repr enums, \
             and scalars: {shape:?}"
            )
        }
    }
}

/// Projects one enum variant's payload fields. Unit variants yield an empty
/// list; tuple-variant fields get `field{n}` names (facet names them `"0"`,
/// which isn't a valid Kotlin/Rust identifier).
fn project_variant_fields(
    variant: &'static facet::Variant,
    structs: &mut BTreeMap<(Option<String>, String), SchemaStruct>,
    enums: &mut BTreeMap<(Option<String>, String), SchemaEnum>,
    wrappers: &mut BTreeMap<String, SchemaWrapper>,
    opaques: &mut Vec<SchemaOpaque>,
) -> anyhow::Result<Vec<FieldPlacement>> {
    variant
        .data
        .fields
        .iter()
        .map(|f| {
            let name = if variant.data.kind == facet::StructKind::TupleStruct {
                format!("field{}", f.effective_name())
            } else {
                f.effective_name().to_string()
            };
            let field_shape = f.proxy_shape().unwrap_or_else(|| f.shape());
            Ok(FieldPlacement {
                name,
                offset: f.offset as u64,
                ty: convert_shape(field_shape, structs, enums, wrappers, opaques)?,
                is_proxy: f.proxy_shape().is_some(),
                real_ty: f.proxy_shape().map(|_| {
                    let real = f.shape();
                    (
                        real.effective_name().to_string(),
                        real.module_path.map(|p| p.to_owned()),
                    )
                }),
            })
        })
        .collect()
}

fn scalar_kind_of_enum_repr(repr: facet::EnumRepr) -> anyhow::Result<ScalarKind> {
    match repr {
        facet::EnumRepr::U8 => Ok(ScalarKind::U8),
        facet::EnumRepr::U16 => Ok(ScalarKind::U16),
        facet::EnumRepr::U32 => Ok(ScalarKind::U32),
        facet::EnumRepr::U64 => Ok(ScalarKind::U64),
        facet::EnumRepr::I8 => Ok(ScalarKind::I8),
        facet::EnumRepr::I16 => Ok(ScalarKind::I16),
        facet::EnumRepr::I32 => Ok(ScalarKind::I32),
        facet::EnumRepr::I64 => Ok(ScalarKind::I64),
        facet::EnumRepr::Rust | facet::EnumRepr::RustNPO => {
            anyhow::bail!("koffi: no required #[repr(C)] or explicit primitive repr on enums")
        }
        facet::EnumRepr::USize | facet::EnumRepr::ISize => {
            anyhow::bail!(
                "koffi: unsupported enum discriminants with a platform-dependent layout \
             (#[repr(usize)]/#[repr(isize)])"
            )
        }
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
        other => anyhow::bail!("koffi: unrecognized scalar `{other}`"),
    }
}

#[cfg(test)]
mod tests {
    use facet::Facet;
    use koffi_core::{FnShapeRef, TypeShapeRef};

    use super::build_schema;

    /// Double nullability has no Kotlin type: `None` of `Option<Option<T>>`
    /// would need a third state. This must fail at codegen time, not at the
    /// Kotlin compiler.
    #[test]
    fn double_nullability_bails() {
        let entry = FnShapeRef {
            name: "bad",
            params: &[],
            return_type: TypeShapeRef::from_shape(<Option<Option<u32>> as Facet>::SHAPE),
            module_path: None,
            parent: None,
        };

        let err = build_schema("t".to_string(), &[entry])
            .expect_err("build_schema must reject Option<Option<T>>");
        assert!(err.to_string().contains("Option<Option<T>>"), "got: {err}");
    }
}
