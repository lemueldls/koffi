use std::collections::BTreeMap;

use crate::schema::{
    ScalarKind, SchemaEnum, SchemaEnumVariant, SchemaField, SchemaStruct, SchemaTypeRef,
    SchemaWrapper, WrapperMember,
};

#[derive(Debug, Clone, Copy)]
pub struct Layout {
    pub size: u64,
    pub align: u64,
}

#[derive(Debug, Clone)]
pub struct FieldPlacement {
    pub name: String,
    pub ty: SchemaTypeRef,
    pub offset: u64,
}

#[derive(Debug, Clone)]
pub enum LayoutEntry {
    Value { kotlin_layout: String, name: String },
    Padding { bytes: u64 },
}

#[derive(Debug, Clone)]
pub struct StructLayoutInfo {
    pub entries: Vec<LayoutEntry>,
    pub placements: Vec<FieldPlacement>,
    pub total: Layout,
}

const fn round_up(n: u64, align: u64) -> u64 {
    n.div_ceil(align) * align
}

#[must_use]
pub const fn scalar_layout(kind: ScalarKind) -> Layout {
    match kind {
        ScalarKind::Bool | ScalarKind::U8 | ScalarKind::I8 => Layout { size: 1, align: 1 },
        ScalarKind::U16 | ScalarKind::I16 => Layout { size: 2, align: 2 },
        ScalarKind::U32 | ScalarKind::I32 | ScalarKind::F32 => Layout { size: 4, align: 4 },
        ScalarKind::U64 | ScalarKind::I64 | ScalarKind::F64 => Layout { size: 8, align: 8 },
    }
}

pub fn layout_of(
    ty: &SchemaTypeRef,
    structs: &BTreeMap<(Option<String>, String), SchemaStruct>,
    enums: &BTreeMap<(Option<String>, String), SchemaEnum>,
    wrappers: &BTreeMap<String, SchemaWrapper>,
) -> anyhow::Result<Layout> {
    match ty {
        SchemaTypeRef::Scalar(k) => Ok(scalar_layout(*k)),
        SchemaTypeRef::Struct { name, module_path } => {
            let s = structs
                .get(&(module_path.clone(), name.clone()))
                .ok_or_else(|| anyhow::anyhow!("layout_of: `{name}` not yet in the struct map"))?;
            Ok(s.layout.total)
        }
        SchemaTypeRef::Enum {
            name, module_path, ..
        } => {
            let e = enums
                .get(&(module_path.clone(), name.clone()))
                .ok_or_else(|| anyhow::anyhow!("layout_of: `{name}` not yet in the enum map"))?;
            Ok(e.layout.total)
        }
        SchemaTypeRef::Option { .. } | SchemaTypeRef::Result { .. } => {
            let w = wrappers.get(&ty.unique_ident()).ok_or_else(|| {
                anyhow::anyhow!(
                    "layout_of: `{}` not yet in the wrapper map",
                    ty.unique_ident()
                )
            })?;
            Ok(w.layout.total)
        }
    }
}

pub fn compute_struct_layout(
    fields: &[SchemaField],
    structs: &BTreeMap<(Option<String>, String), SchemaStruct>,
    enums: &BTreeMap<(Option<String>, String), SchemaEnum>,
    wrappers: &BTreeMap<String, SchemaWrapper>,
) -> anyhow::Result<StructLayoutInfo> {
    let mut entries = Vec::new();
    let mut placements = Vec::new();
    let mut offset: u64 = 0;
    let mut max_align: u64 = 1;

    for field in fields {
        let fl = layout_of(&field.ty, structs, enums, wrappers)?;
        let aligned = round_up(offset, fl.align);
        if aligned > offset {
            entries.push(LayoutEntry::Padding {
                bytes: aligned - offset,
            });
        }
        entries.push(LayoutEntry::Value {
            kotlin_layout: kotlin_value_layout_name(&field.ty, true, structs, enums, wrappers)?,
            name: field.name.clone(),
        });
        placements.push(FieldPlacement {
            name: field.name.clone(),
            ty: field.ty.clone(),
            offset: aligned,
        });
        offset = aligned + fl.size;
        max_align = max_align.max(fl.align);
    }

    let total_size = round_up(offset, max_align);
    if total_size > offset {
        entries.push(LayoutEntry::Padding {
            bytes: total_size - offset,
        });
    }

    Ok(StructLayoutInfo {
        entries,
        placements,
        total: Layout {
            size: total_size,
            align: max_align,
        },
    })
}

