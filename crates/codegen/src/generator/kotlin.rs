use heck::ToUpperCamelCase;

use crate::{
    layout::FieldPlacement,
    schema::{ScalarKind, Schema, SchemaFn, SchemaParam, SchemaStruct, SchemaTypeRef},
};

impl Schema {
    #[must_use]
    pub fn ffi_object_name(&self) -> String {
        format!("{}Ffi", self.crate_ident_pascal())
    }

    #[must_use]
    pub fn crate_ident_pascal(&self) -> String {
        self.crate_name.to_upper_camel_case()
    }

    #[must_use]
    pub fn kotlin_package(&self) -> String {
        self.functions
            .first()
            .and_then(|f| f.module_path.as_deref())
            .unwrap_or("")
            .replace("::", ".")
    }

    #[must_use]
    pub fn loader_object_name(&self) -> String {
        format!("{}Loader", self.crate_ident_pascal())
    }

    #[must_use]
    pub fn glue_crate_ident(&self) -> String {
        format!("{}_glue", self.crate_ident())
    }

    /// Every fn whose `parent` is `s`, in Rust declaration order. Drives
    /// every per-struct Kotlin member in common.kt.j2. The flat
    /// `functions` list stays flat elsewhere: it's what the low-level FFI
    /// object is built from, where every fn needs one ABI symbol regardless
    /// of which struct, if any, it belongs to.
    #[must_use]
    pub fn functions_of<'a>(&'a self, s: &'a SchemaStruct) -> Vec<&'a SchemaFn> {
        self.functions
            .iter()
            .filter(|f| f.parent.as_ref().is_some_and(|p| p.same_struct(s)))
            .collect()
    }

    /// Does `s` have at least one associated fn that isn't an instance
    /// method (a constructor or an ordinary companion fn)? Drives whether
    /// common.kt.j2 opens a `companion object { .. }` block at all; an
    /// empty one isn't valid Kotlin to emit unconditionally.
    #[must_use]
    pub fn has_companion_functions(&self, s: &SchemaStruct) -> bool {
        self.functions_of(s).iter().any(|f| !f.has_receiver())
    }
}

impl SchemaTypeRef {
    #[must_use]
    pub fn kotlin_type(&self) -> String {
        match self {
            SchemaTypeRef::Scalar(k) => k.kotlin_type().to_string(),
            SchemaTypeRef::Struct { name, .. } => name.clone(),
        }
    }

    #[must_use]
    pub fn kotlin_ffm_value_layout(&self) -> String {
        match self {
            SchemaTypeRef::Scalar(k) => k.kotlin_ffm_value_layout().to_string(),
            SchemaTypeRef::Struct { .. } => format!("{}Layout", self.unique_ident()),
        }
    }

    #[must_use]
    pub const fn to_ffm_suffix(&self) -> &'static str {
        match self {
            SchemaTypeRef::Scalar(k) => k.to_ffm_suffix(),
            SchemaTypeRef::Struct { .. } => "",
        }
    }

    #[must_use]
    pub const fn from_ffm_suffix(&self) -> &'static str {
        match self {
            SchemaTypeRef::Scalar(k) => k.from_ffm_suffix(),
            SchemaTypeRef::Struct { .. } => "",
        }
    }

    #[must_use]
    pub fn struct_field_placements<'a>(
        &self,
        structs: &'a [SchemaStruct],
    ) -> Option<&'a Vec<FieldPlacement>> {
        match self {
            SchemaTypeRef::Scalar(_) => None,
            SchemaTypeRef::Struct { name, module_path } => {
                structs
                    .iter()
                    .find(|s| &s.name == name && &s.module_path == module_path)
                    .map(|s| &s.layout.placements)
            }
        }
    }
}

impl SchemaParam {
    #[must_use]
    pub fn struct_field_placements<'a>(
        &self,
        structs: &'a [SchemaStruct],
    ) -> Option<&'a Vec<FieldPlacement>> {
        self.ty.struct_field_placements(structs)
    }
}

impl ScalarKind {
    #[must_use]
    pub const fn kotlin_type(&self) -> &'static str {
        match self {
            ScalarKind::Bool => "Boolean",
            ScalarKind::U8 => "UByte",
            ScalarKind::U16 => "UShort",
            ScalarKind::U32 => "UInt",
            ScalarKind::U64 => "ULong",
            ScalarKind::I8 => "Byte",
            ScalarKind::I16 => "Short",
            ScalarKind::I32 => "Int",
            ScalarKind::I64 => "Long",
            ScalarKind::F32 => "Float",
            ScalarKind::F64 => "Double",
        }
    }

    #[must_use]
    pub const fn kotlin_ffm_value_layout(&self) -> &'static str {
        match self {
            ScalarKind::Bool => "ValueLayout.JAVA_BOOLEAN",
            ScalarKind::U8 | ScalarKind::I8 => "ValueLayout.JAVA_BYTE",
            ScalarKind::U16 | ScalarKind::I16 => "ValueLayout.JAVA_SHORT",
            ScalarKind::U32 | ScalarKind::I32 => "ValueLayout.JAVA_INT",
            ScalarKind::U64 | ScalarKind::I64 => "ValueLayout.JAVA_LONG",
            ScalarKind::F32 => "ValueLayout.JAVA_FLOAT",
            ScalarKind::F64 => "ValueLayout.JAVA_DOUBLE",
        }
    }

    #[must_use]
    pub const fn jvm_boxed_type(&self) -> &'static str {
        match self {
            ScalarKind::Bool => "Boolean",
            ScalarKind::U8 | ScalarKind::I8 => "Byte",
            ScalarKind::U16 | ScalarKind::I16 => "Short",
            ScalarKind::U32 | ScalarKind::I32 => "Int",
            ScalarKind::U64 | ScalarKind::I64 => "Long",
            ScalarKind::F32 => "Float",
            ScalarKind::F64 => "Double",
        }
    }

    #[must_use]
    pub const fn to_ffm_suffix(&self) -> &'static str {
        match self {
            ScalarKind::U8 => ".toByte()",
            ScalarKind::U16 => ".toShort()",
            ScalarKind::U32 => ".toInt()",
            ScalarKind::U64 => ".toLong()",
            _ => "",
        }
    }

    #[must_use]
    pub const fn from_ffm_suffix(&self) -> &'static str {
        match self {
            ScalarKind::U8 => ".toUByte()",
            ScalarKind::U16 => ".toUShort()",
            ScalarKind::U32 => ".toUInt()",
            ScalarKind::U64 => ".toULong()",
            _ => "",
        }
    }
}

impl SchemaStruct {
    /// Does calling this struct's own primary (field) constructor take the
    /// same argument list, in order and type, as `params`? `Type(..)` call
    /// syntax already means the primary constructor; a companion `invoke`
    /// can overload it only when the signatures differ. When they match,
    /// common.kt.j2 skips `invoke` and emits only the always-safe named
    /// companion function (`Type.new(..)`).
    #[must_use]
    pub fn matches_primary_constructor(&self, params: &[SchemaParam]) -> bool {
        self.fields.len() == params.len()
            && self
                .fields
                .iter()
                .zip(params)
                .all(|(field, param)| field.ty == param.ty)
    }

    #[must_use]
    pub fn kotlin_ffm_value_layout(&self) -> String {
        format!("{}Layout", self.unique_ident())
    }
}
