#include "rocksdb/options.h"
#include "stdio.h"
using namespace ROCKSDB_NAMESPACE;

// In the FFI the structs themselves is exported, but only as a forward
// declaration. Since we access fields we must include the definitions here. The
// safety of all of this is verified by tests in the rust side

struct rocksdb_compactoptions_t {
  CompactRangeOptions rep;
  Slice full_history_ts_low;
};

struct rocksdb_flushoptions_t {
  FlushOptions rep;
};

struct rocksdb_options_t {
  Options rep;
};

extern "C" {
void rocksdb_extras_compactoptions_set_max_subcompactions(
    rocksdb_compactoptions_t *compact_options, uint32_t max_subcompactions) {
  compact_options->rep.max_subcompactions = max_subcompactions;
}

// This only exists for memory safety tests. We assume that if setting the level
// works, so does setting the max subcompactions
int rocksdb_extras_compactoptions_get_target_level(
    rocksdb_compactoptions_t *compact_options) {
  return compact_options->rep.target_level;
}

void rocksdb_extras_flushoptions_set_allow_write_stall(
    rocksdb_flushoptions_t *flush_options, bool allow_write_stall) {
  flush_options->rep.allow_write_stall = allow_write_stall;
}

bool rocksdb_extras_flushoptions_get_allow_write_stall(
    rocksdb_flushoptions_t *flush_options) {
  return flush_options->rep.allow_write_stall;
}
void rocksdb_extras_options_set_avoid_flush_during_shutdown(
    rocksdb_options_t *opt, unsigned char val) {
  opt->rep.avoid_flush_during_shutdown = val;
}
}
