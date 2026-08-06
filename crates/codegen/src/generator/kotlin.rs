use heck::{ToLowerCamelCase, ToSnakeCase, ToUpperCamelCase};

use crate::{
    layout::FieldPlacement,
    schema::{
        ScalarKind, Schema, SchemaEnum, SchemaField, SchemaFn, SchemaParam, SchemaStruct,
        SchemaTypeRef, SchemaWrapper, WrapperKind, WrapperMember,
    },
};

impl Schema {
    #[must_use]
    pub fn ffi_object_name(&self) -> String {
        format!("{}Ffi", self.crate_ident_pascal())
    }

    /// Wire size in bytes of a memory-backed type: the total layout size
    /// of the struct, data-carrying enum or Option/Result wrapper, padding
    /// included. Drives the out-buffer allocation in jni.kt.j2.
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
            SchemaTypeRef::Option { .. } | SchemaTypeRef::Result { .. } => {
                self.wrappers
                    .iter()
                    .find(|w| w.unique_ident == ty.unique_ident())
                    .map_or(0, |w| w.layout.total.size)
            }
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

    /// Does the schema use `Result` anywhere? Decides whether common.kt.j2
    /// declares the generic `KoffiResult` class at all; emitting it for a
    /// Result-less crate would just be dead weight.
    #[must_use]
    pub fn has_result_wrappers(&self) -> bool {
        self.wrappers.iter().any(|w| w.kind == WrapperKind::Result)
    }
}

