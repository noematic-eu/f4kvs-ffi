//! FFI bindings for F4KVS with persistent LSM storage.

#![allow(unsafe_op_in_unsafe_fn)]

use f4kvs_lsm::core::config::{WalDurability, WalEngine, WalSyncMode};
use f4kvs_lsm::core::PrefixScanState;
use f4kvs_lsm::{LsmConfig, LsmTreeEngine};
use f4kvs_storage_core::traits::StorageEngine;
use f4kvs_value::Value;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int, c_uchar, c_uint};
use std::path::PathBuf;
use std::ptr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

#[cfg(test)]
thread_local! {
    /// When true, the next `cursor_next_n` fails after taking scan state.
    static CURSOR_FAIL_NEXT: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}
use tokio::runtime::{Handle, Runtime};

/// Maximum key length in bytes (1MB default)
const MAX_KEY_LENGTH: usize = 1 * 1024 * 1024;

/// Maximum value length in bytes (100MB default)
const MAX_VALUE_LENGTH: usize = 100 * 1024 * 1024;

static ENGINE_COUNTER: AtomicU64 = AtomicU64::new(0);
static RUNTIME: OnceLock<Runtime> = OnceLock::new();
static RUNTIME_HANDLE: OnceLock<Handle> = OnceLock::new();

/// FFI-safe result type
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum F4KvsResult {
    Success = 0,
    ErrorInvalidArgument = 1,
    ErrorNotFound = 2,
    ErrorStorage = 3,
    ErrorNetwork = 4,
    ErrorTimeout = 5,
    ErrorUnknown = 99,
}

/// FFI-safe key-value pair returned by prefix scans.
#[repr(C)]
pub struct F4KvsKVPair {
    pub key: *mut c_char,
    pub value: *mut u8,
    pub value_len: usize,
}

/// FFI-safe scan result container.
#[repr(C)]
pub struct F4KvsScanResult {
    pub pairs: *mut F4KvsKVPair,
    pub count: usize,
}

/// Length-prefixed scan pair (no CString / NUL). Free key and value with
/// `f4kvs_bytes_free`.
#[repr(C)]
pub struct F4KvsKVPairKv {
    pub key: *mut u8,
    pub key_len: usize,
    pub value: *mut u8,
    pub value_len: usize,
}

/// Container for `f4kvs_engine_cursor_next_n_kv`. Free with
/// `f4kvs_scan_result_kv_free`.
#[repr(C)]
pub struct F4KvsScanResultKv {
    pub pairs: *mut F4KvsKVPairKv,
    pub count: usize,
}

/// Opaque handle to an LSM-backed F4KVS engine.
pub struct F4KvsEngine {
    engine: Arc<LsmTreeEngine>,
}

fn runtime() -> &'static Runtime {
    RUNTIME.get_or_init(|| Runtime::new().expect("failed to create Tokio runtime for f4kvs-ffi"))
}

fn runtime_handle() -> &'static Handle {
    RUNTIME_HANDLE.get_or_init(|| runtime().handle().clone())
}

/// Drive an async engine call from a Go/C thread.
///
/// `Runtime::block_on` is exclusive (hence the old process-wide mutex).
/// `Handle::block_on` from non-worker threads can run concurrently, so
/// Gets on different shards no longer serialize.
fn block_on<F, T>(future: F) -> T
where
    F: std::future::Future<Output = T>,
{
    runtime_handle().block_on(future)
}

fn key_from_bytes<'a>(bytes: &'a [u8], field: &str) -> Result<&'a str, F4KvsResult> {
    if bytes.len() > MAX_KEY_LENGTH {
        set_last_error(&format!(
            "Invalid argument: {} exceeds maximum length of {} bytes",
            field, MAX_KEY_LENGTH
        ));
        return Err(F4KvsResult::ErrorInvalidArgument);
    }
    match std::str::from_utf8(bytes) {
        Ok(s) => Ok(s),
        Err(e) => {
            set_last_error(&format!("Invalid UTF-8 in {}: {}", field, e));
            Err(F4KvsResult::ErrorInvalidArgument)
        }
    }
}

unsafe fn key_from_raw<'a>(
    ptr: *const u8,
    len: usize,
    field: &str,
) -> Result<&'a str, F4KvsResult> {
    if len == 0 {
        return Ok("");
    }
    if ptr.is_null() {
        set_last_error(&format!("Invalid argument: {} is null", field));
        return Err(F4KvsResult::ErrorInvalidArgument);
    }
    key_from_bytes(std::slice::from_raw_parts(ptr, len), field)
}

fn reject_batch_count(engine: &LsmTreeEngine, count: usize) -> Option<F4KvsResult> {
    let max = engine.config().performance.max_batch_size;
    if count > max {
        set_last_error(&format!(
            "Invalid argument: batch count {count} exceeds max_batch_size {max}"
        ));
        Some(F4KvsResult::ErrorInvalidArgument)
    } else {
        None
    }
}

fn unique_data_dir() -> PathBuf {
    let id = ENGINE_COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("f4kvs_ffi_{}_{}", std::process::id(), id))
}

/// Pre-struct_size layout. First word is group_commit_enabled (0/1).
#[repr(C)]
struct F4KvsOpenOptionsV0 {
    group_commit_enabled: c_uchar,
    group_commit_max_wait_ms: c_uint,
    group_commit_max_batch_size: c_uint,
    group_commit_wait_durable: c_uchar,
    wal_engine: c_uchar,
    wal_durability: c_uchar,
    group_commit_idle_flush_ms: c_uint,
    max_batch_size: c_uint,
    compaction_background: c_uchar,
    max_sstables_per_level: c_uint,
    memtable_max_size: c_uint,
    sstable_target_size: c_uint,
    sstable_max_size: c_uint,
}

/// FFI mirror of `F4KvsOpenOptions` from f4kvs.h
#[repr(C)]
pub struct F4KvsOpenOptions {
    /// Must be `size_of::<Self>()`. Values below 8 are a legacy V0 layout.
    pub struct_size: c_uint,
    pub group_commit_enabled: c_uchar,
    pub group_commit_max_wait_ms: c_uint,
    pub group_commit_max_batch_size: c_uint,
    pub group_commit_wait_durable: c_uchar,
    pub wal_engine: c_uchar,
    pub wal_durability: c_uchar,
    pub group_commit_idle_flush_ms: c_uint,
    /// Max items per batch_put (0 = default 10_000). See f4kvs.h.
    pub max_batch_size: c_uint,
    /// 0 = default (on), 1 = on, 2 = off. See f4kvs.h.
    pub compaction_background: c_uchar,
    /// L0 file-count trigger (0 = default).
    pub max_sstables_per_level: c_uint,
    /// Memtable size in bytes (0 = default 64 MiB).
    pub memtable_max_size: c_uint,
    /// Compaction output target in bytes (0 = default 64 MiB).
    pub sstable_target_size: c_uint,
    /// Compaction output max in bytes (0 = default 128 MiB).
    pub sstable_max_size: c_uint,
}

/// Versioned layouts start at `struct_size`. Legacy first word is 0 or 1.
const MIN_VERSIONED_OPEN_OPTIONS: u32 = 8;

impl F4KvsOpenOptions {
    pub fn new() -> Self {
        Self {
            struct_size: std::mem::size_of::<Self>() as c_uint,
            group_commit_enabled: 0,
            group_commit_max_wait_ms: 0,
            group_commit_max_batch_size: 0,
            group_commit_wait_durable: 0,
            wal_engine: 0,
            wal_durability: 0,
            group_commit_idle_flush_ms: 0,
            max_batch_size: 0,
            compaction_background: 0,
            max_sstables_per_level: 0,
            memtable_max_size: 0,
            sstable_target_size: 0,
            sstable_max_size: 0,
        }
    }
}

impl From<F4KvsOpenOptionsV0> for F4KvsOpenOptions {
    fn from(old: F4KvsOpenOptionsV0) -> Self {
        Self {
            struct_size: 0,
            group_commit_enabled: old.group_commit_enabled,
            group_commit_max_wait_ms: old.group_commit_max_wait_ms,
            group_commit_max_batch_size: old.group_commit_max_batch_size,
            group_commit_wait_durable: old.group_commit_wait_durable,
            wal_engine: old.wal_engine,
            wal_durability: old.wal_durability,
            group_commit_idle_flush_ms: old.group_commit_idle_flush_ms,
            max_batch_size: old.max_batch_size,
            compaction_background: old.compaction_background,
            max_sstables_per_level: old.max_sstables_per_level,
            memtable_max_size: old.memtable_max_size,
            sstable_target_size: old.sstable_target_size,
            sstable_max_size: old.sstable_max_size,
        }
    }
}

