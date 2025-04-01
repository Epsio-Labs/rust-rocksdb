use std::{ffi::c_int, sync::Once};

use crate::CompactOptions;
use crate::{
    ffi::{rocksdb_compactoptions_t, rocksdb_flushoptions_t, rocksdb_options_t},
    FlushOptions,
};

extern "C" {
    pub(crate) fn rocksdb_extras_compactoptions_set_max_subcompactions(
        compact_options_ptr: *mut rocksdb_compactoptions_t,
        max_subcompactions: u32,
    );
    pub(crate) fn rocksdb_extras_compactoptions_get_target_level(
        compact_options_ptr: *const rocksdb_compactoptions_t,
    ) -> c_int;

    pub(crate) fn rocksdb_extras_flushoptions_set_allow_write_stall(
        flush_options_ptr: *const rocksdb_flushoptions_t,
        set_allow_write_stalls: bool,
    );
    pub(crate) fn rocksdb_extras_flushoptions_get_allow_write_stall(
        flush_options_ptr: *const rocksdb_flushoptions_t,
    ) -> bool;

    pub(crate) fn rocksdb_extras_options_set_avoid_flush_during_shutdown(
        opt: *mut rocksdb_options_t,
        val: u8,
    );
}

static VERIFY_SAFETY: Once = Once::new();

pub fn verify_extras_safety_once() {
    // No need to run this more than once. Once the first rocksdb is opened, this is verified
    VERIFY_SAFETY.call_once(|| {
        verify_compact_options_cast_safety();
        verify_flush_options_cast_safety();
    });
}

pub fn verify_compact_options_cast_safety() {
    let mut options = CompactOptions::default();
    let test_target_level = 0xe9510;
    options.set_target_level(test_target_level);
    let read_target_level = options.get_target_level();
    assert!(
        read_target_level == test_target_level,
        "Failed to safely read target level"
    );
}

pub fn verify_flush_options_cast_safety() {
    let mut options = FlushOptions::new();
    assert!(
        !options.get_allow_write_stall() && options.get_wait(),
        "Invalid flush options layout"
    );
    options.set_wait(false);
    assert!(
        !options.get_allow_write_stall() && !options.get_wait(),
        "Invalid flush options layout"
    );
    options.set_allow_write_stall(true);
    assert!(
        options.get_allow_write_stall() && !options.get_wait(),
        "Invalid flush options layout"
    );
}