/// One entry of `Schema::types_in_layout_order`: a type whose FFM layout
/// constant (or C typedef) can embed other layout-bearing types. Fieldless
/// enums and scalars are leaves and never appear.
#[derive(Debug, Clone, Copy)]
pub enum LayoutOrderItem<'a> {
    Struct(&'a SchemaStruct),
    DataEnum(&'a SchemaEnum),
    Wrapper(&'a SchemaWrapper),
}

impl LayoutOrderItem<'_> {
    fn key(&self) -> String {
        match self {
            LayoutOrderItem::Struct(s) => s.unique_ident(),
            LayoutOrderItem::DataEnum(e) => e.unique_ident(),
            LayoutOrderItem::Wrapper(w) => w.unique_ident.clone(),
        }
    }
}

impl Schema {
    /// Structs, data-carrying enums and wrappers in dependency order: any
    /// type whose FFM layout (or C typedef) embeds another's appears after
    /// it. Kotlin `val` layout properties initialize in declaration order
    /// (a forward reference is a compile error) and C typedefs must be
    /// declared before use, so both ffm.kt.j2 and the header emit in this
    /// order. Wrappers add the struct -> wrapper -> enum chain that the old
    /// fixed "enums first, then structs" order couldn't express.
    ///
    /// Cycles are impossible (Rust rejects value-recursive types), so a
    /// simple DFS suffices. Roots are iterated deterministically: data
    /// enums, then structs, then wrappers (BTreeMap-ordered idents).
    #[must_use]
    pub fn types_in_layout_order(&self) -> Vec<LayoutOrderItem<'_>> {
        fn push_type_deps<'a>(
            ty: &SchemaTypeRef,
            schema: &'a Schema,
            out: &mut Vec<LayoutOrderItem<'a>>,
        ) {
            match ty {
                SchemaTypeRef::Struct { name, module_path } => {
                    if let Some(s) = schema
                        .structs
                        .iter()
                        .find(|s| &s.name == name && &s.module_path == module_path)
                    {
                        out.push(LayoutOrderItem::Struct(s));
                    }
                }
                SchemaTypeRef::Enum {
                    name,
                    module_path,
                    has_data: true,
                    ..
                } => {
                    if let Some(e) = schema
                        .enums
                        .iter()
                        .find(|e| &e.name == name && &e.module_path == module_path)
                    {
                        out.push(LayoutOrderItem::DataEnum(e));
                    }
                }
                // The wrapper's own layout embeds its members' layouts
                // (nested wrappers included); visiting the wrapper item
                // handles the members.
                SchemaTypeRef::Option { .. } | SchemaTypeRef::Result { .. } => {
                    if let Some(w) = schema
                        .wrappers
                        .iter()
                        .find(|w| w.unique_ident == ty.unique_ident())
                    {
                        out.push(LayoutOrderItem::Wrapper(w));
                    }
                }
                // Fieldless enums and scalars embed nothing.
                SchemaTypeRef::Enum {
                    has_data: false, ..
                }
                | SchemaTypeRef::Scalar(_) => {}
            }
        }

        fn visit<'a>(
            item: LayoutOrderItem<'a>,
            schema: &'a Schema,
            visited: &mut std::collections::HashSet<String>,
            result: &mut Vec<LayoutOrderItem<'a>>,
        ) {
            if !visited.insert(item.key()) {
                return;
            }

            let mut deps = Vec::new();
            match item {
                LayoutOrderItem::Struct(s) => {
                    for f in &s.fields {
                        push_type_deps(&f.ty, schema, &mut deps);
                    }
                }
                LayoutOrderItem::DataEnum(e) => {
                    for p in &e.layout.placements {
                        push_type_deps(&p.ty, schema, &mut deps);
                    }
                }
                LayoutOrderItem::Wrapper(w) => {
                    for m in &w.members {
                        push_type_deps(&m.ty, schema, &mut deps);
                    }
                }
            }
            for dep in deps {
                visit(dep, schema, visited, result);
            }
            result.push(item);
        }

        let mut visited = std::collections::HashSet::new();
        let mut result = Vec::new();
        let mut roots: Vec<LayoutOrderItem> = Vec::new();
        roots.extend(
            self.enums
                .iter()
                .filter(|e| e.has_data)
                .map(LayoutOrderItem::DataEnum),
        );
        roots.extend(self.structs.iter().map(LayoutOrderItem::Struct));
        roots.extend(self.wrappers.iter().map(LayoutOrderItem::Wrapper));
        for root in roots {
            visit(root, self, &mut visited, &mut result);
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
            // None is `null`; the nullability is what marshals the
            // None case, so the marshaller just checks for null.
            SchemaTypeRef::Option { inner } => format!("{}?", inner.kotlin_type()),
            SchemaTypeRef::Result { ok, err } => {
                format!("KoffiResult<{}, {}>", ok.kotlin_type(), err.kotlin_type())
            }
        }
    }

    /// C type name in the generated header. Structs, enums and wrappers
    /// cross by their `__koffi_*` typedef, scalars by their fixed-width
    /// C type.
    #[must_use]
    pub fn c_type(&self) -> String {
        match self {
            SchemaTypeRef::Scalar(k) => k.c_type().to_string(),
            SchemaTypeRef::Struct { .. } | SchemaTypeRef::Enum { .. } => self.unique_ident(),
            SchemaTypeRef::Option { .. } | SchemaTypeRef::Result { .. } => self.unique_ident(),
        }
    }

    /// JNI sys type for the native `external fun` signature. Structs,
    /// data-carrying enums and wrappers marshal through a direct
    /// `JByteBuffer`; scalars and fieldless enums (which cross as their
    /// discriminant) use the bit-identical signed primitive.
    #[must_use]
    pub fn jni_type(&self) -> String {
        match self {
            SchemaTypeRef::Scalar(k) => k.jni_type().to_string(),
            SchemaTypeRef::Enum {
                discriminant,
                has_data: false,
                ..
            } => discriminant.jni_type().to_string(),
            SchemaTypeRef::Struct { .. }
            | SchemaTypeRef::Enum { has_data: true, .. }
            | SchemaTypeRef::Option { .. }
            | SchemaTypeRef::Result { .. } => "JByteBuffer".to_string(),
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
            SchemaTypeRef::Struct { .. }
            | SchemaTypeRef::Enum { has_data: true, .. }
            | SchemaTypeRef::Option { .. }
            | SchemaTypeRef::Result { .. } => "ByteBuffer".to_string(),
        }
    }

    /// True for plain structs (always marshalled through a memory segment).
    /// Fieldless enums and scalars cross the ABI as a single value.
    #[must_use]
    pub const fn is_struct(&self) -> bool {
        matches!(self, SchemaTypeRef::Struct { .. })
    }

    /// True for types that cross the FFI boundary as a `MemorySegment` and
    /// need a `SegmentAllocator` argument: structs (always), data-carrying
    /// enums, and every Option/Result wrapper. Scalars and fieldless
    /// enums pass as plain values.
    #[must_use]
    pub const fn is_memory_backed(&self) -> bool {
        matches!(
            self,
            SchemaTypeRef::Struct { .. }
                | SchemaTypeRef::Enum { has_data: true, .. }
                | SchemaTypeRef::Option { .. }
                | SchemaTypeRef::Result { .. }
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
            SchemaTypeRef::Option { .. } | SchemaTypeRef::Result { .. } => {
                format!("{}Layout", self.unique_ident())
            }
        }
    }

    /// Identifier of the top-level marshalling helper for a data-carrying
    /// enum (`statusToFfm`), a struct (`payloadToFfm`) or an Option/Result
    /// wrapper (`__koffi_option_u32ToFfm`). Scalars and fieldless enums
    /// never use this, they cross the FFI boundary as plain values.
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
            // Wrapper idents come from the full `unique_ident`
            // (`__koffi_option_u32ToFfm`), a namespace user types can't
            // reach: a user struct named `OptionU32` already owns
            // `option_u32ToFfm`.
            SchemaTypeRef::Option { .. } | SchemaTypeRef::Result { .. } => {
                format!("{}ToFfm", self.unique_ident())
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
            SchemaTypeRef::Option { .. } | SchemaTypeRef::Result { .. } => {
                format!("{}FromFfm", self.unique_ident())
            }
            _ => String::new(),
        }
    }

    /// Suffix turning a Kotlin value into the C scalar, for the argument
    /// spot in native.kt.j2. Scalars are identity (cinterop already maps
    /// `uint8_t` to `UByte` and so on), `bool` widens explicitly, and a
    /// fieldless enum contributes its discriminant value, which is the
    /// exact C type of its typedef. Structs, data-carrying enums and
    /// wrappers never use this - they marshal through `toC` `CValue`s.
    #[must_use]
    pub fn to_c_suffix(&self) -> String {
        match self {
            SchemaTypeRef::Scalar(k) => k.to_c_suffix().to_string(),
            SchemaTypeRef::Enum {
                has_data: false, ..
            } => ".discriminant".to_string(),
            SchemaTypeRef::Struct { .. }
            | SchemaTypeRef::Enum { has_data: true, .. }
            | SchemaTypeRef::Option { .. }
            | SchemaTypeRef::Result { .. } => String::new(),
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
            SchemaTypeRef::Struct { .. }
            | SchemaTypeRef::Enum { has_data: true, .. }
            | SchemaTypeRef::Option { .. }
            | SchemaTypeRef::Result { .. } => String::new(),
        }
    }

    /// Top-level marshalling helper written to native.kt.j2
    /// (`payloadToC`, `statusToC`, `__koffi_option_u32ToC`). Structs,
    /// data-carrying enums and wrappers marshal through `CValue`s;
    /// scalars and fieldless enums cross as plain values and never use
    /// this.
    #[must_use]
    pub fn to_c_ident(&self) -> String {
        match self {
            SchemaTypeRef::Enum {
                name,
                has_data: true,
                ..
            } => format!("{}ToC", name.to_snake_case()),
            SchemaTypeRef::Struct { name, .. } => format!("{}ToC", name.to_snake_case()),
            SchemaTypeRef::Option { .. } | SchemaTypeRef::Result { .. } => {
                format!("{}ToC", self.unique_ident())
            }
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
            SchemaTypeRef::Option { .. } | SchemaTypeRef::Result { .. } => {
                format!("{}FromC", self.unique_ident())
            }
            _ => String::new(),
        }
    }

    /// Top-level marshalling helpers written to jni.kt.j2 (`payloadToWire`,
    /// `payloadFromWire`, `payloadToBuf`, plus the wrapper equivalents):
    /// the direct-ByteBuffer counterparts of the ffm segment marshallers.
    /// `ToWire` writes into an existing buffer (nested fields recurse via
    /// `section`), `FromWire` reads one back, and `ToBuf` allocates a
    /// fresh wire buffer for a function argument.
    #[must_use]
    pub fn to_wire_ident(&self) -> String {
        match self {
            SchemaTypeRef::Enum {
                name,
                has_data: true,
                ..
            } => format!("{}ToWire", name.to_snake_case()),
            SchemaTypeRef::Struct { name, .. } => format!("{}ToWire", name.to_snake_case()),
            SchemaTypeRef::Option { .. } | SchemaTypeRef::Result { .. } => {
                format!("{}ToWire", self.unique_ident())
            }
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
            SchemaTypeRef::Option { .. } | SchemaTypeRef::Result { .. } => {
                format!("{}FromWire", self.unique_ident())
            }
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
            SchemaTypeRef::Option { .. } | SchemaTypeRef::Result { .. } => {
                format!("{}ToBuf", self.unique_ident())
            }
            _ => String::new(),
        }
    }

    /// Suffix turning a Kotlin value into the raw FFI scalar, for the
    /// parameter-arg spot in ffm.kt.j2. A fieldless enum contributes its
    /// discriminant value (`status.discriminant.toUInt()`-shaped); a
    /// data-carrying enum, a struct, or a wrapper never uses this (they
    /// go through a segment and `toFfm`).
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
            SchemaTypeRef::Struct { .. }
            | SchemaTypeRef::Enum { has_data: true, .. }
            | SchemaTypeRef::Option { .. }
            | SchemaTypeRef::Result { .. } => String::new(),
        }
    }

    /// Suffix turning the boxed FFI scalar back into a Kotlin value, for
    /// the field-read spot in ffm.kt.j2. Fieldless enums roundtrip through
    /// `fromDiscriminant`; data-carrying enums, structs and wrappers use
    /// `fromFfm` on a segment and never hit this.
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
            SchemaTypeRef::Struct { .. }
            | SchemaTypeRef::Enum { has_data: true, .. }
            | SchemaTypeRef::Option { .. }
            | SchemaTypeRef::Result { .. } => String::new(),
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
            SchemaTypeRef::Scalar(_)
            | SchemaTypeRef::Option { .. }
            | SchemaTypeRef::Result { .. } => {
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

impl SchemaWrapper {
    /// The Kotlin source type for a wrapper: nullable `T?` for `Option`,
    /// `KoffiResult<Ok, Err>` for `Result`. Recursion composes the inner
    /// type, so `Result<Option<u32>, Status>` reads naturally.
    #[must_use]
    pub fn kotlin_type(&self) -> String {
        match self.kind {
            WrapperKind::Option => format!("{}?", self.some().ty.kotlin_type()),
            WrapperKind::Result => {
                format!(
                    "KoffiResult<{}, {}>",
                    self.ok().ty.kotlin_type(),
                    self.err().ty.kotlin_type()
                )
            }
        }
    }

    /// True for an `Option` wrapper, false for `Result`. Template switch
    /// for the Option-vs-Result From impls and sealed-class reads.
    #[must_use]
    pub fn is_option(&self) -> bool {
        self.kind == WrapperKind::Option
    }

    /// The generated FFM struct layout constant for a wrapper. Wrappers
    /// are always memory-backed, so they always get one, mirroring structs
    /// and data-carrying enums.
    #[must_use]
    pub fn kotlin_ffm_value_layout(&self) -> String {
        format!("{}Layout", self.unique_ident)
    }

    /// Absolute offset of the payload union: all members sit at the same
    /// offset, so this one number drives every inner read/write.
    #[must_use]
    pub fn union_offset(&self) -> u64 {
        self.layout.placements.first().map_or(0, |p| p.offset)
    }

    /// Accessor for the single `some` member of an `Option` wrapper. Askama
    /// can't index into `Vec`, so templates reach members by name.
    #[must_use]
    pub fn some(&self) -> &WrapperMember {
        &self.members[0]
    }

    /// Accessor for the `ok` member of a `Result` wrapper.
    #[must_use]
    pub fn ok(&self) -> &WrapperMember {
        self.members
            .iter()
            .find(|m| m.name == "ok")
            .expect("Result wrapper has an ok member")
    }

    /// Accessor for the `err` member of a `Result` wrapper.
    #[must_use]
    pub fn err(&self) -> &WrapperMember {
        self.members
            .iter()
            .find(|m| m.name == "err")
            .expect("Result wrapper has an err member")
    }

    #[must_use]
    pub fn to_ffm_ident(&self) -> String {
        format!("{}ToFfm", self.unique_ident)
    }

    #[must_use]
    pub fn from_ffm_ident(&self) -> String {
        format!("{}FromFfm", self.unique_ident)
    }

    #[must_use]
    pub fn to_c_ident(&self) -> String {
        format!("{}ToC", self.unique_ident)
    }

    #[must_use]
    pub fn from_c_ident(&self) -> String {
        format!("{}FromC", self.unique_ident)
    }

    #[must_use]
    pub fn to_wire_ident(&self) -> String {
        format!("{}ToWire", self.unique_ident)
    }

    #[must_use]
    pub fn from_wire_ident(&self) -> String {
        format!("{}FromWire", self.unique_ident)
    }

    #[must_use]
    pub fn to_buf_ident(&self) -> String {
        format!("{}ToBuf", self.unique_ident)
    }
}
