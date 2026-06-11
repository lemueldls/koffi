pub mod envelope;

use std::{
    any::Any,
    panic::{UnwindSafe, catch_unwind},
    sync::{
        Arc, OnceLock,
        atomic::{AtomicU64, Ordering},
    },
};

use dashmap::DashMap;

/// Thread-safe global registry for opaque handles.
/// Kotlin keeps a 64-bit ID (`KoffiHandle`), and Rust holds the real object.
pub struct HandleRegistry {
    map: DashMap<u64, Arc<dyn Any + Send + Sync>>,
    next_id: AtomicU64,
}

impl HandleRegistry {
    /// Retrieve the global instance of the handle registry.
    pub fn global() -> &'static Self {
        static INSTANCE: OnceLock<HandleRegistry> = OnceLock::new();

        INSTANCE.get_or_init(|| {
            HandleRegistry {
                map: DashMap::new(),
                next_id: AtomicU64::new(1),
            }
        })
    }

    /// Insert a value into the registry, returning its unique handle ID.
    pub fn insert<T: Any + Send + Sync>(&self, value: T) -> u64 {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        self.map.insert(id, Arc::new(value));

        id
    }

    /// Insert an existing Arc-wrapped value into the registry.
    pub fn insert_arc<T: Any + Send + Sync>(&self, value: Arc<T>) -> u64 {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        self.map.insert(id, value);

        id
    }

    /// Get a cloned Arc-wrapped reference of a value by its ID.
    pub fn get<T: Any + Send + Sync>(&self, id: u64) -> Option<Arc<T>> {
        self.map
            .get(&id)
            .and_then(|val| val.clone().downcast::<T>().ok())
    }

    /// Remove a value from the registry by its ID.
    pub fn remove(&self, id: u64) -> bool {
        self.map.remove(&id).is_some()
    }
}

/// Frees an opaque handle when the Kotlin garbage collector or manual close is
/// triggered.
#[unsafe(no_mangle)]
pub extern "C" fn koffi_handle_release(handle_id: u64) {
    HandleRegistry::global().remove(handle_id);
}

/// A C-ABI safe byte buffer structure used for passing serialized data across the FFI boundary.
#[repr(C)]
pub struct KoffiByteBuf {
    pub ptr: *mut u8,
    pub len: usize,
    pub cap: usize,
}

impl KoffiByteBuf {
    /// Create a `KoffiByteBuf` from a Rust Vec<u8>, transfering ownership to the caller.
    #[must_use]
    pub const fn new(mut vec: Vec<u8>) -> Self {
        let ptr = vec.as_mut_ptr();
        let len = vec.len();
        let cap = vec.capacity();
        std::mem::forget(vec);

        Self { ptr, len, cap }
    }

    /// Consume this `KoffiByteBuf` and reconstruct the original Vec<u8>,
    /// reclaiming memory ownership.
    ///
    /// # Safety
    ///
    /// This is unsafe as it assumes the pointer was originally allocated by Vec.
    #[must_use]
    pub unsafe fn into_vec(self) -> Vec<u8> {
        if self.ptr.is_null() {
            Vec::new()
        } else {
            unsafe { Vec::from_raw_parts(self.ptr, self.len, self.cap) }
        }
    }
}

/// Frees a `KoffiByteBuf` allocated by Rust.
///
/// # Safety
///
/// The buffer must have been created by `KoffiByteBuf::new`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn koffi_free_byte_buf(buf: KoffiByteBuf) {
    unsafe {
        let _ = buf.into_vec();
    }
}

/// Executes a closure, catching any Rust panics and converting them into a clean Result.
pub fn catch_panic<F, R>(f: F) -> Result<R, String>
where F: FnOnce() -> R + UnwindSafe {
    catch_unwind(f).map_err(|err| {
        if let Some(s) = err.downcast_ref::<&str>() {
            s.to_string()
        } else if let Some(s) = err.downcast_ref::<String>() {
            s.clone()
        } else {
            "Unknown Rust panic".to_string()
        }
    })
}

/// Serializes an object using postcard into a `KoffiByteBuf`.
pub fn serialize_to_buf<T: serde::Serialize>(val: &T) -> Result<KoffiByteBuf, postcard::Error> {
    let bytes = postcard::to_allocvec(val)?;

    Ok(KoffiByteBuf::new(bytes))
}

/// Deserializes an object from raw bytes using postcard.
///
/// # Safety
///
/// The pointer must be valid for the given length.
pub unsafe fn deserialize_from_raw<T: serde::de::DeserializeOwned>(
    ptr: *const u8,
    len: usize,
) -> Result<T, postcard::Error> {
    unsafe {
        let slice = std::slice::from_raw_parts(ptr, len);
        postcard::from_bytes(slice)
    }
}