/// Lays out a data-carrying enum as C would: the discriminant at
/// offset 0, then the union of every variant's payload (facet's
/// offsets already include the discriminant).
///
/// The per-variant `placements` keep their own absolute offsets and field
/// types. The `entries` region only needs to cover the union faithfully, so at
/// each offset shared by several variants' fields (they overlap by
/// construction) the widest one wins.
pub fn compute_enum_layout(
    discriminant: ScalarKind,
    variants: &[SchemaEnumVariant],
    structs: &BTreeMap<(Option<String>, String), SchemaStruct>,
    enums: &BTreeMap<(Option<String>, String), SchemaEnum>,
    wrappers: &BTreeMap<String, SchemaWrapper>,
) -> anyhow::Result<StructLayoutInfo> {
    if !matches!(
        discriminant,
        ScalarKind::U8
            | ScalarKind::U16
            | ScalarKind::U32
            | ScalarKind::U64
            | ScalarKind::I8
            | ScalarKind::I16
            | ScalarKind::I32
            | ScalarKind::I64
    ) {
        anyhow::bail!("koffi: enum discriminant must be an integer scalar");
    }

    let d_layout = scalar_layout(discriminant);
    let mut max_align = d_layout.align;
    let mut max_end = d_layout.size;

    // Per unique offset: the widest field layout (and its Kotlin layout
    // name), for the union region. Overlapping payload fields of different
    // variants only need the region sized; actual reads/writes always use
    // per-variant placement types.
    let mut by_offset: BTreeMap<u64, (Layout, String)> = BTreeMap::new();

    for variant in variants {
        for placement in &variant.placements {
            let fl = layout_of(&placement.ty, structs, enums, wrappers)?;
            let layout_name =
                kotlin_value_layout_name(&placement.ty, false, structs, enums, wrappers)?;
            let end = placement.offset + fl.size;
            max_end = max_end.max(end);
            max_align = max_align.max(fl.align);

            by_offset
                .entry(placement.offset)
                .and_modify(|(l, n)| {
                    if fl.size > l.size {
                        *l = fl;
                        n.clone_from(&layout_name);
                    }
                })
                .or_insert((fl, layout_name));
        }
    }

    let total_size = round_up(max_end, max_align);
    let mut entries = Vec::new();
    let mut offset: u64 = d_layout.size;

    entries.push(LayoutEntry::Value {
        kotlin_layout: kotlin_value_layout_name(
            &SchemaTypeRef::Scalar(discriminant),
            false,
            structs,
            enums,
            wrappers,
        )?,
        name: "discriminant".to_string(),
    });

    for (off, (l, name)) in by_offset {
        if off > offset {
            entries.push(LayoutEntry::Padding {
                bytes: off - offset,
            });
        }
        entries.push(LayoutEntry::Value {
            kotlin_layout: name,
            name: String::new(),
        });
        offset = off + l.size;
    }
    if total_size > offset {
        entries.push(LayoutEntry::Padding {
            bytes: total_size - offset,
        });
    }

    Ok(StructLayoutInfo {
        entries,
        placements: variants.iter().flat_map(|v| v.placements.clone()).collect(),
        total: Layout {
            size: total_size,
            align: max_align,
        },
    })
}