/// Copy at most `declared` bytes. Never reads past the caller's struct.
unsafe fn read_open_options(ptr: *const F4KvsOpenOptions) -> Option<F4KvsOpenOptions> {
    if ptr.is_null() {
        return None;
    }
    let first = ptr::read_unaligned(ptr as *const u32);
    if first < MIN_VERSIONED_OPEN_OPTIONS {
        return Some(F4KvsOpenOptions::from(ptr::read_unaligned(
            ptr as *const F4KvsOpenOptionsV0,
        )));
    }
    let n = (first as usize).min(std::mem::size_of::<F4KvsOpenOptions>());
    let mut buf = std::mem::MaybeUninit::<F4KvsOpenOptions>::zeroed();
    ptr::copy_nonoverlapping(ptr as *const u8, buf.as_mut_ptr() as *mut u8, n);
    Some(buf.assume_init())
}

fn apply_open_options(config: &mut LsmConfig, options: Option<&F4KvsOpenOptions>) {
    let Some(options) = options else {
        return;
    };

    let durability = match options.wal_durability {
        1 => WalDurability::Amortized,
        2 => WalDurability::Buffered,
        _ => WalDurability::Strict,
    };
    durability.apply_to(&mut config.wal);

    if options.group_commit_enabled != 0 {
        config.wal.group_commit_enabled = true;
    }
    if options.group_commit_max_wait_ms > 0 {
        config.wal.group_commit_max_wait =
            Duration::from_millis(options.group_commit_max_wait_ms as u64);
    }
    if options.group_commit_max_batch_size > 0 {
        config.wal.group_commit_max_batch_size = options.group_commit_max_batch_size as usize;
    }
    if options.group_commit_wait_durable != 0 {
        config.wal.group_commit_wait_durable = true;
    }
    if options.group_commit_idle_flush_ms > 0 {
        config.wal.group_commit_idle_flush = Some(Duration::from_millis(
            options.group_commit_idle_flush_ms as u64,
        ));
    } else if durability == WalDurability::Amortized && config.wal.group_commit_idle_flush.is_none()
    {
        config.wal.group_commit_idle_flush = Some(Duration::from_millis(100));
    }
    if durability == WalDurability::Amortized && options.group_commit_max_wait_ms == 0 {
        config.wal.group_commit_max_wait = Duration::from_millis(50);
    }
    config.wal.engine = match options.wal_engine {
        1 => WalEngine::Frame,
        2 => WalEngine::Indexed,
        _ => WalEngine::Segment,
    };
    if options.max_batch_size > 0 {
        config.performance.max_batch_size = options.max_batch_size as usize;
    }
    match options.compaction_background {
        2 => config.compaction.background_enabled = false,
        1 => config.compaction.background_enabled = true,
        _ => {}
    }
    if options.max_sstables_per_level > 0 {
        config.levels.max_sstables_per_level = options.max_sstables_per_level as usize;
    }
    if options.memtable_max_size > 0 {
        config.memtable.max_size = options.memtable_max_size as usize;
    }
    if options.sstable_target_size > 0 {
        config.sstable.target_size = options.sstable_target_size as usize;
    }
    if options.sstable_max_size > 0 {
        config.sstable.max_size = options.sstable_max_size as usize;
    }
    if config.sstable.max_size < config.sstable.target_size {
        config.sstable.max_size = config.sstable.target_size;
    }
}

fn open_lsm_engine(
    data_dir: PathBuf,
    options: Option<&F4KvsOpenOptions>,
) -> Result<F4KvsEngine, F4KvsResult> {
    let mut config = LsmConfig::default();
    config.data_dir = data_dir.clone();
    config.wal.dir = data_dir.join("wal");
    // FFI callers use block_on; async fsync on the tokio runtime can stall workers.
    config.wal.sync_mode = WalSyncMode::FsyncAsync;
    apply_open_options(&mut config, options);

    let engine = block_on(LsmTreeEngine::new(config)).map_err(|e| {
        set_last_error(&format!("Failed to open LSM engine: {}", e));
        F4KvsResult::ErrorStorage
    })?;

    Ok(F4KvsEngine {
        engine: Arc::new(engine),
    })
}

fn validate_data_dir(data_dir: *const c_char) -> Result<PathBuf, F4KvsResult> {
    if data_dir.is_null() {
        set_last_error("Invalid argument: data_dir is null");
        return Err(F4KvsResult::ErrorInvalidArgument);
    }

    let cstr = unsafe { CStr::from_ptr(data_dir) };
    let path = cstr.to_str().map_err(|e| {
        set_last_error(&format!("Invalid UTF-8 in data_dir: {}", e));
        F4KvsResult::ErrorInvalidArgument
    })?;

    if path.is_empty() {
        set_last_error("Invalid argument: data_dir is empty");
        return Err(F4KvsResult::ErrorInvalidArgument);
    }

    Ok(PathBuf::from(path))
}

fn value_to_bytes(value: Value) -> Vec<u8> {
    match value {
        Value::Bytes(b) => b,
        Value::String(s) => s.into_bytes(),
        Value::Json(j) => j.to_string().into_bytes(),
        Value::Int64(n) => n.to_string().into_bytes(),
        Value::UInt64(n) => n.to_string().into_bytes(),
        Value::Float64(n) => n.to_string().into_bytes(),
        Value::Bool(b) => b.to_string().into_bytes(),
        Value::Null => Vec::new(),
    }
}

fn value_to_string(value: Value) -> String {
    match value {
        Value::String(s) => s,
        Value::Bytes(b) => String::from_utf8_lossy(&b).to_string(),
        Value::Json(j) => j.to_string(),
        Value::Int64(n) => n.to_string(),
        Value::UInt64(n) => n.to_string(),
        Value::Float64(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Null => String::new(),
    }
}

thread_local! {
    /// Per-thread last-error message. Overwritten on each failure; the
    /// previous allocation is reclaimed at that point (no leak), and the
    /// pointer returned by `f4kvs_get_last_error` stays valid until the
    /// next failed call on the same thread.
    static LAST_ERROR: RefCell<Option<CString>> = const { RefCell::new(None) };
}

fn set_last_error(msg: &str) {
    let cstring = CString::new(msg).unwrap_or_else(|_| {
        CString::new(msg.replace('\0', "\\0")).expect("fallback message contains no NUL")
    });
    LAST_ERROR.with(|slot| *slot.borrow_mut() = Some(cstring));
}

fn get_last_error_ptr() -> *const c_char {
    LAST_ERROR.with(|slot| match &*slot.borrow() {
        Some(cstring) => cstring.as_ptr(),
        None => ptr::null(),
    })
}

struct StringAllocator {
    allocations: Mutex<HashSet<usize>>,
}

impl StringAllocator {
    fn new() -> Self {
        Self {
            allocations: Mutex::new(HashSet::new()),
        }
    }

    fn register(&self, ptr: *mut c_char) -> bool {
        if ptr.is_null() {
            return false;
        }
        let addr = ptr as usize;
        self.allocations.lock().unwrap().insert(addr)
    }

    fn unregister(&self, ptr: *mut c_char) -> bool {
        if ptr.is_null() {
            return false;
        }
        let addr = ptr as usize;
        self.allocations.lock().unwrap().remove(&addr)
    }

    fn is_allocated(&self, ptr: *mut c_char) -> bool {
        if ptr.is_null() {
            return false;
        }
        let addr = ptr as usize;
        self.allocations.lock().unwrap().contains(&addr)
    }
}

struct BytesAllocator {
    allocations: Mutex<HashMap<usize, usize>>,
}

impl BytesAllocator {
    fn new() -> Self {
        Self {
            allocations: Mutex::new(HashMap::new()),
        }
    }

    fn register(&self, ptr: *mut u8, len: usize) -> bool {
        if ptr.is_null() {
            return false;
        }
        self.allocations
            .lock()
            .unwrap()
            .insert(ptr as usize, len)
            .is_none()
    }

    fn unregister(&self, ptr: *mut u8) -> Option<usize> {
        if ptr.is_null() {
            return None;
        }
        self.allocations.lock().unwrap().remove(&(ptr as usize))
    }
}

static STRING_ALLOCATOR: OnceLock<StringAllocator> = OnceLock::new();
static BYTES_ALLOCATOR: OnceLock<BytesAllocator> = OnceLock::new();

fn get_string_allocator() -> &'static StringAllocator {
    STRING_ALLOCATOR.get_or_init(StringAllocator::new)
}

fn get_bytes_allocator() -> &'static BytesAllocator {
    BYTES_ALLOCATOR.get_or_init(BytesAllocator::new)
}

