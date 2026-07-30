use std::collections::BTreeMap;

use crate::schema::{ScalarKind, SchemaField, SchemaStruct, SchemaTypeRef};

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
    Value {
        kotlin_layout: &'static str,
        name: String,
    },
    Padding {
        bytes: u64,
    },
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
) -> anyhow::Result<Layout> {
    match ty {
        SchemaTypeRef::Scalar(k) => Ok(scalar_layout(*k)),
        SchemaTypeRef::Struct { name, module_path } => {
            let s = structs
                .get(&(module_path.clone(), name.clone()))
                .ok_or_else(|| anyhow::anyhow!("layout_of: `{name}` not yet in the struct map"))?;
            Ok(s.layout.total)
        }
    }
}

pub fn compute_struct_layout(
    fields: &[SchemaField],
    structs: &BTreeMap<(Option<String>, String), SchemaStruct>,
) -> anyhow::Result<StructLayoutInfo> {
    let mut entries = Vec::new();
    let mut placements = Vec::new();
    let mut offset: u64 = 0;
    let mut max_align: u64 = 1;

    for field in fields {
        let fl = layout_of(&field.ty, structs)?;
        let aligned = round_up(offset, fl.align);
        if aligned > offset {
            entries.push(LayoutEntry::Padding {
                bytes: aligned - offset,
            });
        }
        entries.push(LayoutEntry::Value {
            kotlin_layout: kotlin_value_layout_name(&field.ty)?,
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

fn kotlin_value_layout_name(ty: &SchemaTypeRef) -> anyhow::Result<&'static str> {
    match ty {
        SchemaTypeRef::Scalar(ScalarKind::Bool) => Ok("ValueLayout.JAVA_BOOLEAN"),
        SchemaTypeRef::Scalar(ScalarKind::U8 | ScalarKind::I8) => Ok("ValueLayout.JAVA_BYTE"),
        SchemaTypeRef::Scalar(ScalarKind::U16 | ScalarKind::I16) => Ok("ValueLayout.JAVA_SHORT"),
        SchemaTypeRef::Scalar(ScalarKind::U32 | ScalarKind::I32) => Ok("ValueLayout.JAVA_INT"),
        SchemaTypeRef::Scalar(ScalarKind::U64 | ScalarKind::I64) => Ok("ValueLayout.JAVA_LONG"),
        SchemaTypeRef::Scalar(ScalarKind::F32) => Ok("ValueLayout.JAVA_FLOAT"),
        SchemaTypeRef::Scalar(ScalarKind::F64) => Ok("ValueLayout.JAVA_DOUBLE"),
        // A nested struct field needs a *dynamic* name (the other struct's
        // own layout constant, declared earlier in the same generated file),
        // not a &'static str, and building that name here would be a third
        // place computing what generator/rust.rs's SchemaStruct::unique_ident
        // already computes, the exact class of bug the c_abi_symbol fix was
        // about. Left as an explicit, named gap rather than a silently wrong
        // placeholder: M0's worked example doesn't have a struct-typed field,
        // so it isn't blocking, but this needs a real answer (most likely:
        // return an owned String here and have this function take the same
        // naming closure/fn generator/rust.rs and generator/kotlin.rs both
        // already call, rather than duplicating the formula) before a struct
        // with a struct-typed field can work.
        SchemaTypeRef::Struct { name, .. } => {
            anyhow::bail!(
                "koffi M0 doesn't yet support a struct-typed field (`{name}`); \
             needs a shared naming function, not a third copy of the formula"
            )
        }
    }
}
