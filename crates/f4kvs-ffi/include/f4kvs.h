#ifndef F4KVS_H
#define F4KVS_H

/**
 * @file f4kvs.h
 * @brief C ABI for the F4KVS embedded LSM key-value store.
 *
 * Overview
 * --------
 * This library exposes a persistent LSM-backed key-value engine to C and other
 * languages via FFI. Keys are null-terminated UTF-8 strings. String and binary
 * value variants are supported.
 *
 * Thread safety
 * -------------
 * A single engine handle may be used concurrently from multiple threads for
 * read/write operations. Error messages are stored in a global buffer protected
 * by a mutex; call f4kvs_get_last_error() immediately after a failed call.
 *
 * Engine lifecycle
 * ----------------
 * - f4kvs_engine_new()  — ephemeral engine in a temporary data directory.
 * - f4kvs_engine_open() — persistent engine at the given data_dir.
 * - f4kvs_engine_close() — flush pending writes and shut down the engine.
 * - f4kvs_engine_free() — release the engine handle (call after close).
 *
 * Memory ownership
 * ----------------
 * Buffers returned by get/scan functions are allocated by the library and must
 * be freed by the caller:
 * - f4kvs_string_free()  — strings from f4kvs_engine_get().
 * - f4kvs_bytes_free()   — byte buffers from f4kvs_engine_get_bytes() and scan pairs.
 * - f4kvs_scan_result_free() — entire scan results from f4kvs_engine_scan_prefix().
 *
 * Do not free pointers that were not allocated by this library. Double-free is
 * prevented but silently ignored for unknown pointers.
 *
 * Limits
 * ------
 * - Maximum key length: 1 MB (1,048,576 bytes, excluding the null terminator).
 * - Maximum value length: 100 MB (104,857,600 bytes).
 *
 * Error handling
 * --------------
 * Functions return F4KvsResult. On failure, call f4kvs_get_last_error() for a
 * human-readable message. f4kvs_result_to_string() maps codes to static strings.
 */

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/** @brief Result codes returned by FFI functions. */
typedef enum {
    F4KVS_SUCCESS = 0,
    F4KVS_ERROR_INVALID_ARGUMENT = 1,
    F4KVS_ERROR_NOT_FOUND = 2,
    F4KVS_ERROR_STORAGE = 3,
    F4KVS_ERROR_NETWORK = 4,
    F4KVS_ERROR_TIMEOUT = 5,
    F4KVS_ERROR_UNKNOWN = 99
} F4KvsResult;

/** @brief Opaque handle to an LSM-backed F4KVS engine. */
typedef struct F4KvsEngine F4KvsEngine;

/** @brief Key-value pair returned by prefix scans. */
typedef struct {
    char *key;          /**< Allocated by the library; free with f4kvs_string_free(). */
    uint8_t *value;     /**< Allocated by the library; free with f4kvs_bytes_free(). */
    size_t value_len;
} F4KvsKVPair;

/** @brief Container for scan results. */
typedef struct {
    F4KvsKVPair *pairs; /**< Array of count pairs; free with f4kvs_scan_result_free(). */
    size_t count;
} F4KvsScanResult;

/**
 * Create a new engine in a temporary data directory.
 * @return Engine handle, or NULL on failure (see f4kvs_get_last_error()).
 *         Free with f4kvs_engine_free() after f4kvs_engine_close().
 */
F4KvsEngine *f4kvs_engine_new(void);

/**
 * Open a persistent engine at data_dir.
 * @param data_dir Null-terminated UTF-8 path. Must not be NULL or empty.
 * @return Engine handle, or NULL on failure.
 */
F4KvsEngine *f4kvs_engine_open(const char *data_dir);

/**
 * Flush pending writes and shut down the engine.
 * @param engine Valid engine handle.
 */
F4KvsResult f4kvs_engine_close(F4KvsEngine *engine);

/**
 * Release the engine handle. Does not delete on-disk data.
 * @param engine Engine handle (may be NULL).
 */
void f4kvs_engine_free(F4KvsEngine *engine);

/**
 * Compact on-disk LSM data.
 * @param engine Valid engine handle.
 */
F4KvsResult f4kvs_engine_compact(F4KvsEngine *engine);

/**
 * Store a string key-value pair.
 * @param key    Null-terminated UTF-8 key (≤ 1 MB).
 * @param value  Null-terminated UTF-8 value (≤ 100 MB).
 */
F4KvsResult f4kvs_engine_put(F4KvsEngine *engine, const char *key, const char *value);

/**
 * Retrieve a string value by key.
 * @param value_out Output pointer; set to an allocated string on success.
 *                  Caller must free with f4kvs_string_free().
 *                  Set to NULL when the key is not found (F4KVS_ERROR_NOT_FOUND).
 */
F4KvsResult f4kvs_engine_get(F4KvsEngine *engine, const char *key, char **value_out);

/**
 * Delete a key.
 */
F4KvsResult f4kvs_engine_delete(F4KvsEngine *engine, const char *key);

/**
 * Delete multiple keys in one call.
 * @param keys  Array of count null-terminated key strings.
 * @param count Number of keys (0 is allowed).
 */
F4KvsResult f4kvs_engine_batch_delete(F4KvsEngine *engine, const char **keys, size_t count);

/**
 * Check whether a key exists.
 * @param exists_out Set to 1 if the key exists, 0 otherwise.
 */
F4KvsResult f4kvs_engine_exists(F4KvsEngine *engine, const char *key, int *exists_out);

/**
 * Store a binary value under a string key.
 * @param value     Pointer to value_len bytes (may be NULL when value_len is 0).
 * @param value_len Length in bytes (≤ 100 MB).
 */
F4KvsResult f4kvs_engine_put_bytes(F4KvsEngine *engine, const char *key, const uint8_t *value,
                                   size_t value_len);

/**
 * Retrieve a binary value by key.
 * @param value_out     Output pointer; allocated on success, free with f4kvs_bytes_free().
 * @param value_len_out Output length in bytes.
 */
F4KvsResult f4kvs_engine_get_bytes(F4KvsEngine *engine, const char *key, uint8_t **value_out,
                                   size_t *value_len_out);

/** Free a byte buffer allocated by this library. Safe to call with NULL. */
void f4kvs_bytes_free(uint8_t *ptr);

/**
 * Scan all keys with the given prefix and return matching key-value pairs.
 * @param result_out Filled on success; free with f4kvs_scan_result_free().
 */
F4KvsResult f4kvs_engine_scan_prefix(F4KvsEngine *engine, const char *prefix,
                                     F4KvsScanResult *result_out);

/** Free a scan result and all pairs allocated by f4kvs_engine_scan_prefix(). */
void f4kvs_scan_result_free(F4KvsScanResult *result);

/**
 * Get the last error message set by the library.
 * @return Pointer to a null-terminated string, or NULL if no error was recorded.
 *         The pointer is valid until the next failed call overwrites it.
 */
const char *f4kvs_get_last_error(void);

/** Map a result code to a static description string. Do not free the return value. */
const char *f4kvs_result_to_string(F4KvsResult result);

/** Free a string allocated by this library. Safe to call with NULL. */
void f4kvs_string_free(char *ptr);

#ifdef __cplusplus
}
#endif

#endif /* F4KVS_H */
