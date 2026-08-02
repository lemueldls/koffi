use heck::{ToLowerCamelCase, ToSnakeCase, ToUpperCamelCase};

use crate::{
    layout::FieldPlacement,
    schema::{
        ScalarKind, Schema, SchemaEnum, SchemaFn, SchemaParam, SchemaStruct, SchemaTypeRef,
    },
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

    /// Every fn whose `parent` is `e` (methods, constructors and companion
    /// fns of an `impl e { .. }` block), in Rust declaration order. Drives
    /// the members of the generated Kotlin enum class in common.kt.j2.
    #[must_use]
    pub fn functions_of_enum<'a>(&'a self, e: &'a SchemaEnum) -> Vec<&'a SchemaFn> {
        self.functions
            .iter()
            .filter(|f| f.parent.as_ref().is_some_and(|p| p.same_enum(e)))
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

    /// Same as [`Self::has_companion_functions`], for enum impls.
    #[must_use]
    pub fn has_companion_functions_enum(&self, e: &SchemaEnum) -> bool {
        self.functions_of_enum(e).iter().any(|f| !f.has_receiver())
    }

    /// The reflected enum a data-carrying `SchemaTypeRef::Enum` points at,
    /// `None` for everything else (scalars, structs, fieldless enums).
    #[must_use]
    pub fn enum_of<'a>(&'a self, ty: &SchemaTypeRef) -> Option<&'a SchemaEnum> {
        match ty {
            SchemaTypeRef::Enum { name, module_path, .. } => self
                .enums
                .iter()
                .find(|e| &e.name == name && &e.module_path == module_path),
            _ => None,
        }
    }
}

impl SchemaTypeRef {
    #[must_use]
    pub fn kotlin_type(&self) -> String {
        match self {
            SchemaTypeRef::Scalar(k) => k.kotlin_type().to_string(),
            SchemaTypeRef::Struct { name, .. } | SchemaTypeRef::Enum { name, .. } => name.clone(),
        }
    }

    #[must_use]
    pub fn kotlin_ffm_value_layout(&self) -> String {
        match self {
            SchemaTypeRef::Scalar(k) => k.kotlin_ffm_value_layout().to_string(),
            SchemaTypeRef::Struct { .. } => format!("{}Layout", self.unique_ident()),
            SchemaTypeRef::Enum { discriminant, has_data, .. } => {
                if *has_data {
                    format!("{}Layout", self.unique_ident())
                } else {
                    discriminant.kotlin_ffm_value_layout().to_string()
                }
            }
        }
    }

    /// Identifier of the top-level marshalling helper for a data-carrying
    /// enum (`statusToFfm`), or empty for everything else. Data enums
    /// cross the FFI boundary as segments, so the parameter-prep and
    /// return spots call these helpers instead of using suffixes.
    #[must_use]
    pub fn to_ffm_ident(&self) -> String {
        match self {
            SchemaTypeRef::Enum { name, has_data: true, .. } => {
                format!("{}ToFfm", name.to_snake_case())
            }
            _ => String::new(),
        }
    }

    #[must_use]
    pub fn from_ffm_ident(&self) -> String {
        match self {
            SchemaTypeRef::Enum { name, has_data: true, .. } => {
                format!("{}FromFfm", name.to_snake_case())
            }
            _ => String::new(),
        }
    }

    /// Suffix turning a Kotlin value into the raw FFI scalar, for the
    /// parameter-arg spot in ffm.kt.j2. A fieldless enum contributes its
    /// discriminant value (`status.discriminant.toUInt()`-shaped); a
    /// data-carrying enum never uses this (it goes through a segment and
    /// `toFfm`), and neither does a struct.
    #[must_use]
    pub fn to_ffm_suffix(&self) -> String {
        match self {
            SchemaTypeRef::Scalar(k) => k.to_ffm_suffix().to_string(),
            SchemaTypeRef::Enum { discriminant, has_data: false, .. } => {
                format!(".discriminant{}", discriminant.to_ffm_suffix())
            }
            SchemaTypeRef::Struct { .. } | SchemaTypeRef::Enum { has_data: true, .. } => {
                String::new()
            }
        }
    }

