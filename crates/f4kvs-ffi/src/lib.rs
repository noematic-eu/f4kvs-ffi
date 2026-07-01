//! FFI bindings for F4KVS with persistent LSM storage.

#![allow(unsafe_op_in_unsafe_fn)]

use f4kvs_lsm::core::config::{WalDurability, WalEngine, WalSyncMode};
use f4kvs_lsm::{LsmConfig, LsmTreeEngine};
use f4kvs_storage_core::traits::StorageEngine;
use f4kvs_value::Value;
use std::collections::{HashMap, HashSet};
use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int, c_uint, c_uchar};
use std::time::Duration;
use std::path::PathBuf;
use std::ptr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use tokio::runtime::Runtime;

/// Maximum key length in bytes (1MB default)
const MAX_KEY_LENGTH: usize = 1 * 1024 * 1024;

/// Maximum value length in bytes (100MB default)
const MAX_VALUE_LENGTH: usize = 100 * 1024 * 1024;

static ENGINE_COUNTER: AtomicU64 = AtomicU64::new(0);
static RUNTIME: OnceLock<Runtime> = OnceLock::new();

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

/// Opaque handle to an LSM-backed F4KVS engine.
pub struct F4KvsEngine {
    engine: Arc<LsmTreeEngine>,
}

fn runtime() -> &'static Runtime {
    RUNTIME.get_or_init(|| {
        Runtime::new().expect("failed to create Tokio runtime for f4kvs-ffi")
    })
}

fn unique_data_dir() -> PathBuf {
    let id = ENGINE_COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("f4kvs_ffi_{}_{}", std::process::id(), id))
}

/// FFI mirror of `F4KvsOpenOptions` from f4kvs.h
#[repr(C)]
pub struct F4KvsOpenOptions {
    pub group_commit_enabled: c_uchar,
    pub group_commit_max_wait_ms: c_uint,
    pub group_commit_max_batch_size: c_uint,
    pub group_commit_wait_durable: c_uchar,
    pub wal_engine: c_uchar,
    pub wal_durability: c_uchar,
    pub group_commit_idle_flush_ms: c_uint,
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
        config.wal.group_commit_idle_flush =
            Some(Duration::from_millis(options.group_commit_idle_flush_ms as u64));
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

    let engine = runtime()
        .block_on(LsmTreeEngine::new(config))
        .map_err(|e| {
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

static mut GLOBAL_ERROR_MESSAGE: Option<&'static CStr> = None;
static GLOBAL_ERROR_LOCK: Mutex<()> = Mutex::new(());

fn set_global_error(msg: &str) {
    unsafe {
        let _guard = GLOBAL_ERROR_LOCK.lock().unwrap();
        GLOBAL_ERROR_MESSAGE = None;

        match CString::new(msg) {
            Ok(cstring) => {
                let bytes_with_nul = cstring.into_bytes_with_nul();
                let boxed_bytes = bytes_with_nul.into_boxed_slice();
                let leaked_bytes: &'static [u8] = Box::leak(boxed_bytes);
                let cstr_ref = CStr::from_bytes_with_nul_unchecked(leaked_bytes);
                GLOBAL_ERROR_MESSAGE = Some(cstr_ref);
            }
            Err(_) => {
                let safe_msg = msg.replace('\0', "\\0");
                if let Ok(cstring) = CString::new(safe_msg) {
                    let bytes_with_nul = cstring.into_bytes_with_nul();
                    let boxed_bytes = bytes_with_nul.into_boxed_slice();
                    let leaked_bytes: &'static [u8] = Box::leak(boxed_bytes);
                    let cstr_ref = CStr::from_bytes_with_nul_unchecked(leaked_bytes);
                    GLOBAL_ERROR_MESSAGE = Some(cstr_ref);
                }
            }
        }
    }
}

fn get_global_error_ptr() -> *const c_char {
    unsafe {
        let _guard = GLOBAL_ERROR_LOCK.lock().unwrap();
        match GLOBAL_ERROR_MESSAGE {
            Some(cstr) => cstr.as_ptr(),
            None => ptr::null(),
        }
    }
}

fn set_last_error(msg: &str) {
    set_global_error(msg);
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
        self.allocations.lock().unwrap().insert(ptr as usize, len).is_none()
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

    let opts = if options.is_null() {
        None
    } else {
        Some(&*options)
    };

    match open_lsm_engine(path, opts) {
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

    match runtime().block_on(engine_ref.engine.shutdown()) {
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

    match runtime().block_on(engine_ref.engine.compact()) {
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
    engine_ref
        .engine
        .set_bulk_import(enabled != 0);
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

    match runtime().block_on(engine_ref.engine.flush()) {
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

    match runtime().block_on(engine_ref.engine.flush_wal()) {
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

    match runtime().block_on(engine_ref.engine.scan_prefix(&prefix_str)) {
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

    match runtime().block_on(
        engine_ref
            .engine
            .put(&key_str, &Value::String(value_str)),
    ) {
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

    match runtime().block_on(engine_ref.engine.put(&key_str, &Value::Bytes(bytes))) {
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

    match runtime().block_on(engine_ref.engine.batch_put(items)) {
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

    match runtime().block_on(engine_ref.engine.get(&key_str)) {
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

    match runtime().block_on(engine_ref.engine.get(&key_str)) {
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

    match runtime().block_on(engine_ref.engine.delete(&key_str)) {
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

    match runtime().block_on(engine_ref.engine.batch_delete(key_strings)) {
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

    match runtime().block_on(engine_ref.engine.exists(&key_str)) {
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

    match runtime().block_on(engine_ref.engine.scan_prefix_with_values(&prefix_str)) {
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
    get_global_error_ptr()
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