fn validate_c_string(
    ptr: *const c_char,
    max_length: usize,
    field_name: &str,
) -> Result<String, F4KvsResult> {
    if ptr.is_null() {
        let msg = match field_name {
            "key" => "Invalid argument: key is null",
            "value" => "Invalid argument: value is null",
            "prefix" => "Invalid argument: prefix is null",
            _ => "Invalid argument: unknown field is null",
        };
        set_last_error(msg);
        return Err(F4KvsResult::ErrorInvalidArgument);
    }

    let cstr = unsafe { CStr::from_ptr(ptr) };
    let bytes = cstr.to_bytes();

    if bytes.len() > max_length {
        set_last_error(&format!(
            "Invalid argument: {} exceeds maximum length of {} bytes",
            field_name, max_length
        ));
        return Err(F4KvsResult::ErrorInvalidArgument);
    }

    match cstr.to_str() {
        Ok(s) => Ok(s.to_string()),
        Err(e) => {
            set_last_error(&format!("Invalid UTF-8 in {}: {}", field_name, e));
            Err(F4KvsResult::ErrorInvalidArgument)
        }
    }
}

fn validate_engine(engine: *mut F4KvsEngine) -> Result<&'static F4KvsEngine, F4KvsResult> {
    if engine.is_null() {
        set_last_error("Invalid argument: engine is null");
        return Err(F4KvsResult::ErrorInvalidArgument);
    }
    Ok(unsafe { &*engine })
}

fn allocate_c_string(value: String) -> Result<*mut c_char, F4KvsResult> {
    match CString::new(value) {
        Ok(cstr) => {
            let ptr = cstr.into_raw();
            get_string_allocator().register(ptr);
            Ok(ptr)
        }
        Err(e) => {
            set_last_error(&format!("Failed to create C string: {}", e));
            Err(F4KvsResult::ErrorStorage)
        }
    }
}

fn allocate_bytes(value: Vec<u8>) -> Result<(*mut u8, usize), F4KvsResult> {
    let len = value.len();
    let mut boxed = value.into_boxed_slice();
    let ptr = boxed.as_mut_ptr();
    std::mem::forget(boxed);
    get_bytes_allocator().register(ptr, len);
    Ok((ptr, len))
}

/// Create a new F4KVS engine in a temporary data directory.
///
/// # Safety
/// The returned pointer must be freed with `f4kvs_engine_free`.
#[no_mangle]
pub unsafe extern "C" fn f4kvs_engine_new() -> *mut F4KvsEngine {
    match open_lsm_engine(unique_data_dir(), None) {
        Ok(engine) => Box::into_raw(Box::new(engine)),
        Err(e) => {
            set_last_error(&format!("Failed to create engine: {:?}", e));
            ptr::null_mut()
        }
    }
}

/// Open a persistent F4KVS engine at the given data directory.
///
/// # Safety
/// `data_dir` must be a valid null-terminated C string.
#[no_mangle]
pub unsafe extern "C" fn f4kvs_engine_open(data_dir: *const c_char) -> *mut F4KvsEngine {
    let path = match validate_data_dir(data_dir) {
        Ok(path) => path,
        Err(_) => return ptr::null_mut(),
    };

    match open_lsm_engine(path, None) {
        Ok(engine) => Box::into_raw(Box::new(engine)),
        Err(e) => {
            set_last_error(&format!("Failed to open engine: {:?}", e));
            ptr::null_mut()
        }
    }
}

/// Open a persistent F4KVS engine with optional WAL tuning.
///
/// # Safety
/// `data_dir` must be a valid null-terminated C string. `options` may be NULL.
#[no_mangle]
pub unsafe extern "C" fn f4kvs_engine_open_ex(
    data_dir: *const c_char,
    options: *const F4KvsOpenOptions,
) -> *mut F4KvsEngine {
    let path = match validate_data_dir(data_dir) {
        Ok(path) => path,
        Err(_) => return ptr::null_mut(),
    };

    let owned = unsafe { read_open_options(options) };
    match open_lsm_engine(path, owned.as_ref()) {
        Ok(engine) => Box::into_raw(Box::new(engine)),
        Err(e) => {
            set_last_error(&format!("Failed to open engine: {:?}", e));
            ptr::null_mut()
        }
    }
}

/// Shut down the engine and flush pending writes.
///
/// # Safety
/// `engine` must be a valid pointer returned by `f4kvs_engine_new` or `f4kvs_engine_open`.
#[no_mangle]
pub unsafe extern "C" fn f4kvs_engine_close(engine: *mut F4KvsEngine) -> F4KvsResult {
    let engine_ref = match validate_engine(engine) {
        Ok(engine) => engine,
        Err(e) => return e,
    };

    match block_on(engine_ref.engine.shutdown()) {
        Ok(_) => F4KvsResult::Success,
        Err(e) => {
            set_last_error(&format!("Close failed: {}", e));
            F4KvsResult::ErrorStorage
        }
    }
}

/// Free an F4KVS engine instance.
///
/// # Safety
/// The pointer must be valid and not used after this call.
#[no_mangle]
pub unsafe extern "C" fn f4kvs_engine_free(engine: *mut F4KvsEngine) {
    if !engine.is_null() {
        let _ = Box::from_raw(engine);
    }
}

/// Compact the on-disk LSM data.
///
/// # Safety
/// `engine` must be a valid pointer returned by `f4kvs_engine_new` or `f4kvs_engine_open`.
#[no_mangle]
pub unsafe extern "C" fn f4kvs_engine_compact(engine: *mut F4KvsEngine) -> F4KvsResult {
    let engine_ref = match validate_engine(engine) {
        Ok(engine) => engine,
        Err(e) => return e,
    };

    match block_on(engine_ref.engine.compact()) {
        Ok(_) => F4KvsResult::Success,
        Err(e) => {
            set_last_error(&format!("Compact failed: {}", e));
            F4KvsResult::ErrorStorage
        }
    }
}

/// Toggle bulk-import mode (skips per-key SSTable probes during batch_put).
///
/// # Safety
/// `engine` must be a valid pointer returned by `f4kvs_engine_new` or `f4kvs_engine_open`.
#[no_mangle]
pub unsafe extern "C" fn f4kvs_engine_set_bulk_import(
    engine: *mut F4KvsEngine,
    enabled: c_uchar,
) -> F4KvsResult {
    let engine_ref = match validate_engine(engine) {
        Ok(engine) => engine,
        Err(e) => return e,
    };
    engine_ref.engine.set_bulk_import(enabled != 0);
    F4KvsResult::Success
}

/// Flush pending WAL and memtable writes.
///
/// # Safety
/// `engine` must be a valid pointer returned by `f4kvs_engine_new` or `f4kvs_engine_open`.
#[no_mangle]
pub unsafe extern "C" fn f4kvs_engine_flush(engine: *mut F4KvsEngine) -> F4KvsResult {
    let engine_ref = match validate_engine(engine) {
        Ok(engine) => engine,
        Err(e) => return e,
    };

    match block_on(engine_ref.engine.flush()) {
        Ok(_) => F4KvsResult::Success,
        Err(e) => {
            set_last_error(&format!("Flush failed: {}", e));
            F4KvsResult::ErrorStorage
        }
    }
}

/// Flush WAL buffers without flushing memtable to SSTable.
///
/// # Safety
/// `engine` must be a valid pointer returned by `f4kvs_engine_new` or `f4kvs_engine_open`.
#[no_mangle]
pub unsafe extern "C" fn f4kvs_engine_flush_wal(engine: *mut F4KvsEngine) -> F4KvsResult {
    let engine_ref = match validate_engine(engine) {
        Ok(engine) => engine,
        Err(e) => return e,
    };

    match block_on(engine_ref.engine.flush_wal()) {
        Ok(_) => F4KvsResult::Success,
        Err(e) => {
            set_last_error(&format!("Flush WAL failed: {}", e));
            F4KvsResult::ErrorStorage
        }
    }
}

/// Key-only prefix scan result container.
#[repr(C)]
pub struct F4KvsKeyScanResult {
    pub keys: *mut *mut c_char,
    pub count: usize,
}

/// Scan keys by prefix without loading values.
///
/// # Safety
/// `prefix` and `result_out` must be valid pointers.
#[no_mangle]
pub unsafe extern "C" fn f4kvs_engine_scan_prefix_keys(
    engine: *mut F4KvsEngine,
    prefix: *const c_char,
    result_out: *mut F4KvsKeyScanResult,
) -> F4KvsResult {
    if result_out.is_null() {
        set_last_error("Invalid argument: result_out is null");
        return F4KvsResult::ErrorInvalidArgument;
    }

    let engine_ref = match validate_engine(engine) {
        Ok(engine) => engine,
        Err(e) => return e,
    };

    let prefix_str = match validate_c_string(prefix, MAX_KEY_LENGTH, "prefix") {
        Ok(s) => s,
        Err(e) => return e,
    };

    match block_on(engine_ref.engine.scan_prefix(&prefix_str)) {
        Ok(keys) => {
            let count = keys.len();
            if count == 0 {
                (*result_out).keys = ptr::null_mut();
                (*result_out).count = 0;
                return F4KvsResult::Success;
            }

            let mut key_ptrs: Vec<*mut c_char> = Vec::with_capacity(count);
            for key in keys {
                match allocate_c_string(key) {
                    Ok(ptr) => key_ptrs.push(ptr),
                    Err(e) => {
                        for ptr in key_ptrs {
                            f4kvs_string_free(ptr);
                        }
                        return e;
                    }
                }
            }

            let mut boxed = key_ptrs.into_boxed_slice();
            let ptr = boxed.as_mut_ptr();
            std::mem::forget(boxed);

            (*result_out).keys = ptr;
            (*result_out).count = count;
            F4KvsResult::Success
        }
        Err(e) => {
            set_last_error(&format!("Scan prefix keys failed: {}", e));
            F4KvsResult::ErrorStorage
        }
    }
}