fn kotlin_value_layout_name(
    ty: &SchemaTypeRef,
    allow_structs: bool,
    _structs: &BTreeMap<(Option<String>, String), SchemaStruct>,
    enums: &BTreeMap<(Option<String>, String), SchemaEnum>,
    wrappers: &BTreeMap<String, SchemaWrapper>,
) -> anyhow::Result<String> {
    match ty {
        SchemaTypeRef::Scalar(k) => Ok(k.kotlin_ffm_value_layout().to_string()),
        SchemaTypeRef::Enum {
            name,
            module_path,
            discriminant,
            has_data,
        } => {
            if *has_data {
                let e = enums
                    .get(&(module_path.clone(), name.clone()))
                    .ok_or_else(|| anyhow::anyhow!("`{name}` not yet in the enum map"))?;
                Ok(e.kotlin_ffm_value_layout())
            } else {
                Ok(discriminant.kotlin_ffm_value_layout().to_string())
            }
        }
        // A nested struct field is marshalled via per-struct `toFfm`/
        // `fromFfm` helpers (generated in ffm.kt.j2). The layout name is
        // the struct's own FFM layout constant, which must be declared
        // before the containing struct's layout, enforced by the
        // topological sort in `Schema::structs_in_layout_order`.
        SchemaTypeRef::Struct { name, .. } => {
            if allow_structs {
                Ok(ty.kotlin_ffm_value_layout())
            } else {
                anyhow::bail!(
                    "koffi doesn't yet support a struct-typed field in an enum variant (`{name}`)"
                )
            }
        }
        // A wrapper's layout is always a leaf reference to its own FFM
        // layout constant, struct-typed or not: the struct inside is one
        // nesting level removed from the enum variant, and the topo sort
        // in `Schema::types_in_layout_order` handles the declaration order.
        SchemaTypeRef::Option { .. } | SchemaTypeRef::Result { .. } => {
            let w = wrappers.get(&ty.unique_ident()).ok_or_else(|| {
                anyhow::anyhow!("`{}` not yet in the wrapper map", ty.unique_ident())
            })?;
            Ok(w.kotlin_ffm_value_layout())
        }
    }
}

/// Lays out an `Option`/`Result` wrapper as C would: the `u8`
/// discriminant at offset 0, then the payload union over its members.
///
/// Unlike `compute_enum_layout`, the union starts *all* members at one
/// shared offset (`round_up(1, max_align)`): `Result<u8, u32>` puts both
/// `ok` and `err` at offset 4, never `ok` at 1.
///
/// `align(Option<T>) == align(T)`, so a wrapper's alignment matches the
/// real type's, which is what keeps variant-payload field offsets (from
/// facet) lining up with the wire struct. Sizes can diverge for niche
/// inners (`Option<bool>` is 1 real byte, 2 on the wire) which only
/// matters when a later field in the same variant would land in the
/// gap; that's a documented limitation, not something this layout fixes.
pub fn compute_wrapper_layout(
    members: &[WrapperMember],
    structs: &BTreeMap<(Option<String>, String), SchemaStruct>,
    enums: &BTreeMap<(Option<String>, String), SchemaEnum>,
    wrappers: &BTreeMap<String, SchemaWrapper>,
) -> anyhow::Result<StructLayoutInfo> {
    let d_layout = scalar_layout(ScalarKind::U8);
    let mut max_align = d_layout.align;

    let mut member_layouts: Vec<(Layout, String)> = Vec::with_capacity(members.len());
    for member in members {
        let fl = layout_of(&member.ty, structs, enums, wrappers)?;
        let layout_name = kotlin_value_layout_name(&member.ty, true, structs, enums, wrappers)?;
        max_align = max_align.max(fl.align);
        member_layouts.push((fl, layout_name));
    }

    // All members sit at the same union offset. The widest one sizes the
    // region; placement offsets are per-member (`placements`), so writes
    // always use the member's own layout.
    let union_offset = round_up(d_layout.size, max_align);
    let max_size = member_layouts
        .iter()
        .map(|(l, _)| l.size)
        .max()
        .unwrap_or(0);
    let total_size = round_up(union_offset + max_size, max_align);

    let mut entries = Vec::new();
    entries.push(LayoutEntry::Value {
        kotlin_layout: ScalarKind::U8.kotlin_ffm_value_layout().to_string(),
        name: "discriminant".to_string(),
    });
    if union_offset > d_layout.size {
        entries.push(LayoutEntry::Padding {
            bytes: union_offset - d_layout.size,
        });
    }
    if let Some((_, widest_name)) = member_layouts.iter().max_by_key(|(l, _)| l.size) {
        entries.push(LayoutEntry::Value {
            kotlin_layout: widest_name.clone(),
            name: String::new(),
        });
    }
    if total_size > union_offset + max_size {
        entries.push(LayoutEntry::Padding {
            bytes: total_size - (union_offset + max_size),
        });
    }

    let placements = members
        .iter()
        .map(|m| {
            FieldPlacement {
                name: m.name.clone(),
                ty: m.ty.clone(),
                offset: union_offset,
            }
        })
        .collect();

    Ok(StructLayoutInfo {
        entries,
        placements,
        total: Layout {
            size: total_size,
            align: max_align,
        },
    })
}
