mod common;

use common::{from_c_string, to_c_string};
use f4kvs_ffi::*;
use std::os::raw::c_char;

fn put_bytes(engine: *mut F4KvsEngine, key: &str, value: &[u8]) {
    unsafe {
        let key_c = to_c_string(key);
        let ptr = value.as_ptr() as *const u8;
        let result = f4kvs_engine_put_bytes(engine, key_c.as_ptr(), ptr, value.len());
        assert_eq!(result, F4KvsResult::Success, "put_bytes failed for {}", key);
    }
}

fn get_bytes(engine: *mut F4KvsEngine, key: &str) -> Vec<u8> {
    unsafe {
        let key_c = to_c_string(key);
        let mut out: *mut u8 = std::ptr::null_mut();
        let mut out_len: usize = 0;
        let result = f4kvs_engine_get_bytes(engine, key_c.as_ptr(), &mut out, &mut out_len);
        assert_eq!(result, F4KvsResult::Success, "get_bytes failed for {}", key);
        let bytes = std::slice::from_raw_parts(out, out_len).to_vec();
        f4kvs_bytes_free(out);
        bytes
    }
}

#[test]
fn test_lexical_key_patterns() {
    unsafe {
        let engine = f4kvs_engine_new();
        assert!(!engine.is_null());

        put_bytes(
            engine,
            "chunk:contract-1",
            br#"{"id":"contract-1","fields":{"body":"liability"}}"#,
        );
        put_bytes(engine, "lex:df:liability", &[0, 0, 0, 1]);
        put_bytes(engine, "lex:post:liability", &[0, 0, 0, 1, 0, 11, b'c', b'o', b'n', b't', b'r', b'a', b'c', b't', b'-', b'1', 1, 4, b'b', b'o', b'd', b'y', 0, 0, 0, 1]);

        assert_eq!(
            get_bytes(engine, "chunk:contract-1"),
            br#"{"id":"contract-1","fields":{"body":"liability"}}"#
        );
        assert_eq!(get_bytes(engine, "lex:df:liability"), vec![0, 0, 0, 1]);

        let meta_key = to_c_string("lex:meta");
        let meta_val = to_c_string(r#"{"n":1,"avg_dl":5.0,"version":1}"#);
        let result = f4kvs_engine_put(engine, meta_key.as_ptr(), meta_val.as_ptr());
        assert_eq!(result, F4KvsResult::Success);

        let prefix = to_c_string("lex:");
        let mut scan = F4KvsScanResult {
            pairs: std::ptr::null_mut(),
            count: 0,
        };
        let result = f4kvs_engine_scan_prefix(engine, prefix.as_ptr(), &mut scan);
        assert_eq!(result, F4KvsResult::Success);
        assert!(scan.count >= 3);
        f4kvs_scan_result_free(&mut scan);

        let del_key = to_c_string("lex:df:liability");
        let result = f4kvs_engine_delete(engine, del_key.as_ptr());
        assert_eq!(result, F4KvsResult::Success);

        let mut value_out: *mut c_char = std::ptr::null_mut();
        let result = f4kvs_engine_get(engine, del_key.as_ptr(), &mut value_out);
        assert_eq!(result, F4KvsResult::ErrorNotFound);

        let batch_keys = [
            to_c_string("lex:post:liability"),
            to_c_string("lex:meta"),
        ];
        let key_ptrs: Vec<*const c_char> = batch_keys.iter().map(|k| k.as_ptr()).collect();
        let result = f4kvs_engine_batch_delete(
            engine,
            key_ptrs.as_ptr(),
            batch_keys.len(),
        );
        assert_eq!(result, F4KvsResult::Success);

        let chunk_key = to_c_string("chunk:contract-1");
        let result = f4kvs_engine_get(engine, chunk_key.as_ptr(), &mut value_out);
        assert_eq!(result, F4KvsResult::Success);
        let chunk_json = from_c_string(value_out);
        assert!(chunk_json.contains("contract-1"));
        f4kvs_string_free(value_out);

        f4kvs_engine_free(engine);
    }
}

#[test]
fn test_batch_delete_empty() {
    unsafe {
        let engine = f4kvs_engine_new();
        let result = f4kvs_engine_batch_delete(engine, std::ptr::null(), 0);
        assert_eq!(result, F4KvsResult::Success);
        f4kvs_engine_free(engine);
    }
}
