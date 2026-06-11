use thiserror::Error;

use crate::KoffiByteBuf;

pub const MAGIC: u16 = 0x4b46;
pub const VERSION: u16 = 0x0001;

/// Serialize a value using postcard and wrap it in a Koffi envelope.
pub fn serialize_envelope<T: serde::Serialize>(
    value: &T,
    schema_hash: u64,
) -> Result<KoffiByteBuf, postcard::Error> {
    let payload = postcard::to_allocvec(value)?;
    let mut buf = Vec::with_capacity(16 + payload.len());
    buf.extend_from_slice(&MAGIC.to_le_bytes());
    buf.extend_from_slice(&VERSION.to_le_bytes());
    // 4 bytes padding to align schema_hash to 8 bytes
    buf.extend_from_slice(&[0u8; 4]);
    buf.extend_from_slice(&schema_hash.to_le_bytes());
    buf.extend_from_slice(&payload);

    Ok(KoffiByteBuf::new(buf))
}

/// Deserialize a Koffi envelope and verify the schema hash.
///
/// # Safety
///
/// The pointer must be valid for `len` bytes.
pub unsafe fn deserialize_envelope<T: serde::de::DeserializeOwned>(
    ptr: *const u8,
    len: usize,
    expected_hash: u64,
) -> Result<T, EnvelopeError> {
    if len < 16 {
        return Err(EnvelopeError::TooShort(len));
    }
    let bytes = unsafe { std::slice::from_raw_parts(ptr, len) };

    let magic = u16::from_le_bytes(bytes[0..2].try_into().expect("Invalid magic bytes"));
    let version = u16::from_le_bytes(bytes[2..4].try_into().expect("Invalid version bytes"));
    // bytes[4..8] = padding
    let hash = u64::from_le_bytes(bytes[8..16].try_into().expect("Invalid hash bytes"));

    if magic != MAGIC {
        return Err(EnvelopeError::BadMagic(magic));
    }
    if version != VERSION {
        return Err(EnvelopeError::UnsupportedVersion(version));
    }
    if expected_hash != 0 && hash != expected_hash {
        return Err(EnvelopeError::HashMismatch {
            expected: expected_hash,
            actual: hash,
        });
    }

    postcard::from_bytes(&bytes[16..]).map_err(EnvelopeError::Postcard)
}

#[derive(Error, Debug)]
pub enum EnvelopeError {
    #[error("Envelope too short: {0} bytes")]
    TooShort(usize),

    #[error("Bad magic: 0x{0:04x}")]
    BadMagic(u16),

    #[error("Unsupported envelope version: {0}")]
    UnsupportedVersion(u16),

    #[error("Schema hash mismatch: expected 0x{expected:016x}, got 0x{actual:016x}")]
    HashMismatch { expected: u64, actual: u64 },

    #[error("Postcard error: {0}")]
    Postcard(#[from] postcard::Error),
}
