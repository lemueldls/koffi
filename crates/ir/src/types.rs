use facet::Facet;

/// Stable identity for a Rust crate. Version is stored as a string to
/// avoid a semver dependency in this leaf crate.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Facet)]
pub struct CrateId {
    pub name: String,
    pub version: String, // SemVer string, e.g. "0.3.1"
}

/// A fully-qualified reference to a user-defined type.
/// This is the unit of identity used for cross-crate deduplication.
///
/// Two [`TypeRef`]s are the same type if and only if all fields match.
/// `schema_hash` is a content hash of the type's *structure* (field names,
/// field types, declaration order), NOT of its name. It is used at runtime
/// to detect version mismatches across independently-updated Rust and Kotlin
/// binaries.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Facet)]
pub struct TypeRef {
    pub crate_id: CrateId,
    pub module_path: Vec<String>, // e.g. ["camera", "frame"] for camera::frame::RgbFrame
    pub name: String,
    pub schema_hash: u64,
}

impl TypeRef {
    /// The canonical string form used in generated code comments and error
    /// messages.
    #[must_use]
    pub fn qualified_name(&self) -> String {
        let mut parts = vec![self.crate_id.name.replace('-', "_")];
        parts.extend(self.module_path.clone());
        parts.push(self.name.clone());

        parts.join("::")
    }
}

/// The primary type enum. Every variant is either a primitive that maps
/// directly to a JVM/C type, a standard-library type resolved by its
/// fully-qualified path, or a user type referenced by [`TypeRef`].
#[derive(Debug, Clone, PartialEq, Eq, Facet)]
#[repr(u8)]
pub enum FFIType {
    Bool,
    I8,
    I16,
    I32,
    I64,
    U8,
    U16,
    U32,
    U64,
    F32,
    F64,
    Unit,

    String, // std::string::String or &str
    Bytes,  // Vec<u8> or &[u8]
    Option(Box<FFIType>),
    Result(Box<FFIType>, Box<FFIType>), // (Ok, Err)
    Vec(Box<FFIType>),
    Map(Box<FFIType>, Box<FFIType>), // (Key, Value)
    Set(Box<FFIType>),

    /// A Rust struct managed as an opaque integer handle via `HandleRegistry`.
    /// The Kotlin side holds only a `Long` handle ID and calls methods
    /// through generated JNI/C-ABI wrappers.
    Opaque(TypeRef),

    /// A Rust struct or enum serialized with postcard across the boundary.
    /// The type must implement `serde::Serialize + serde::Deserialize + Clone`.
    Data(TypeRef),
}

impl FFIType {
    /// True for types that map to a JVM/C primitive with no serialization.
    #[must_use]
    pub const fn is_blittable(&self) -> bool {
        matches!(
            self,
            Self::Bool
                | Self::I8
                | Self::I16
                | Self::I32
                | Self::I64
                | Self::U8
                | Self::U16
                | Self::U32
                | Self::U64
                | Self::F32
                | Self::F64
        )
    }

    /// True if the type is passed as a JNI `jlong` handle ID.
    #[must_use]
    pub const fn is_opaque(&self) -> bool {
        matches!(self, Self::Opaque(_))
    }

    /// True if the type is serialized to/from a postcard byte array.
    #[must_use]
    pub const fn is_serialized(&self) -> bool {
        matches!(
            self,
            Self::Option(_)
                | Self::Result(..)
                | Self::Vec(_)
                | Self::Map(..)
                | Self::Set(_)
                | Self::Data(_)
        )
    }

    /// Returns all [`TypeRef`]s transitively referenced by this type.
    #[must_use]
    pub fn collect_type_refs(&self) -> Vec<&TypeRef> {
        match self {
            Self::Opaque(r) | Self::Data(r) => vec![r],
            Self::Option(inner) | Self::Vec(inner) | Self::Set(inner) => inner.collect_type_refs(),
            Self::Result(ok, err) | Self::Map(ok, err) => {
                let mut refs = ok.collect_type_refs();
                refs.extend(err.collect_type_refs());

                refs
            }
            _ => vec![],
        }
    }

    /// True if this type needs an additional `size` parameter in the C header
    /// for array length.
    #[must_use]
    pub const fn needs_len_param(&self) -> bool {
        matches!(self, Self::String | Self::Bytes | Self::Data(_))
    }
}