/// Free a key-only scan result.
///
/// # Safety
/// `result` must be a pointer to a result filled by `f4kvs_engine_scan_prefix_keys`.
#[no_mangle]
pub unsafe extern "C" fn f4kvs_key_scan_result_free(result: *mut F4KvsKeyScanResult) {
    if result.is_null() {
        return;
    }

    let scan = &mut *result;
    if !scan.keys.is_null() && scan.count > 0 {
        let keys = std::slice::from_raw_parts_mut(scan.keys, scan.count);
        for key in keys {
            f4kvs_string_free(*key);
        }
        let keys_boxed = std::slice::from_raw_parts_mut(scan.keys, scan.count);
        let _ = Box::from_raw(keys_boxed);
        scan.keys = ptr::null_mut();
        scan.count = 0;
    }
}

/// Put a key-value pair.
///
/// # Safety
/// `key` and `value` must be valid null-terminated C strings.
#[no_mangle]
pub unsafe extern "C" fn f4kvs_engine_put(
    engine: *mut F4KvsEngine,
    key: *const c_char,
    value: *const c_char,
) -> F4KvsResult {
    let engine_ref = match validate_engine(engine) {
        Ok(engine) => engine,
        Err(e) => return e,
    };

    let key_str = match validate_c_string(key, MAX_KEY_LENGTH, "key") {
        Ok(s) => s,
        Err(e) => return e,
    };

    let value_str = match validate_c_string(value, MAX_VALUE_LENGTH, "value") {
        Ok(s) => s,
        Err(e) => return e,
    };

    match block_on(engine_ref.engine.put(&key_str, &Value::String(value_str))) {
        Ok(_) => F4KvsResult::Success,
        Err(e) => {
            set_last_error(&format!("Put failed: {}", e));
            F4KvsResult::ErrorStorage
        }
    }
}

/// Put a binary key-value pair.
///
/// # Safety
/// `key` must be a valid null-terminated C string and `value` must point to `value_len` bytes.
#[no_mangle]
pub unsafe extern "C" fn f4kvs_engine_put_bytes(
    engine: *mut F4KvsEngine,
    key: *const c_char,
    value: *const u8,
    value_len: usize,
) -> F4KvsResult {
    let engine_ref = match validate_engine(engine) {
        Ok(engine) => engine,
        Err(e) => return e,
    };

    let key_str = match validate_c_string(key, MAX_KEY_LENGTH, "key") {
        Ok(s) => s,
        Err(e) => return e,
    };

    if value_len > MAX_VALUE_LENGTH {
        set_last_error(&format!(
            "Invalid argument: value exceeds maximum length of {} bytes",
            MAX_VALUE_LENGTH
        ));
        return F4KvsResult::ErrorInvalidArgument;
    }

    if value_len > 0 && value.is_null() {
        set_last_error("Invalid argument: value is null");
        return F4KvsResult::ErrorInvalidArgument;
    }

    let bytes = if value_len == 0 {
        Vec::new()
    } else {
        std::slice::from_raw_parts(value, value_len).to_vec()
    };

    match block_on(engine_ref.engine.put(&key_str, &Value::Bytes(bytes))) {
        Ok(_) => F4KvsResult::Success,
        Err(e) => {
            set_last_error(&format!("Put bytes failed: {}", e));
            F4KvsResult::ErrorStorage
        }
    }
}

/// Put multiple binary key-value pairs in one WAL batch.
///
/// # Safety
/// `keys`, `values`, and `value_lens` must point to `count` valid elements when `count > 0`.
#[no_mangle]
pub unsafe extern "C" fn f4kvs_engine_batch_put_bytes(
    engine: *mut F4KvsEngine,
    keys: *const *const c_char,
    values: *const *const u8,
    value_lens: *const usize,
    count: usize,
) -> F4KvsResult {
    let engine_ref = match validate_engine(engine) {
        Ok(engine) => engine,
        Err(e) => return e,
    };
    if let Some(e) = reject_batch_count(&engine_ref.engine, count) {
        return e;
    }

    if count > 0 && (keys.is_null() || values.is_null() || value_lens.is_null()) {
        set_last_error("Invalid argument: keys, values, or value_lens is null");
        return F4KvsResult::ErrorInvalidArgument;
    }

    let mut items = Vec::with_capacity(count);
    for i in 0..count {
        let key_ptr = unsafe { *keys.add(i) };
        let key_str = match validate_c_string(key_ptr, MAX_KEY_LENGTH, "key") {
            Ok(s) => s,
            Err(e) => return e,
        };

        let value_len = unsafe { *value_lens.add(i) };
        if value_len > MAX_VALUE_LENGTH {
            set_last_error(&format!(
                "Invalid argument: value at index {} exceeds maximum length of {} bytes",
                i, MAX_VALUE_LENGTH
            ));
            return F4KvsResult::ErrorInvalidArgument;
        }

        let value_ptr = unsafe { *values.add(i) };
        if value_len > 0 && value_ptr.is_null() {
            set_last_error(&format!("Invalid argument: value at index {} is null", i));
            return F4KvsResult::ErrorInvalidArgument;
        }

        let bytes = if value_len == 0 {
            Vec::new()
        } else {
            std::slice::from_raw_parts(value_ptr, value_len).to_vec()
        };

        items.push((key_str, Value::Bytes(bytes)));
    }

    match block_on(engine_ref.engine.batch_put(items)) {
        Ok(_) => F4KvsResult::Success,
        Err(e) => {
            set_last_error(&format!("Batch put bytes failed: {}", e));
            F4KvsResult::ErrorStorage
        }
    }
}

/// Get a value by key.
///
/// # Safety
/// `key` must be a valid null-terminated C string and `value_out` must point to a valid `char*`.
#[no_mangle]
pub unsafe extern "C" fn f4kvs_engine_get(
    engine: *mut F4KvsEngine,
    key: *const c_char,
    value_out: *mut *mut c_char,
) -> F4KvsResult {
    if value_out.is_null() {
        set_last_error("Invalid argument: value_out is null");
        return F4KvsResult::ErrorInvalidArgument;
    }

    let engine_ref = match validate_engine(engine) {
        Ok(engine) => engine,
        Err(e) => return e,
    };

    let key_str = match validate_c_string(key, MAX_KEY_LENGTH, "key") {
        Ok(s) => s,
        Err(e) => return e,
    };

    match block_on(engine_ref.engine.get(&key_str)) {
        Ok(Some(value)) => match allocate_c_string(value_to_string(value)) {
            Ok(ptr) => {
                *value_out = ptr;
                F4KvsResult::Success
            }
            Err(e) => e,
        },
        Ok(None) => {
            *value_out = ptr::null_mut();
            F4KvsResult::ErrorNotFound
        }
        Err(e) => {
            set_last_error(&format!("Get failed: {}", e));
            F4KvsResult::ErrorStorage
        }
    }
}

/// Get a binary value by key.
///
/// # Safety
/// `key` must be a valid null-terminated C string and output pointers must be valid.
#[no_mangle]
pub unsafe extern "C" fn f4kvs_engine_get_bytes(
    engine: *mut F4KvsEngine,
    key: *const c_char,
    value_out: *mut *mut u8,
    value_len_out: *mut usize,
) -> F4KvsResult {
    if value_out.is_null() || value_len_out.is_null() {
        set_last_error("Invalid argument: output pointer is null");
        return F4KvsResult::ErrorInvalidArgument;
    }

    let engine_ref = match validate_engine(engine) {
        Ok(engine) => engine,
        Err(e) => return e,
    };

    let key_str = match validate_c_string(key, MAX_KEY_LENGTH, "key") {
        Ok(s) => s,
        Err(e) => return e,
    };

    match block_on(engine_ref.engine.get(&key_str)) {
        Ok(Some(value)) => {
            let bytes = value_to_bytes(value);
            match allocate_bytes(bytes) {
                Ok((ptr, allocated_len)) => {
                    *value_out = ptr;
                    *value_len_out = allocated_len;
                    F4KvsResult::Success
                }
                Err(e) => e,
            }
        }
        Ok(None) => {
            *value_out = ptr::null_mut();
            *value_len_out = 0;
            F4KvsResult::ErrorNotFound
        }
        Err(e) => {
            set_last_error(&format!("Get bytes failed: {}", e));
            F4KvsResult::ErrorStorage
        }
    }
}