    /// Suffix turning the boxed FFI scalar back into a Kotlin value, for
    /// the field-read spot in ffm.kt.j2. Fieldless enums roundtrip through
    /// `fromDiscriminant`; data-carrying enums use `fromFfm` on a segment
    /// and never hit this.
    #[must_use]
    pub fn from_ffm_suffix(&self) -> String {
        match self {
            SchemaTypeRef::Scalar(k) => k.from_ffm_suffix().to_string(),
            SchemaTypeRef::Enum { name, discriminant, has_data: false, .. } => {
                let suffix = discriminant.from_ffm_suffix();
                if suffix.is_empty() {
                    format!(".let {{ {name}.fromDiscriminant(it) }}")
                } else {
                    format!(".let {{ {name}.fromDiscriminant(it{suffix}) }}")
                }
            }
            SchemaTypeRef::Struct { .. } | SchemaTypeRef::Enum { has_data: true, .. } => {
                String::new()
            }
        }
    }

    #[must_use]
    pub fn struct_field_placements<'a>(
        &self,
        structs: &'a [SchemaStruct],
    ) -> Option<&'a Vec<FieldPlacement>> {
        match self {
            SchemaTypeRef::Scalar(_) | SchemaTypeRef::Enum { .. } => None,
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

impl SchemaFn {
    /// FFI-object member name: the `expect`/`actual fun`s generated on the
    /// shared object. Free fns keep their plain lowerCamelCase name, but
    /// parent-having fns are prefixed with the parent type (module-aware)
    /// so `Payload::new` and `Status::new` — or the same type name in
    /// another module — can't collide on the one object. The public API
    /// (companion fns, instance methods) keeps using `kotlin_name`.
    #[must_use]
    pub fn ffi_member_name(&self) -> String {
        let Some(parent) = &self.parent else {
            return self.kotlin_name.clone();
        };
        let (name, module_path) = match parent {
            SchemaTypeRef::Struct { name, module_path }
            | SchemaTypeRef::Enum { name, module_path, .. } => (name, module_path),
            SchemaTypeRef::Scalar(_) => {
                unreachable!("impl-block parents are always structs or enums")
            }
        };
        let mod_infix = module_path.as_deref().unwrap_or("").replace("::", "_");
        let prefix = format!("{mod_infix}_{name}").to_lower_camel_case();
        format!("{prefix}{}", self.kotlin_name.to_upper_camel_case())
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

    /// A Kotlin literal for an enum discriminant of this kind. Unsigned
    /// kinds wrap negative values to their bit pattern (`#[repr(C)]` enum
    /// `Error = -1` is stored as `0xFFFF_FFFF`, i.e. `4294967295u`).
    // The narrowing casts are the point: wrapping to the wire width is
    // exactly the bit-pattern semantics of the enum's repr.
    #[must_use]
    #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
    pub fn kotlin_discriminant_literal(&self, value: &i64) -> String {
        let value = *value;
        match self {
            ScalarKind::U8 => format!("{}u", value as u8),
            ScalarKind::U16 => format!("{}u", value as u16),
            ScalarKind::U32 => format!("{}u", value as u32),
            ScalarKind::U64 => format!("{}uL", value as u64),
            _ => value.to_string(),
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

impl SchemaEnum {
    /// The generated FFM struct layout constant for a data-carrying enum.
    /// Fieldless enums cross the ABI as their plain discriminant scalar and
    /// never get a layout constant.
    #[must_use]
    pub fn kotlin_ffm_value_layout(&self) -> String {
        format!("{}Layout", self.unique_ident())
    }

    /// Top-level marshalling helpers written to ffm.kt.j2 (`statusToFfm`,
    /// `statusFromFfm`). Member extensions are not callable via the class
    /// name from inside `actual object`, so they live as file-level private
    /// functions instead.
    #[must_use]
    pub fn to_ffm_ident(&self) -> String {
        format!("{}ToFfm", self.name.to_snake_case())
    }

    #[must_use]
    pub fn from_ffm_ident(&self) -> String {
        format!("{}FromFfm", self.name.to_snake_case())
    }
}
