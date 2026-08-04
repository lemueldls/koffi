use heck::{ToLowerCamelCase, ToSnakeCase, ToUpperCamelCase};

use crate::{
    layout::FieldPlacement,
    schema::{
        ScalarKind, Schema, SchemaEnum, SchemaField, SchemaFn, SchemaParam, SchemaStruct,
        SchemaTypeRef,
    },
};

impl Schema {
    #[must_use]
    pub fn ffi_object_name(&self) -> String {
        format!("{}Ffi", self.crate_ident_pascal())
    }

    /// Wire size in bytes of a memory-backed type: the total layout size
    /// of the struct or data-carrying enum, padding included. Drives the
    /// out-buffer allocation in jni.kt.j2.
    #[must_use]
    pub fn layout_size(&self, ty: &SchemaTypeRef) -> u64 {
        match ty {
            SchemaTypeRef::Struct { name, module_path } => {
                self.structs
                    .iter()
                    .find(|s| s.name == *name && s.module_path == *module_path)
                    .map_or(0, |s| s.layout.total.size)
            }
            SchemaTypeRef::Enum {
                name, module_path, ..
            } => {
                self.enums
                    .iter()
                    .find(|e| e.name == *name && e.module_path == *module_path)
                    .map_or(0, |e| e.layout.total.size)
            }
            SchemaTypeRef::Scalar(_) => 0,
        }
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
    /// every per-struct Kotlin member in common.kt.j2. Elsewhere the flat
    /// `functions` list remains the single source for the low-level FFI
    /// object: every fn needs one ABI symbol regardless of which struct,
    /// if any, it belongs to.
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
    /// common.kt.j2 opens a `companion object { .. }` block at all;
    /// emitting an empty block isn't valid Kotlin.
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
            SchemaTypeRef::Enum {
                name, module_path, ..
            } => {
                self.enums
                    .iter()
                    .find(|e| &e.name == name && &e.module_path == module_path)
            }
            _ => None,
        }
    }

    /// Structs ordered so that any struct used as a field type of another
    /// struct appears before it. Kotlin `val` layout properties are
    /// initialized in declaration order, so a forward reference to a
    /// not-yet-initialized `val` is a compile error. Function declarations
    /// (`toFfm`/`fromFfm`) don't have this constraint, but the layout
    /// `val`s do. This sort feeds the layout declaration order in
    /// `ffm.kt.j2`.
    ///
    /// Cycles are impossible (Rust rejects value-recursive types), so a
    /// simple DFS suffices.
    #[must_use]
    pub fn structs_in_layout_order(&self) -> Vec<&SchemaStruct> {
        fn key(s: &SchemaStruct) -> (Option<String>, String) {
            (s.module_path.clone(), s.name.clone())
        }

        fn visit<'a>(
            s: &'a SchemaStruct,
            all: &'a [SchemaStruct],
            visited: &mut std::collections::HashSet<(Option<String>, String)>,
            result: &mut Vec<&'a SchemaStruct>,
        ) {
            let k = key(s);
            if !visited.insert(k) {
                return;
            }
            for field in &s.fields {
                if let SchemaTypeRef::Struct { name, module_path } = &field.ty
                    && let Some(dep) = all
                        .iter()
                        .find(|s2| &s2.name == name && &s2.module_path == module_path)
                {
                    visit(dep, all, visited, result);
                }
            }
            result.push(s);
        }

        let mut visited = std::collections::HashSet::new();
        let mut result = Vec::with_capacity(self.structs.len());
        for s in &self.structs {
            visit(s, &self.structs, &mut visited, &mut result);
        }
        result
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

    /// C type name in the generated header. Structs and enums cross by
    /// their `__koffi_*` typedef, scalars by their fixed-width C type.
    #[must_use]
    pub fn c_type(&self) -> String {
        match self {
            SchemaTypeRef::Scalar(k) => k.c_type().to_string(),
            SchemaTypeRef::Struct { .. } | SchemaTypeRef::Enum { .. } => self.unique_ident(),
        }
    }

    /// JNI sys type for the native `external fun` signature. Structs and
    /// data-carrying enums marshal through a direct `JByteBuffer`;
    /// scalars and fieldless enums (which cross as their discriminant)
    /// use the bit-identical signed primitive.
    #[must_use]
    pub fn jni_type(&self) -> String {
        match self {
            SchemaTypeRef::Scalar(k) => k.jni_type().to_string(),
            SchemaTypeRef::Enum {
                discriminant,
                has_data: false,
                ..
            } => discriminant.jni_type().to_string(),
            SchemaTypeRef::Struct { .. } | SchemaTypeRef::Enum { has_data: true, .. } => {
                "JByteBuffer".to_string()
            }
        }
    }

    /// Kotlin type of the `private external fun` parameter or return.
    /// JNI primitives are signed (`jshort` = `Short`, ...), so unsigned
    /// kinds arrive as their signed counterpart; the byte width still
    /// matches, and `to_ffm_suffix`/`from_ffm_suffix` do the value
    /// conversions at the call sites.
    #[must_use]
    pub fn jni_kotlin_type(&self) -> String {
        match self {
            SchemaTypeRef::Scalar(k) => k.jni_kotlin_type().to_string(),
            SchemaTypeRef::Enum {
                discriminant,
                has_data: false,
                ..
            } => discriminant.jni_kotlin_type().to_string(),
            SchemaTypeRef::Struct { .. } | SchemaTypeRef::Enum { has_data: true, .. } => {
                "ByteBuffer".to_string()
            }
        }
    }

    /// True for plain structs (always marshalled through a memory segment).
    /// Fieldless enums and scalars cross the ABI as a single value.
    #[must_use]
    pub const fn is_struct(&self) -> bool {
        matches!(self, SchemaTypeRef::Struct { .. })
    }

    /// True for types that cross the FFI boundary as a `MemorySegment` and
    /// need a `SegmentAllocator` argument: structs (always) and
    /// data-carrying enums. Scalars and fieldless enums pass as plain
    /// values.
    #[must_use]
    pub const fn is_memory_backed(&self) -> bool {
        matches!(
            self,
            SchemaTypeRef::Struct { .. } | SchemaTypeRef::Enum { has_data: true, .. }
        )
    }

    #[must_use]
    pub fn kotlin_ffm_value_layout(&self) -> String {
        match self {
            SchemaTypeRef::Scalar(k) => k.kotlin_ffm_value_layout().to_string(),
            SchemaTypeRef::Struct { .. } => format!("{}Layout", self.unique_ident()),
            SchemaTypeRef::Enum {
                discriminant,
                has_data,
                ..
            } => {
                if *has_data {
                    format!("{}Layout", self.unique_ident())
                } else {
                    discriminant.kotlin_ffm_value_layout().to_string()
                }
            }
        }
    }

    /// Identifier of the top-level marshalling helper for a data-carrying
    /// enum (`statusToFfm`) or a struct (`payloadToFfm`). Scalars and
    /// fieldless enums never use this, they cross the FFI boundary as
    /// plain values.
    #[must_use]
    pub fn to_ffm_ident(&self) -> String {
        match self {
            SchemaTypeRef::Enum {
                name,
                has_data: true,
                ..
            } => {
                format!("{}ToFfm", name.to_snake_case())
            }
            SchemaTypeRef::Struct { name, .. } => {
                format!("{}ToFfm", name.to_snake_case())
            }
            _ => String::new(),
        }
    }

    #[must_use]
    pub fn from_ffm_ident(&self) -> String {
        match self {
            SchemaTypeRef::Enum {
                name,
                has_data: true,
                ..
            } => {
                format!("{}FromFfm", name.to_snake_case())
            }
            SchemaTypeRef::Struct { name, .. } => {
                format!("{}FromFfm", name.to_snake_case())
            }
            _ => String::new(),
        }
    }

    /// Suffix turning a Kotlin value into the C scalar, for the argument
    /// spot in native.kt.j2. Scalars are identity (cinterop already maps
    /// `uint8_t` to `UByte` and so on), `bool` widens explicitly, and a
    /// fieldless enum contributes its discriminant value, which is the
    /// exact C type of its typedef. Structs and data-carrying enums never
    /// use this — they marshal through `toC` `CValue`s.
    #[must_use]
    pub fn to_c_suffix(&self) -> String {
        match self {
            SchemaTypeRef::Scalar(k) => k.to_c_suffix().to_string(),
            SchemaTypeRef::Enum {
                has_data: false, ..
            } => ".discriminant".to_string(),
            SchemaTypeRef::Struct { .. } | SchemaTypeRef::Enum { has_data: true, .. } => {
                String::new()
            }
        }
    }

    /// Suffix turning a C scalar back into a Kotlin value, for the
    /// field-read and return spots in native.kt.j2. Fieldless enums
    /// roundtrip through `fromDiscriminant`; bool narrows back from the
    /// `uint8_t` it crossed the ABI as.
    #[must_use]
    pub fn from_c_suffix(&self) -> String {
        match self {
            SchemaTypeRef::Scalar(k) => k.from_c_suffix().to_string(),
            SchemaTypeRef::Enum {
                name,
                has_data: false,
                ..
            } => format!(".let {{ {name}.fromDiscriminant(it) }}"),
            SchemaTypeRef::Struct { .. } | SchemaTypeRef::Enum { has_data: true, .. } => {
                String::new()
            }
        }
    }

    /// Top-level marshalling helper written to native.kt.j2
    /// (`payloadToC`, `statusToC`). Structs and data-carrying enums
    /// marshal through `CValue`s; scalars and fieldless enums cross as
    /// plain values and never use this.
    #[must_use]
    pub fn to_c_ident(&self) -> String {
        match self {
            SchemaTypeRef::Enum {
                name,
                has_data: true,
                ..
            } => format!("{}ToC", name.to_snake_case()),
            SchemaTypeRef::Struct { name, .. } => format!("{}ToC", name.to_snake_case()),
            _ => String::new(),
        }
    }

    #[must_use]
    pub fn from_c_ident(&self) -> String {
        match self {
            SchemaTypeRef::Enum {
                name,
                has_data: true,
                ..
            } => format!("{}FromC", name.to_snake_case()),
            SchemaTypeRef::Struct { name, .. } => format!("{}FromC", name.to_snake_case()),
            _ => String::new(),
        }
    }

    /// Top-level marshalling helpers written to jni.kt.j2 (`payloadToWire`,
    /// `payloadFromWire`, `payloadToBuf`): the direct-ByteBuffer
    /// counterparts of the ffm segment marshallers. `ToWire` writes into
    /// an existing buffer (nested fields recurse via `section`), `FromWire`
    /// reads one back, and `ToBuf` allocates a fresh wire buffer for a
    /// function argument.
    #[must_use]
    pub fn to_wire_ident(&self) -> String {
        match self {
            SchemaTypeRef::Enum {
                name,
                has_data: true,
                ..
            } => format!("{}ToWire", name.to_snake_case()),
            SchemaTypeRef::Struct { name, .. } => format!("{}ToWire", name.to_snake_case()),
            _ => String::new(),
        }
    }

    #[must_use]
    pub fn from_wire_ident(&self) -> String {
        match self {
            SchemaTypeRef::Enum {
                name,
                has_data: true,
                ..
            } => format!("{}FromWire", name.to_snake_case()),
            SchemaTypeRef::Struct { name, .. } => format!("{}FromWire", name.to_snake_case()),
            _ => String::new(),
        }
    }

    #[must_use]
    pub fn to_buf_ident(&self) -> String {
        match self {
            SchemaTypeRef::Enum {
                name,
                has_data: true,
                ..
            } => format!("{}ToBuf", name.to_snake_case()),
            SchemaTypeRef::Struct { name, .. } => format!("{}ToBuf", name.to_snake_case()),
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
            SchemaTypeRef::Enum {
                discriminant,
                has_data: false,
                ..
            } => {
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
            SchemaTypeRef::Enum {
                name,
                discriminant,
                has_data: false,
                ..
            } => {
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
}

impl SchemaFn {
    /// FFI-object member name for a parent-having fn: the `expect`/`actual`
    /// fun generated on the shared object. Parent-having fns are prefixed
    /// with the parent type (module-aware) so `Payload::new` and
    /// `Status::new` (or the same type name in another module) can't
    /// collide on the one object. Free fns never use this: they have no
    /// `parent`, so they land as top-level Kotlin functions under their
    /// plain `kotlin_name`.
    #[must_use]
    pub fn ffi_member_name(&self) -> String {
        let Some(parent) = &self.parent else {
            return self.kotlin_name.clone();
        };
        let (name, module_path) = match parent {
            SchemaTypeRef::Struct { name, module_path }
            | SchemaTypeRef::Enum {
                name, module_path, ..
            } => (name, module_path),
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

    /// The C type for this scalar in the generated header. bool is
    /// `uint8_t`; everything else is the matching fixed-width type, so a
    /// `repr(C)` struct in Rust and the C struct in the header always agree
    /// without padding bookkeeping.
    #[must_use]
    pub const fn c_type(&self) -> &'static str {
        match self {
            ScalarKind::Bool => "uint8_t",
            ScalarKind::U8 => "uint8_t",
            ScalarKind::U16 => "uint16_t",
            ScalarKind::U32 => "uint32_t",
            ScalarKind::U64 => "uint64_t",
            ScalarKind::I8 => "int8_t",
            ScalarKind::I16 => "int16_t",
            ScalarKind::I32 => "int32_t",
            ScalarKind::I64 => "int64_t",
            ScalarKind::F32 => "float",
            ScalarKind::F64 => "double",
        }
    }

    /// JNI sys type for this scalar (`jboolean`, `jint`, ...). The jni
    /// crate aliases these to the matching fixed-width primitive, so
    /// scalar values cross the boundary with plain `as` casts, bit for bit.
    #[must_use]
    pub const fn jni_type(&self) -> &'static str {
        match self {
            ScalarKind::Bool => "jboolean",
            ScalarKind::U8 | ScalarKind::I8 => "jbyte",
            ScalarKind::U16 | ScalarKind::I16 => "jshort",
            ScalarKind::U32 | ScalarKind::I32 => "jint",
            ScalarKind::U64 | ScalarKind::I64 => "jlong",
            ScalarKind::F32 => "jfloat",
            ScalarKind::F64 => "jdouble",
        }
    }

    /// Kotlin type of the JNI primitive: `jshort` surfaces as `Short`,
    /// `jint` as `Int`, and so on. The JVM bridges the signed primitive
    /// to this type directly.
    #[must_use]
    pub const fn jni_kotlin_type(&self) -> &'static str {
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

    /// `ByteBuffer` absolute putter for the wire image of this scalar
    /// (`putShort`, `putInt`, ...). Bool uses `put` but never reaches the
    /// generic branch (it needs the `if` conversion, not `.toByte()`).
    #[must_use]
    pub const fn wire_putter(&self) -> &'static str {
        match self {
            ScalarKind::Bool | ScalarKind::U8 | ScalarKind::I8 => "put",
            ScalarKind::U16 | ScalarKind::I16 => "putShort",
            ScalarKind::U32 | ScalarKind::I32 => "putInt",
            ScalarKind::U64 | ScalarKind::I64 => "putLong",
            ScalarKind::F32 => "putFloat",
            ScalarKind::F64 => "putDouble",
        }
    }

    /// `ByteBuffer` absolute getter for the wire image of this scalar
    /// (`getShort`, `getInt`, ...), the read side of `wire_putter`.
    #[must_use]
    pub const fn wire_getter(&self) -> &'static str {
        match self {
            ScalarKind::Bool | ScalarKind::U8 | ScalarKind::I8 => "get",
            ScalarKind::U16 | ScalarKind::I16 => "getShort",
            ScalarKind::U32 | ScalarKind::I32 => "getInt",
            ScalarKind::U64 | ScalarKind::I64 => "getLong",
            ScalarKind::F32 => "getFloat",
            ScalarKind::F64 => "getDouble",
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

    /// C-interop equivalent of `to_ffm_suffix`: cinterop maps every
    /// fixed-width C integer to the matching Kotlin type, so only bool
    /// (which crossed as `uint8_t`) needs converting. The `.let` form
    /// keeps the suffix usable in both argument and `cValue`-field
    /// positions.
    #[must_use]
    pub const fn to_c_suffix(&self) -> &'static str {
        match self {
            ScalarKind::Bool => ".let { if (it) 1u.toUByte() else 0u.toUByte() }",
            _ => "",
        }
    }

    #[must_use]
    pub const fn from_c_suffix(&self) -> &'static str {
        match self {
            ScalarKind::Bool => " != 0u.toUByte()",
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

impl SchemaField {
    /// Kotlin property name for a Rust struct field: lowerCamelCase. The
    /// raw `name` stays the Rust field name for the glue crate's Rust
    /// templates.
    #[must_use]
    pub fn kotlin_name(&self) -> String {
        self.name.to_lower_camel_case()
    }
}

impl FieldPlacement {
    /// Kotlin property name for a reflected field (struct layout or enum
    /// variant payload): lowerCamelCase. The raw `name` stays the Rust
    /// field name for the glue crate's Rust templates.
    #[must_use]
    pub fn kotlin_name(&self) -> String {
        self.name.to_lower_camel_case()
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

    /// Top-level marshalling helpers written to ffm.kt.j2
    /// (`payloadToFfm`, `payloadFromFfm`). Structs are marshalled through
    /// memory segments just like data-carrying enums.
    #[must_use]
    pub fn to_ffm_ident(&self) -> String {
        format!("{}ToFfm", self.name.to_snake_case())
    }

    #[must_use]
    pub fn from_ffm_ident(&self) -> String {
        format!("{}FromFfm", self.name.to_snake_case())
    }

    /// Top-level marshalling helpers written to native.kt.j2
    /// (`payloadToC`, `payloadFromC`): the cinterop counterpart of the
    /// ffm pair, marshalling through `CValue`s.
    #[must_use]
    pub fn to_c_ident(&self) -> String {
        format!("{}ToC", self.name.to_snake_case())
    }

    #[must_use]
    pub fn from_c_ident(&self) -> String {
        format!("{}FromC", self.name.to_snake_case())
    }

    /// Top-level marshalling helpers written to jni.kt.j2
    /// (`payloadToWire`, `payloadFromWire`, `payloadToBuf`), mirroring
    /// `to_c_ident`/`from_c_ident` for direct `ByteBuffer`s.
    #[must_use]
    pub fn to_wire_ident(&self) -> String {
        format!("{}ToWire", self.name.to_snake_case())
    }

    #[must_use]
    pub fn from_wire_ident(&self) -> String {
        format!("{}FromWire", self.name.to_snake_case())
    }

    #[must_use]
    pub fn to_buf_ident(&self) -> String {
        format!("{}ToBuf", self.name.to_snake_case())
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

    /// Top-level marshalling helpers written to native.kt.j2
    /// (`statusToC`, `statusFromC`): the cinterop counterpart of the ffm
    /// pair, marshalling through `CValue`s.
    #[must_use]
    pub fn to_c_ident(&self) -> String {
        format!("{}ToC", self.name.to_snake_case())
    }

    #[must_use]
    pub fn from_c_ident(&self) -> String {
        format!("{}FromC", self.name.to_snake_case())
    }

    /// Top-level marshalling helpers written to jni.kt.j2
    /// (`statusToWire`, `statusFromWire`, `statusToBuf`), mirroring
    /// `to_c_ident`/`from_c_ident` for direct `ByteBuffer`s.
    #[must_use]
    pub fn to_wire_ident(&self) -> String {
        format!("{}ToWire", self.name.to_snake_case())
    }

    #[must_use]
    pub fn from_wire_ident(&self) -> String {
        format!("{}FromWire", self.name.to_snake_case())
    }

    #[must_use]
    pub fn to_buf_ident(&self) -> String {
        format!("{}ToBuf", self.name.to_snake_case())
    }
}