/// Delete a key.
///
/// # Safety
/// `key` must be a valid null-terminated C string.
#[no_mangle]
pub unsafe extern "C" fn f4kvs_engine_delete(
    engine: *mut F4KvsEngine,
    key: *const c_char,
) -> F4KvsResult {
    let engine_ref = match validate_engine(engine) {
        Ok(engine) => engine,
        Err(e) => return e,
    };

    let key_str = match validate_c_string(key, MAX_KEY_LENGTH, "key") {
        Ok(s) => s,
        Err(e) => return e,
    };

    match block_on(engine_ref.engine.delete(&key_str)) {
        Ok(_) => F4KvsResult::Success,
        Err(e) => {
            set_last_error(&format!("Delete failed: {}", e));
            F4KvsResult::ErrorStorage
        }
    }
}

/// Delete multiple keys in one call.
///
/// # Safety
/// `keys` must point to `count` valid null-terminated C strings when `count > 0`.
#[no_mangle]
pub unsafe extern "C" fn f4kvs_engine_batch_delete(
    engine: *mut F4KvsEngine,
    keys: *const *const c_char,
    count: usize,
) -> F4KvsResult {
    let engine_ref = match validate_engine(engine) {
        Ok(engine) => engine,
        Err(e) => return e,
    };
    if let Some(e) = reject_batch_count(&engine_ref.engine, count) {
        return e;
    }

    if count > 0 && keys.is_null() {
        set_last_error("Invalid argument: keys is null");
        return F4KvsResult::ErrorInvalidArgument;
    }

    let mut key_strings = Vec::with_capacity(count);
    for i in 0..count {
        let key_ptr = unsafe { *keys.add(i) };
        let key_str = match validate_c_string(key_ptr, MAX_KEY_LENGTH, "key") {
            Ok(s) => s,
            Err(e) => return e,
        };
        key_strings.push(key_str);
    }

    match block_on(engine_ref.engine.batch_delete(key_strings)) {
        Ok(_) => F4KvsResult::Success,
        Err(e) => {
            set_last_error(&format!("Batch delete failed: {}", e));
            F4KvsResult::ErrorStorage
        }
    }
}

/// Check if a key exists.
///
/// # Safety
/// `key` must be a valid null-terminated C string and `exists_out` must point to a valid `int`.
#[no_mangle]
pub unsafe extern "C" fn f4kvs_engine_exists(
    engine: *mut F4KvsEngine,
    key: *const c_char,
    exists_out: *mut c_int,
) -> F4KvsResult {
    if exists_out.is_null() {
        set_last_error("Invalid argument: exists_out is null");
        return F4KvsResult::ErrorInvalidArgument;
    }

    let engine_ref = match validate_engine(engine) {
        Ok(engine) => engine,
        Err(e) => return e,
    };

    let key_str = match validate_c_string(key, MAX_KEY_LENGTH, "key") {
        Ok(s) => s,
        Err(e) => return e,
    };

    match block_on(engine_ref.engine.exists(&key_str)) {
        Ok(exists) => {
            *exists_out = if exists { 1 } else { 0 };
            F4KvsResult::Success
        }
        Err(e) => {
            set_last_error(&format!("Exists check failed: {}", e));
            F4KvsResult::ErrorStorage
        }
    }
}

/// Scan all keys with the given prefix and return key/value pairs.
///
/// # Safety
/// `prefix` must be a valid null-terminated C string and `result_out` must be valid.
#[no_mangle]
pub unsafe extern "C" fn f4kvs_engine_scan_prefix(
    engine: *mut F4KvsEngine,
    prefix: *const c_char,
    result_out: *mut F4KvsScanResult,
) -> F4KvsResult {
    if result_out.is_null() {
        set_last_error("Invalid argument: result_out is null");
        return F4KvsResult::ErrorInvalidArgument;
    }

    let engine_ref = match validate_engine(engine) {
        Ok(engine) => engine,
        Err(e) => return e,
    };

    let prefix_str = match validate_c_string(prefix, MAX_KEY_LENGTH, "prefix") {
        Ok(s) => s,
        Err(e) => return e,
    };

    match block_on(engine_ref.engine.scan_prefix_with_values(&prefix_str)) {
        Ok(items) => {
            let count = items.len();
            if count == 0 {
                (*result_out).pairs = ptr::null_mut();
                (*result_out).count = 0;
                return F4KvsResult::Success;
            }

            let mut pairs: Vec<F4KvsKVPair> = Vec::with_capacity(count);
            for (key, value) in items {
                let key_ptr = match allocate_c_string(key) {
                    Ok(ptr) => ptr,
                    Err(e) => {
                        for pair in &pairs {
                            f4kvs_string_free(pair.key);
                            f4kvs_bytes_free(pair.value);
                        }
                        return e;
                    }
                };

                let bytes = value_to_bytes(value);
                let value_len = bytes.len();
                let (value_ptr, _) = match allocate_bytes(bytes) {
                    Ok(allocation) => allocation,
                    Err(e) => {
                        f4kvs_string_free(key_ptr);
                        for pair in &pairs {
                            f4kvs_string_free(pair.key);
                            f4kvs_bytes_free(pair.value);
                        }
                        return e;
                    }
                };

                pairs.push(F4KvsKVPair {
                    key: key_ptr,
                    value: value_ptr,
                    value_len,
                });
            }

            let mut boxed_pairs = pairs.into_boxed_slice();
            let pairs_ptr = boxed_pairs.as_mut_ptr();
            std::mem::forget(boxed_pairs);

            (*result_out).pairs = pairs_ptr;
            (*result_out).count = count;
            F4KvsResult::Success
        }
        Err(e) => {
            set_last_error(&format!("Scan prefix failed: {}", e));
            F4KvsResult::ErrorStorage
        }
    }
}

/// Free a scan result returned by `f4kvs_engine_scan_prefix`.
///
/// # Safety
/// `result` must be a pointer to a scan result previously filled by the library.
#[no_mangle]
pub unsafe extern "C" fn f4kvs_scan_result_free(result: *mut F4KvsScanResult) {
    if result.is_null() {
        return;
    }

    let scan = &mut *result;
    if !scan.pairs.is_null() && scan.count > 0 {
        let pairs = std::slice::from_raw_parts_mut(scan.pairs, scan.count);
        for pair in pairs {
            f4kvs_string_free(pair.key);
            f4kvs_bytes_free(pair.value);
        }
        let _ = Box::from_raw(std::slice::from_raw_parts_mut(scan.pairs, scan.count));
    }

    scan.pairs = ptr::null_mut();
    scan.count = 0;
}

/// Get the last error message.
#[no_mangle]
pub extern "C" fn f4kvs_get_last_error() -> *const c_char {
    get_last_error_ptr()
}

/// Convert a result code to a string.
#[no_mangle]
pub extern "C" fn f4kvs_result_to_string(result: F4KvsResult) -> *const c_char {
    static SUCCESS: &str = "Success\0";
    static ERROR_INVALID_ARG: &str = "Invalid argument\0";
    static ERROR_NOT_FOUND: &str = "Not found\0";
    static ERROR_STORAGE: &str = "Storage error\0";
    static ERROR_NETWORK: &str = "Network error\0";
    static ERROR_TIMEOUT: &str = "Timeout\0";
    static ERROR_UNKNOWN: &str = "Unknown error\0";

    match result {
        F4KvsResult::Success => SUCCESS.as_ptr() as *const c_char,
        F4KvsResult::ErrorInvalidArgument => ERROR_INVALID_ARG.as_ptr() as *const c_char,
        F4KvsResult::ErrorNotFound => ERROR_NOT_FOUND.as_ptr() as *const c_char,
        F4KvsResult::ErrorStorage => ERROR_STORAGE.as_ptr() as *const c_char,
        F4KvsResult::ErrorNetwork => ERROR_NETWORK.as_ptr() as *const c_char,
        F4KvsResult::ErrorTimeout => ERROR_TIMEOUT.as_ptr() as *const c_char,
        F4KvsResult::ErrorUnknown => ERROR_UNKNOWN.as_ptr() as *const c_char,
    }
}

/// Free a C string allocated by the FFI.
///
/// # Safety
/// `ptr` must be a pointer returned by this library or NULL.
#[no_mangle]
pub unsafe extern "C" fn f4kvs_string_free(ptr: *mut c_char) {
    if ptr.is_null() {
        return;
    }

    let allocator = get_string_allocator();
    if !allocator.is_allocated(ptr) {
        return;
    }

    allocator.unregister(ptr);
    let _ = CString::from_raw(ptr);
}

/// Free a byte buffer allocated by the FFI.
///
/// # Safety
/// `ptr` must be a pointer returned by this library or NULL.
#[no_mangle]
pub unsafe extern "C" fn f4kvs_bytes_free(ptr: *mut u8) {
    if ptr.is_null() {
        return;
    }

    let allocator = get_bytes_allocator();
    if let Some(len) = allocator.unregister(ptr) {
        let _ = Box::from_raw(std::slice::from_raw_parts_mut(ptr, len));
    }
}

