use core::ffi::c_char;
use core::sync::atomic::AtomicU64;
use rocksdb::custom_cache::{BlockType, ROCKSDB_CACHE_KEY_SIZE};
use rocksdb::{
    statistics, BlockBasedOptions, Cache, CustomCache, CustomCacheCallback, DBCompressionType,
    Options, ReadOptions, DB,
};
use std::backtrace::Backtrace;
use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};

/// Simple in-memory custom cache implementation for testing
#[derive(Debug)]
struct TestCustomCache {
    cache: Mutex<HashMap<[u8; ROCKSDB_CACHE_KEY_SIZE], Arc<[u8]>>>,
    lookup_calls: AtomicU64,
    insert_calls: AtomicU64,
}

impl TestCustomCache {
    fn new() -> Self {
        Self {
            cache: Mutex::new(HashMap::new()),
            lookup_calls: AtomicU64::new(0),
            insert_calls: AtomicU64::new(0),
        }
    }

    fn lookup_count(&self) -> u64 {
        self.lookup_calls.load(Ordering::Relaxed)
    }

    fn insert_count(&self) -> u64 {
        self.insert_calls.load(Ordering::Relaxed)
    }
}

impl CustomCacheCallback for TestCustomCache {
    fn lookup(
        &self,
        key: &[u8; ROCKSDB_CACHE_KEY_SIZE],
        _block_type: BlockType,
    ) -> std::result::Result<Option<*const u8>, i32> {
        let bt = Backtrace::capture();
        self.lookup_calls.fetch_add(1, Ordering::Relaxed);
        let cache = self.cache.lock();
        let value_arc = cache.unwrap().get(key).cloned();
        if let Some(value_arc) = value_arc {
            let ptr = value_arc.as_ptr();
            unsafe {
                Arc::increment_strong_count(Arc::into_raw(value_arc));
            }
            Ok(Some(ptr))
        } else {
            Ok(None)
        }
    }

    fn insert(
        &self,
        key: &[u8; ROCKSDB_CACHE_KEY_SIZE],
        _block_type: BlockType,
        data: &[u8],
    ) -> std::result::Result<(), i32> {
        self.insert_calls.fetch_add(1, Ordering::Relaxed);
        let mut cache = self.cache.lock().unwrap();
        cache.insert(*key, Arc::from(data));
        Ok(())
    }

    fn unref(&self, data: *const c_char) {}
}

#[test]
fn test_custom_cache_integration() {
    let test_cache = Arc::new(TestCustomCache::new());
    let custom_cache = CustomCache::new(test_cache.clone());

    // Create database options with custom cache
    let mut opts = Options::default();
    opts.create_if_missing(true);

    let mut block_opts = BlockBasedOptions::default();
    block_opts.set_custom_cache(&custom_cache);
    block_opts.set_block_cache(&Cache::new_lru_cache(16));
    opts.set_compression_type(DBCompressionType::Snappy);
    opts.set_paranoid_checks(true);
    opts.set_block_based_table_factory(&block_opts);
    opts.set_statistics_level(statistics::StatsLevel::All);
    // Create a temporary directory for the test
    let tempdir = tempfile::Builder::new()
        .prefix("rocksdb_custom_cache_test")
        .tempdir()
        .expect("Failed to create temporary directory");

    let path = tempdir.path();

    {
        let db = DB::open(&opts, path).unwrap();
        let mut keys = Vec::new();
        for _ in 0..100 {
            let key: Vec<u8> = (0..64).map(|_| 0 as u8).collect();
            let value: Vec<u8> = (0..512).map(|_| 1).collect();
            db.put(&key, &value).unwrap();
            keys.push((key, value));
        }

        db.flush().unwrap();
        // Verify that all keys and values were written correctly
        let mut o = ReadOptions::default();
        o.set_verify_checksums(true);
        o.set_readahead_size(0);
        for (key, value) in keys {
            let result = db.get_opt(&key, &o).unwrap();
            assert_eq!(result, Some(value));
        }
    }
    // Verify that custom cache was actually called
    assert!(test_cache.lookup_count() > 0);
    assert!(test_cache.insert_count() > 0);
}
