use std::mem;
use std::ptr::NonNull;
use std::slice;
use std::sync::Arc;
use std::{ffi::c_void, ptr};

use libc::{c_char, c_int, size_t};
use std::backtrace::Backtrace;

use crate::ffi;

pub const ROCKSDB_CACHE_KEY_SIZE: usize = 16;

#[derive(Debug, PartialEq, Eq)]
pub enum BlockType {
    Data = 0,
    Filter = 1,
    FilterPartitionIndex = 2,
    Properties = 3,
    CompressionDictionary = 4,
    RangeDeletion = 5,
    HashIndexPrefixes = 6,
    HashIndexMetadata = 7,
    MetaIndex = 8,
    Index = 9,
}

impl From<i32> for BlockType {
    fn from(value: i32) -> Self {
        match value {
            0 => BlockType::Data,
            1 => BlockType::Filter,
            2 => BlockType::FilterPartitionIndex,
            3 => BlockType::Properties,
            4 => BlockType::CompressionDictionary,
            5 => BlockType::RangeDeletion,
            6 => BlockType::HashIndexPrefixes,
            7 => BlockType::HashIndexMetadata,
            8 => BlockType::MetaIndex,
            9 => BlockType::Index,
            _ => unreachable!("Unknown block type: {}", value),
        }
    }
}

pub trait CustomCacheCallback: Send + Sync {
    /// Lookup a cache entry.
    /// Returns Ok(Some(Arc<[u8]>)) if found, Ok(None) if not found, or Err for errors.
    fn lookup(
        &self,
        key: &[u8; ROCKSDB_CACHE_KEY_SIZE],
        block_type: BlockType,
    ) -> Result<Option<*const u8>, i32>;

    /// Insert a cache entry.
    /// Returns Ok(()) on success or Err(error_code) on failure.
    fn insert(
        &self,
        key: &[u8; ROCKSDB_CACHE_KEY_SIZE],
        block_type: BlockType,
        data: &[u8],
    ) -> Result<(), i32>;

    /// Called by RocksDB when the block data buffer reference is dropped on the RocksDB side
    fn unref(&self, data: *const c_char);
}

pub struct CallbackOwner {
    cb: Arc<dyn CustomCacheCallback>,
    // c_ptr is equivalent to: Box<Box dyn CustomCacheCallback>>. So a pointer to the fat pointer. The box it points to should be freed before
    // cb is
    c_ptr: *const c_void,
}

impl CallbackOwner {
    pub fn new(mut cb: Arc<dyn CustomCacheCallback>) -> Self {
        let raw_trait_ptr: *const dyn CustomCacheCallback = &*cb;
        let trait_thin_pointer: *const *const dyn CustomCacheCallback =
            Box::into_raw(Box::new(raw_trait_ptr));
        Self {
            cb,
            c_ptr: trait_thin_pointer as *const c_void,
        }
    }

    pub fn as_pointer(&self) -> *const c_void {
        self.c_ptr
    }
}

impl Drop for CallbackOwner {
    fn drop(&mut self) {
        // First free the box that c_ptr points to
        unsafe {
            drop(Box::from_raw(self.c_ptr as *mut c_void));
        }
        // the `cb` box arc refcount is automatically decrefed
    }
}

pub(crate) struct CustomCacheWrapper {
    pub(crate) inner: NonNull<ffi::rocksdb_custom_cache_t>,
    callback: CallbackOwner,
}

impl Drop for CustomCacheWrapper {
    fn drop(&mut self) {
        unsafe {
            ffi::rocksdb_custom_cache_destroy(self.inner.as_ptr());
        }
    }
}

/// Custom cache that allows replacing Linux page cache functionality.
#[derive(Clone)]
pub struct CustomCache(pub(crate) Arc<CustomCacheWrapper>);

impl CustomCache {
    /// Create a new custom cache with the provided callback implementation.
    pub fn new(callback: Arc<dyn CustomCacheCallback>) -> CustomCache {
        let callback = CallbackOwner::new(callback);
        let inner = NonNull::new(unsafe {
            ffi::rocksdb_custom_cache_create(
                Some(lookup_callback),
                Some(insert_callback),
                Some(drop_callback),
                callback.as_pointer(),
            )
        })
        .unwrap();

        CustomCache(Arc::new(CustomCacheWrapper { inner, callback }))
    }
}

// C callback functions that bridge to Rust trait methods

unsafe extern "C" fn lookup_callback(
    user_data: *const c_void,
    key: *const c_char,
    block_type: c_int,
    data: *mut *const c_char,
) -> c_int {
    let ptr_to_dyn = user_data as *const *const dyn CustomCacheCallback;
    let callback = &**ptr_to_dyn;
    let key = &*(key as *const [u8; ROCKSDB_CACHE_KEY_SIZE]);

    match callback.lookup(key, block_type.into()) {
        Ok(Some(found_data)) => {
            let data_ptr = found_data as *const i8;
            *data = data_ptr;
            0
        }
        Ok(None) => 1,                 // Not found
        Err(error_code) => error_code, // Error
    }
}

unsafe extern "C" fn insert_callback(
    user_data: *const c_void,
    key: *const c_char,
    block_type: c_int,
    data: *const c_char,
    data_len: size_t,
) -> c_int {
    let ptr_to_dyn = user_data as *const *const dyn CustomCacheCallback;
    let callback = &**ptr_to_dyn;
    let key = &*(key as *const [u8; ROCKSDB_CACHE_KEY_SIZE]);
    let data_slice = slice::from_raw_parts(data as *const u8, data_len);

    match callback.insert(key, block_type.into(), data_slice) {
        Ok(()) => 0,                   // Success
        Err(error_code) => error_code, // Error
    }
}

unsafe extern "C" fn drop_callback(user_data: *const c_void, data: *const c_char) {
    let ptr_to_dyn = user_data as *const *const dyn CustomCacheCallback;
    let callback = &**ptr_to_dyn;
    callback.unref(data);
}

// Ensure thread safety
unsafe impl Send for CustomCacheWrapper {}
unsafe impl Sync for CustomCacheWrapper {}