/// Binary-key get (UTF-8 key, no C string / hex).
///
/// # Safety
/// `key` is `key_len` bytes (NULL allowed when key_len is 0). Outputs must be valid.
#[no_mangle]
pub unsafe extern "C" fn f4kvs_engine_get_kv(
    engine: *mut F4KvsEngine,
    key: *const u8,
    key_len: usize,
    value_out: *mut *mut u8,
    value_len_out: *mut usize,
) -> F4KvsResult {
    if value_out.is_null() || value_len_out.is_null() {
        set_last_error("Invalid argument: output pointer is null");
        return F4KvsResult::ErrorInvalidArgument;
    }
    let engine_ref = match validate_engine(engine) {
        Ok(engine) => engine,
        Err(e) => return e,
    };
    let key_str = match key_from_raw(key, key_len, "key") {
        Ok(s) => s,
        Err(e) => return e,
    };
    let got = match engine_ref.engine.try_get_sync(key_str) {
        Some(r) => r,
        None => block_on(engine_ref.engine.get(key_str)),
    };
    match got {
        Ok(Some(value)) => {
            let bytes = value_to_bytes(value);
            match allocate_bytes(bytes) {
                Ok((ptr, allocated_len)) => {
                    *value_out = ptr;
                    *value_len_out = allocated_len;
                    F4KvsResult::Success
                }
                Err(e) => e,
            }
        }
        Ok(None) => {
            *value_out = ptr::null_mut();
            *value_len_out = 0;
            F4KvsResult::ErrorNotFound
        }
        Err(e) => {
            set_last_error(&format!("Get kv failed: {}", e));
            F4KvsResult::ErrorStorage
        }
    }
}

/// Binary-key put.
#[no_mangle]
pub unsafe extern "C" fn f4kvs_engine_put_kv(
    engine: *mut F4KvsEngine,
    key: *const u8,
    key_len: usize,
    value: *const u8,
    value_len: usize,
) -> F4KvsResult {
    let engine_ref = match validate_engine(engine) {
        Ok(engine) => engine,
        Err(e) => return e,
    };
    let key_str = match key_from_raw(key, key_len, "key") {
        Ok(s) => s,
        Err(e) => return e,
    };
    if value_len > MAX_VALUE_LENGTH {
        set_last_error(&format!(
            "Invalid argument: value exceeds maximum length of {} bytes",
            MAX_VALUE_LENGTH
        ));
        return F4KvsResult::ErrorInvalidArgument;
    }
    if value_len > 0 && value.is_null() {
        set_last_error("Invalid argument: value is null");
        return F4KvsResult::ErrorInvalidArgument;
    }
    let bytes = if value_len == 0 {
        Vec::new()
    } else {
        std::slice::from_raw_parts(value, value_len).to_vec()
    };
    match block_on(engine_ref.engine.put(&key_str, &Value::Bytes(bytes))) {
        Ok(_) => F4KvsResult::Success,
        Err(e) => {
            set_last_error(&format!("Put kv failed: {}", e));
            F4KvsResult::ErrorStorage
        }
    }
}

/// Binary-key delete.
#[no_mangle]
pub unsafe extern "C" fn f4kvs_engine_delete_kv(
    engine: *mut F4KvsEngine,
    key: *const u8,
    key_len: usize,
) -> F4KvsResult {
    let engine_ref = match validate_engine(engine) {
        Ok(engine) => engine,
        Err(e) => return e,
    };
    let key_str = match key_from_raw(key, key_len, "key") {
        Ok(s) => s,
        Err(e) => return e,
    };
    match block_on(engine_ref.engine.delete(&key_str)) {
        Ok(_) => F4KvsResult::Success,
        Err(e) => {
            set_last_error(&format!("Delete kv failed: {}", e));
            F4KvsResult::ErrorStorage
        }
    }
}

/// Binary-key batch put.
#[no_mangle]
pub unsafe extern "C" fn f4kvs_engine_batch_put_kv(
    engine: *mut F4KvsEngine,
    keys: *const *const u8,
    key_lens: *const usize,
    values: *const *const u8,
    value_lens: *const usize,
    count: usize,
) -> F4KvsResult {
    let engine_ref = match validate_engine(engine) {
        Ok(engine) => engine,
        Err(e) => return e,
    };
    if let Some(e) = reject_batch_count(&engine_ref.engine, count) {
        return e;
    }
    if count > 0
        && (keys.is_null() || key_lens.is_null() || values.is_null() || value_lens.is_null())
    {
        set_last_error("Invalid argument: batch_put_kv pointer is null");
        return F4KvsResult::ErrorInvalidArgument;
    }
    let mut items = Vec::with_capacity(count);
    for i in 0..count {
        let key_len = *key_lens.add(i);
        let key_str = match key_from_raw(*keys.add(i), key_len, "key") {
            Ok(s) => s,
            Err(e) => return e,
        };
        let value_len = *value_lens.add(i);
        if value_len > MAX_VALUE_LENGTH {
            set_last_error("Invalid argument: value exceeds maximum length");
            return F4KvsResult::ErrorInvalidArgument;
        }
        let value_ptr = *values.add(i);
        if value_len > 0 && value_ptr.is_null() {
            set_last_error("Invalid argument: value is null");
            return F4KvsResult::ErrorInvalidArgument;
        }
        let bytes = if value_len == 0 {
            Vec::new()
        } else {
            std::slice::from_raw_parts(value_ptr, value_len).to_vec()
        };
        items.push((key_str.to_owned(), Value::Bytes(bytes)));
    }
    match block_on(engine_ref.engine.batch_put(items)) {
        Ok(_) => F4KvsResult::Success,
        Err(e) => {
            set_last_error(&format!("Batch put kv failed: {}", e));
            F4KvsResult::ErrorStorage
        }
    }
}

/// Binary-key batch delete.
#[no_mangle]
pub unsafe extern "C" fn f4kvs_engine_batch_delete_kv(
    engine: *mut F4KvsEngine,
    keys: *const *const u8,
    key_lens: *const usize,
    count: usize,
) -> F4KvsResult {
    let engine_ref = match validate_engine(engine) {
        Ok(engine) => engine,
        Err(e) => return e,
    };
    if let Some(e) = reject_batch_count(&engine_ref.engine, count) {
        return e;
    }
    if count > 0 && (keys.is_null() || key_lens.is_null()) {
        set_last_error("Invalid argument: batch_delete_kv pointer is null");
        return F4KvsResult::ErrorInvalidArgument;
    }
    let mut key_strings = Vec::with_capacity(count);
    for i in 0..count {
        match key_from_raw(*keys.add(i), *key_lens.add(i), "key") {
            Ok(s) => key_strings.push(s.to_owned()),
            Err(e) => return e,
        }
    }
    match block_on(engine_ref.engine.batch_delete(key_strings)) {
        Ok(_) => F4KvsResult::Success,
        Err(e) => {
            set_last_error(&format!("Batch delete kv failed: {}", e));
            F4KvsResult::ErrorStorage
        }
    }
}

unsafe fn fill_scan_result(
    items: Vec<(String, Value)>,
    result_out: *mut F4KvsScanResult,
) -> F4KvsResult {
    let count = items.len();
    if count == 0 {
        (*result_out).pairs = ptr::null_mut();
        (*result_out).count = 0;
        return F4KvsResult::Success;
    }
    let mut pairs: Vec<F4KvsKVPair> = Vec::with_capacity(count);
    for (key, value) in items {
        let key_ptr = match allocate_c_string(key) {
            Ok(ptr) => ptr,
            Err(e) => {
                for pair in &pairs {
                    f4kvs_string_free(pair.key);
                    f4kvs_bytes_free(pair.value);
                }
                return e;
            }
        };
        let bytes = value_to_bytes(value);
        let value_len = bytes.len();
        let (value_ptr, _) = match allocate_bytes(bytes) {
            Ok(allocation) => allocation,
            Err(e) => {
                f4kvs_string_free(key_ptr);
                for pair in &pairs {
                    f4kvs_string_free(pair.key);
                    f4kvs_bytes_free(pair.value);
                }
                return e;
            }
        };
        pairs.push(F4KvsKVPair {
            key: key_ptr,
            value: value_ptr,
            value_len,
        });
    }
    let mut boxed = pairs.into_boxed_slice();
    let ptr = boxed.as_mut_ptr();
    std::mem::forget(boxed);
    (*result_out).pairs = ptr;
    (*result_out).count = count;
    F4KvsResult::Success
}

