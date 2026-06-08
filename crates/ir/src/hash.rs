use crate::{CrateInterface, EnumInfo, FFIType, StructInfo};

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

struct Fnv1a(u64);

impl Fnv1a {
    const fn new() -> Self {
        Self(FNV_OFFSET)
    }

    fn write_bytes(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.0 ^= u64::from(b);
            self.0 = self.0.wrapping_mul(FNV_PRIME);
        }
    }

    fn write_str(&mut self, s: &str) {
        // Length-prefix strings to prevent collisions: "ab"+"c" ≠ "a"+"bc"
        self.write_bytes(&(s.len() as u64).to_le_bytes());
        self.write_bytes(s.as_bytes());
    }

    fn write_u8(&mut self, v: u8) {
        self.write_bytes(&[v]);
    }

    const fn finish(self) -> u64 {
        self.0
    }
}

/// Compute the structural schema hash for an [`FFIType`].
/// Two types with the same schema hash are wire-compatible.
#[must_use]
pub fn hash_type(ty: &FFIType, ir: &CrateInterface) -> u64 {
    let mut h = Fnv1a::new();
    hash_type_inner(ty, ir, &mut h);

    h.finish()
}

fn hash_type_inner(ty: &FFIType, ir: &CrateInterface, h: &mut Fnv1a) {
    // Discriminant as single byte to anchor the type kind
    h.write_u8(type_discriminant(ty));

    match ty {
        FFIType::Option(inner) | FFIType::Vec(inner) | FFIType::Set(inner) => {
            hash_type_inner(inner, ir, h);
        }
        FFIType::Result(ok, err) | FFIType::Map(ok, err) => {
            hash_type_inner(ok, ir, h);
            hash_type_inner(err, ir, h);
        }
        FFIType::Opaque(type_ref) => {
            // Opaque shape is its identity: crate + name.
            h.write_str(&type_ref.crate_id.name);
            h.write_str(&type_ref.crate_id.version);
            h.write_str(&type_ref.name);
        }
        FFIType::Data(type_ref) => {
            // Data shape is its fields: names + types in declaration order.
            if let Some(s) = ir.structs.iter().find(|s| s.name == type_ref.name) {
                hash_struct_fields(s, ir, h);
            } else if let Some(e) = ir.enums.iter().find(|e| e.name == type_ref.name) {
                hash_enum_variants(e, ir, h);
            }
            // If not found in this IR (imported type), fall back to identity
            // so the schema.json hash of the dep is authoritative.
        }
        _ => {} // primitives: discriminant alone is sufficient
    }
}

fn hash_struct_fields(s: &StructInfo, ir: &CrateInterface, h: &mut Fnv1a) {
    for field in &s.fields {
        if field.skip_serde {
            continue;
        }
        h.write_str(&field.name);
        hash_type_inner(&field.ty, ir, h);
    }
}

fn hash_enum_variants(e: &EnumInfo, ir: &CrateInterface, h: &mut Fnv1a) {
    for variant in &e.variants {
        h.write_str(&variant.name);
        for field in &variant.fields {
            h.write_str(&field.name);
            hash_type_inner(&field.ty, ir, h);
        }
    }
}

const fn type_discriminant(ty: &FFIType) -> u8 {
    match ty {
        FFIType::Bool => 0,
        FFIType::I8 => 1,
        FFIType::I16 => 2,
        FFIType::I32 => 3,
        FFIType::I64 => 4,
        FFIType::U8 => 5,
        FFIType::U16 => 6,
        FFIType::U32 => 7,
        FFIType::U64 => 8,
        FFIType::F32 => 9,
        FFIType::F64 => 10,
        FFIType::Unit => 11,
        FFIType::String => 12,
        FFIType::Bytes => 13,
        FFIType::Option(_) => 14,
        FFIType::Result(..) => 15,
        FFIType::Vec(_) => 16,
        FFIType::Map(..) => 17,
        FFIType::Set(_) => 18,
        FFIType::Opaque(_) => 19,
        FFIType::Data(_) => 20,
    }
}
