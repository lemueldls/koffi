use heck::ToLowerCamelCase;

use crate::{ScalarKind, StructWire, Wire, WireField};

// M0: only scalars and all-scalar structs are handled; anything else is a
// panic (a compile error at the derive site, never a silent wrong answer).
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