/// Binary-prefix scan (values). Result keys are UTF-8 C strings (no hex).
#[no_mangle]
pub unsafe extern "C" fn f4kvs_engine_scan_prefix_kv(
    engine: *mut F4KvsEngine,
    prefix: *const u8,
    prefix_len: usize,
    result_out: *mut F4KvsScanResult,
) -> F4KvsResult {
    if result_out.is_null() {
        set_last_error("Invalid argument: result_out is null");
        return F4KvsResult::ErrorInvalidArgument;
    }
    let engine_ref = match validate_engine(engine) {
        Ok(engine) => engine,
        Err(e) => return e,
    };
    let prefix_str = match key_from_raw(prefix, prefix_len, "prefix") {
        Ok(s) => s,
        Err(e) => return e,
    };
    match block_on(engine_ref.engine.scan_prefix_with_values(&prefix_str)) {
        Ok(items) => fill_scan_result(items, result_out),
        Err(e) => {
            set_last_error(&format!("Scan prefix kv failed: {}", e));
            F4KvsResult::ErrorStorage
        }
    }
}

/// Binary-prefix key-only scan.
#[no_mangle]
pub unsafe extern "C" fn f4kvs_engine_scan_prefix_keys_kv(
    engine: *mut F4KvsEngine,
    prefix: *const u8,
    prefix_len: usize,
    result_out: *mut F4KvsKeyScanResult,
) -> F4KvsResult {
    if result_out.is_null() {
        set_last_error("Invalid argument: result_out is null");
        return F4KvsResult::ErrorInvalidArgument;
    }
    let engine_ref = match validate_engine(engine) {
        Ok(engine) => engine,
        Err(e) => return e,
    };
    let prefix_str = match key_from_raw(prefix, prefix_len, "prefix") {
        Ok(s) => s,
        Err(e) => return e,
    };
    match block_on(engine_ref.engine.scan_prefix(&prefix_str)) {
        Ok(keys) => {
            let count = keys.len();
            if count == 0 {
                (*result_out).keys = ptr::null_mut();
                (*result_out).count = 0;
                return F4KvsResult::Success;
            }
            let mut key_ptrs: Vec<*mut c_char> = Vec::with_capacity(count);
            for key in keys {
                match allocate_c_string(key) {
                    Ok(ptr) => key_ptrs.push(ptr),
                    Err(e) => {
                        for p in key_ptrs {
                            f4kvs_string_free(p);
                        }
                        return e;
                    }
                }
            }
            let mut boxed = key_ptrs.into_boxed_slice();
            let ptr = boxed.as_mut_ptr();
            std::mem::forget(boxed);
            (*result_out).keys = ptr;
            (*result_out).count = count;
            F4KvsResult::Success
        }
        Err(e) => {
            set_last_error(&format!("Scan prefix keys kv failed: {}", e));
            F4KvsResult::ErrorStorage
        }
    }
}

/// Incremental prefix scan. Does not materialize the whole prefix.
pub struct F4KvsCursor {
    engine: Arc<LsmTreeEngine>,
    state: Option<PrefixScanState>,
    prefix: String,
    done: bool,
}

/// Open a prefix cursor. Free with `f4kvs_engine_cursor_free`.
///
/// # Safety
/// `engine` must be a live handle. `prefix` is `prefix_len` UTF-8 bytes.
#[no_mangle]
pub unsafe extern "C" fn f4kvs_engine_cursor_open(
    engine: *mut F4KvsEngine,
    prefix: *const u8,
    prefix_len: usize,
) -> *mut F4KvsCursor {
    let engine_ref = match validate_engine(engine) {
        Ok(e) => e,
        Err(_) => return ptr::null_mut(),
    };
    let prefix = match key_from_raw(prefix, prefix_len, "prefix") {
        Ok(s) => s.to_owned(),
        Err(_) => return ptr::null_mut(),
    };
    Box::into_raw(Box::new(F4KvsCursor {
        engine: Arc::clone(&engine_ref.engine),
        state: None,
        prefix,
        done: false,
    }))
}

fn cursor_ensure_state(cur: &mut F4KvsCursor) -> Result<(), F4KvsResult> {
    if cur.state.is_some() {
        return Ok(());
    }
    let prefix = cur.prefix.clone();
    let started = match cur.engine.try_prefix_scan_start_sync(&prefix) {
        Some(r) => r,
        None => block_on(cur.engine.prefix_scan_start(&prefix)),
    };
    match started {
        Ok(s) => {
            cur.state = Some(s);
            Ok(())
        }
        Err(e) => {
            set_last_error(&format!("Cursor start failed: {}", e));
            Err(F4KvsResult::ErrorStorage)
        }
    }
}

fn cursor_pull_page(
    cur: &mut F4KvsCursor,
    max: usize,
) -> Result<(Vec<(String, Value)>, bool), F4KvsResult> {
    cursor_ensure_state(cur)?;
    let mut state = cur.state.take().expect("cursor state");
    let engine = Arc::clone(&cur.engine);
    let pulled = match engine.try_prefix_scan_next_n_sync(&mut state, max) {
        Some(r) => r,
        None => block_on(async {
            let mut items = Vec::new();
            let mut eof = false;
            for _ in 0..max {
                match engine.prefix_scan_next(&mut state).await {
                    Ok(Some((k, v))) => items.push((k, v)),
                    Ok(None) => {
                        eof = true;
                        break;
                    }
                    Err(e) => return Err(e),
                }
            }
            Ok((items, eof))
        }),
    };
    match pulled {
        Ok((items, eof)) => {
            if !eof {
                cur.state = Some(state);
            }
            Ok((items, eof))
        }
        Err(e) => {
            cur.state = Some(state);
            set_last_error(&format!("Cursor next failed: {}", e));
            Err(F4KvsResult::ErrorStorage)
        }
    }
}

unsafe fn fill_scan_result_kv(
    items: Vec<(String, Value)>,
    result_out: *mut F4KvsScanResultKv,
) -> F4KvsResult {
    let count = items.len();
    if count == 0 {
        (*result_out).pairs = ptr::null_mut();
        (*result_out).count = 0;
        return F4KvsResult::Success;
    }
    let mut pairs: Vec<F4KvsKVPairKv> = Vec::with_capacity(count);
    for (key, value) in items {
        let key_bytes = key.into_bytes();
        let key_len = key_bytes.len();
        let (key_ptr, _) = match allocate_bytes(key_bytes) {
            Ok(a) => a,
            Err(e) => {
                for pair in &pairs {
                    f4kvs_bytes_free(pair.key);
                    f4kvs_bytes_free(pair.value);
                }
                return e;
            }
        };
        let val_bytes = value_to_bytes(value);
        let value_len = val_bytes.len();
        let (value_ptr, _) = match allocate_bytes(val_bytes) {
            Ok(a) => a,
            Err(e) => {
                f4kvs_bytes_free(key_ptr);
                for pair in &pairs {
                    f4kvs_bytes_free(pair.key);
                    f4kvs_bytes_free(pair.value);
                }
                return e;
            }
        };
        pairs.push(F4KvsKVPairKv {
            key: key_ptr,
            key_len,
            value: value_ptr,
            value_len,
        });
    }
    let mut boxed = pairs.into_boxed_slice();
    let ptr = boxed.as_mut_ptr();
    std::mem::forget(boxed);
    (*result_out).pairs = ptr;
    (*result_out).count = count;
    F4KvsResult::Success
}

/// Fill up to `max` live prefix pairs after the last emitted key.
///
/// # Safety
/// `cur` from `cursor_open`. `result_out` must be valid.
#[no_mangle]
pub unsafe extern "C" fn f4kvs_engine_cursor_next_n(
    cur: *mut F4KvsCursor,
    max: usize,
    result_out: *mut F4KvsScanResult,
) -> F4KvsResult {
    if cur.is_null() || result_out.is_null() {
        set_last_error("Invalid argument: cursor or result_out is null");
        return F4KvsResult::ErrorInvalidArgument;
    }
    let cur = &mut *cur;
    if cur.done || max == 0 {
        (*result_out).pairs = ptr::null_mut();
        (*result_out).count = 0;
        return F4KvsResult::Success;
    }
    #[cfg(test)]
    if CURSOR_FAIL_NEXT.with(|f| f.replace(false)) {
        if cursor_ensure_state(cur).is_ok() {
            // Keep state; injected failure must not rewind.
        }
        set_last_error("Cursor next failed: injected");
        return F4KvsResult::ErrorStorage;
    }
    match cursor_pull_page(cur, max) {
        Ok((items, eof)) => {
            cur.done = eof;
            fill_scan_result(items, result_out)
        }
        Err(e) => e,
    }
}

