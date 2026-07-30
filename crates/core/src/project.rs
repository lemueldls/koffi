use heck::ToLowerCamelCase;

use crate::{ScalarKind, StructWire, Wire, WireField};

// Deliberately minimal for M0: only Type::Primitive and all-scalar Type::Struct
// are handled. Everything else is a const-eval panic, which surfaces as a
// compile error at the derive call site, not a silent wrong answer. This is
// the same "exhaustive match or reject" shape the full architecture calls for
// in section 5, just with a short match arm list because M0's scope is short.
pub fn project_shape(shape: &'static facet::Shape) -> Wire {
    match shape.def {
        facet::Def::Scalar => Wire::Scalar(scalar_kind_of(shape)),
        facet::Def::Undefined => {
            match shape.ty {
                facet::Type::User(facet::UserType::Struct(s)) => {
                    Wire::Struct(StructWire {
                        name: shape.effective_name().to_owned(),
                        module_path: shape.module_path.unwrap_or_default().to_owned(),
                        fields: project_fields(s.fields),
                    })
                }
                _ => panic!("koffi M0 only supports plain structs and scalars"),
            }
        }
        _ => panic!("koffi M0 only supports plain structs and scalars"),
    }
}

fn project_fields(fields: &'static [facet::Field]) -> Vec<WireField> {
    fields
        .iter()
        .map(|f| {
            WireField {
                name: f.effective_name().to_lower_camel_case(),
                wire: project_shape(f.shape.get()),
            }
        })
        .collect()
}

fn scalar_kind_of(shape: &'static facet::Shape) -> ScalarKind {
    match shape.effective_name() {
        "bool" => ScalarKind::Bool,
        "u8" => ScalarKind::U8,
        "u16" => ScalarKind::U16,
        "u32" => ScalarKind::U32,
        "u64" => ScalarKind::U64,
        "i8" => ScalarKind::I8,
        "i16" => ScalarKind::I16,
        "i32" => ScalarKind::I32,
        "i64" => ScalarKind::I64,
        "f32" => ScalarKind::F32,
        "f64" => ScalarKind::F64,
        _ => panic!("koffi M0 only supports plain structs and scalars"),
    }
}
