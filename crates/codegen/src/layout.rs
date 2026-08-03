use std::collections::BTreeMap;

use crate::schema::{
    ScalarKind, SchemaEnum, SchemaEnumVariant, SchemaField, SchemaStruct, SchemaTypeRef,
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
    }
}

pub fn compute_struct_layout(
    fields: &[SchemaField],
    structs: &BTreeMap<(Option<String>, String), SchemaStruct>,
    enums: &BTreeMap<(Option<String>, String), SchemaEnum>,
) -> anyhow::Result<StructLayoutInfo> {
    let mut entries = Vec::new();
    let mut placements = Vec::new();
    let mut offset: u64 = 0;
    let mut max_align: u64 = 1;

    for field in fields {
        let fl = layout_of(&field.ty, structs, enums)?;
        let aligned = round_up(offset, fl.align);
        if aligned > offset {
            entries.push(LayoutEntry::Padding {
                bytes: aligned - offset,
            });
        }
        entries.push(LayoutEntry::Value {
            kotlin_layout: kotlin_value_layout_name(&field.ty, true, structs, enums)?,
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
        anyhow::bail!("koffi M0: enum discriminant must be an integer scalar");
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
            let fl = layout_of(&placement.ty, structs, enums)?;
            let layout_name = kotlin_value_layout_name(&placement.ty, false, structs, enums)?;
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
                    "koffi M0 doesn't yet support a struct-typed field in an enum variant (`{name}`)"
                )
            }
        }
    }
}