/// Length-prefixed cursor page. Free with `f4kvs_scan_result_kv_free`.
///
/// # Safety
/// `cur` from `cursor_open`. `result_out` must be valid.
#[no_mangle]
pub unsafe extern "C" fn f4kvs_engine_cursor_next_n_kv(
    cur: *mut F4KvsCursor,
    max: usize,
    result_out: *mut F4KvsScanResultKv,
) -> F4KvsResult {
    if cur.is_null() || result_out.is_null() {
        set_last_error("Invalid argument: cursor or result_out is null");
        return F4KvsResult::ErrorInvalidArgument;
    }
    let cur = &mut *cur;
    if cur.done || max == 0 {
        (*result_out).pairs = ptr::null_mut();
        (*result_out).count = 0;
        return F4KvsResult::Success;
    }
    match cursor_pull_page(cur, max) {
        Ok((items, eof)) => {
            cur.done = eof;
            fill_scan_result_kv(items, result_out)
        }
        Err(e) => e,
    }
}

/// # Safety
/// `result` from `f4kvs_engine_cursor_next_n_kv`.
#[no_mangle]
pub unsafe extern "C" fn f4kvs_scan_result_kv_free(result: *mut F4KvsScanResultKv) {
    if result.is_null() {
        return;
    }
    let scan = &mut *result;
    if !scan.pairs.is_null() && scan.count > 0 {
        let pairs = std::slice::from_raw_parts_mut(scan.pairs, scan.count);
        for pair in pairs {
            f4kvs_bytes_free(pair.key);
            f4kvs_bytes_free(pair.value);
        }
        let _ = Box::from_raw(std::slice::from_raw_parts_mut(scan.pairs, scan.count));
    }
    scan.pairs = ptr::null_mut();
    scan.count = 0;
}

/// # Safety
/// `cur` from `cursor_open` or NULL.
#[no_mangle]
pub unsafe extern "C" fn f4kvs_engine_cursor_free(cur: *mut F4KvsCursor) {
    if !cur.is_null() {
        drop(Box::from_raw(cur));
    }
}

/// Visit every live prefix pair. Pointers passed to `cb` are valid only for
/// that call. Non-zero from `cb` stops the scan.
///
/// # Safety
/// `engine` live. `cb` must be valid for the duration of the call.
#[no_mangle]
pub unsafe extern "C" fn f4kvs_engine_scan_prefix_cb(
    engine: *mut F4KvsEngine,
    prefix: *const u8,
    prefix_len: usize,
    cb: Option<unsafe extern "C" fn(usize, *const u8, usize, *const u8, usize) -> c_int>,
    user: usize,
) -> F4KvsResult {
    let Some(cb) = cb else {
        set_last_error("Invalid argument: cb is null");
        return F4KvsResult::ErrorInvalidArgument;
    };
    let engine_ref = match validate_engine(engine) {
        Ok(e) => e,
        Err(e) => return e,
    };
    let prefix_str = match key_from_raw(prefix, prefix_len, "prefix") {
        Ok(s) => s,
        Err(e) => return e,
    };
    let visit = |k: &[u8], v: &[u8]| cb(user, k.as_ptr(), k.len(), v.as_ptr(), v.len()) == 0;
    match engine_ref
        .engine
        .try_prefix_scan_foreach_sync(prefix_str, visit)
    {
        Some(Ok(())) => F4KvsResult::Success,
        Some(Err(e)) => {
            set_last_error(&format!("Scan prefix cb failed: {}", e));
            F4KvsResult::ErrorStorage
        }
        None => {
            let mut st = match block_on(engine_ref.engine.prefix_scan_start(prefix_str)) {
                Ok(s) => s,
                Err(e) => {
                    set_last_error(&format!("Scan prefix cb start failed: {}", e));
                    return F4KvsResult::ErrorStorage;
                }
            };
            loop {
                match block_on(engine_ref.engine.prefix_scan_next(&mut st)) {
                    Ok(Some((k, v))) => {
                        let payload: &[u8] = match &v {
                            Value::Bytes(b) => b.as_slice(),
                            Value::String(s) => s.as_bytes(),
                            _ => continue,
                        };
                        if !visit(k.as_bytes(), payload) {
                            break;
                        }
                    }
                    Ok(None) => break,
                    Err(e) => {
                        set_last_error(&format!("Scan prefix cb failed: {}", e));
                        return F4KvsResult::ErrorStorage;
                    }
                }
            }
            F4KvsResult::Success
        }
    }
}

#[cfg(test)]
mod cursor_error_tests {
    use super::*;

    unsafe fn pair_key(result: &F4KvsScanResult, i: usize) -> String {
        let pair = &*result.pairs.add(i);
        CStr::from_ptr(pair.key).to_string_lossy().into_owned()
    }

    #[test]
    fn cursor_next_error_does_not_replay_keys() {
        unsafe {
            let engine = f4kvs_engine_new();
            assert!(!engine.is_null());
            for i in 0..10 {
                let k = format!("p/{i:02}");
                let v = format!("v{i}");
                assert_eq!(
                    f4kvs_engine_put_kv(engine, k.as_ptr(), k.len(), v.as_ptr(), v.len()),
                    F4KvsResult::Success
                );
            }
            let prefix = b"p/";
            let cur = f4kvs_engine_cursor_open(engine, prefix.as_ptr(), prefix.len());
            assert!(!cur.is_null());

            let mut result = std::mem::zeroed::<F4KvsScanResult>();
            assert_eq!(
                f4kvs_engine_cursor_next_n(cur, 3, &mut result),
                F4KvsResult::Success
            );
            assert_eq!(result.count, 3);
            let first_page: Vec<String> = (0..result.count).map(|i| pair_key(&result, i)).collect();
            assert_eq!(first_page, ["p/00", "p/01", "p/02"]);
            f4kvs_scan_result_free(&mut result);

            CURSOR_FAIL_NEXT.with(|f| f.set(true));
            assert_eq!(
                f4kvs_engine_cursor_next_n(cur, 100, &mut result),
                F4KvsResult::ErrorStorage
            );
            assert!(!(*cur).done);
            assert!((*cur).state.is_some());

            assert_eq!(
                f4kvs_engine_cursor_next_n(cur, 100, &mut result),
                F4KvsResult::Success
            );
            assert_eq!(result.count, 7);
            let rest: Vec<String> = (0..result.count).map(|i| pair_key(&result, i)).collect();
            assert_eq!(
                rest,
                ["p/03", "p/04", "p/05", "p/06", "p/07", "p/08", "p/09"]
            );
            f4kvs_scan_result_free(&mut result);
            f4kvs_engine_cursor_free(cur);
            f4kvs_engine_free(engine);
        }
    }
}

#[cfg(test)]
mod open_options_and_batch_cap_tests {
    use super::*;

    fn field_offset<T, F>(base: &T, field: &F) -> usize {
        let b = base as *const T as usize;
        let f = field as *const F as usize;
        f - b
    }

    #[test]
    fn versioned_open_options_ignore_fields_past_struct_size() {
        let mut opts = F4KvsOpenOptions::new();
        opts.wal_durability = 1;
        opts.memtable_max_size = 1_048_576;
        let cut = field_offset(&opts, &opts.memtable_max_size) as c_uint;
        opts.struct_size = cut;

        let read = unsafe { read_open_options(&opts) }.expect("read");
        assert_eq!(read.wal_durability, 1);
        assert_eq!(read.memtable_max_size, 0);

        let mut config = LsmConfig::default();
        let default_mem = config.memtable.max_size;
        apply_open_options(&mut config, Some(&read));
        assert!(config.wal.group_commit_enabled);
        assert_eq!(config.memtable.max_size, default_mem);
    }

    #[test]
    fn legacy_open_options_without_struct_size_still_apply() {
        let legacy = F4KvsOpenOptionsV0 {
            group_commit_enabled: 1,
            group_commit_max_wait_ms: 50,
            group_commit_max_batch_size: 0,
            group_commit_wait_durable: 0,
            wal_engine: 2,
            wal_durability: 0,
            group_commit_idle_flush_ms: 0,
            max_batch_size: 0,
            compaction_background: 0,
            max_sstables_per_level: 0,
            memtable_max_size: 0,
            sstable_target_size: 0,
            sstable_max_size: 0,
        };
        let first = unsafe { ptr::read_unaligned(&legacy as *const _ as *const u32) };
        assert!(first < MIN_VERSIONED_OPEN_OPTIONS);

        let read = unsafe { read_open_options(&legacy as *const _ as *const F4KvsOpenOptions) }
            .expect("legacy");
        assert_eq!(read.group_commit_enabled, 1);
        assert_eq!(read.group_commit_max_wait_ms, 50);
        assert_eq!(read.wal_engine, 2);
    }

    #[test]
    fn batch_put_kv_rejects_count_over_max_before_alloc() {
        unsafe {
            let engine = f4kvs_engine_new();
            assert!(!engine.is_null());
            let rc = f4kvs_engine_batch_put_kv(
                engine,
                ptr::null(),
                ptr::null(),
                ptr::null(),
                ptr::null(),
                1_000_000_000,
            );
            assert_eq!(rc, F4KvsResult::ErrorInvalidArgument);
            f4kvs_engine_free(engine);
        }
    }
}
