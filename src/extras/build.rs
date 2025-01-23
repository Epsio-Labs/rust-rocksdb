#[cfg(feature = "extras")]
fn build_extras() {
    println!("cargo:rerun-if-changed=src/extras/ffi.cc");
    println!("cargo:rerun-if-changed=librocksdb-sys/rocksdb");
    let target = std::env::var("TARGET").unwrap();
    let mut config = cc::Build::new();
    // This was taken from librocksdb-sys build.rs
    if target.contains("darwin") {
        config.define("OS_MACOSX", None);
        config.define("ROCKSDB_PLATFORM_POSIX", None);
        config.define("ROCKSDB_LIB_IO_POSIX", None);
    }
    config
        .cpp(true)
        .static_flag(true)
        .file("src/extras/ffi.cc")
        .include("librocksdb-sys/rocksdb/include")
        .flag_if_supported("-O2")
        .flag_if_supported("-std=c++17")
        .compile("extras");
}

fn main() {
    #[cfg(feature = "extras")]
    build_extras();
}
