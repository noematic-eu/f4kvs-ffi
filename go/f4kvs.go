//go:build cgo

package f4kvs

/*
#cgo CFLAGS: -I${SRCDIR}/../crates/f4kvs-ffi/include
#cgo LDFLAGS: -L${SRCDIR}/../target/release -lf4kvs_ffi

#include <stdlib.h>
#include "f4kvs.h"
*/
import "C"

import (
	"fmt"
	"sync"
	"unsafe"
)

// F4KVS wraps the f4kvs-ffi LSM engine.
type F4KVS struct {
	handle *C.F4KvsEngine
	mu     sync.Mutex
	closed bool
}

// NewMemoryEngine opens an ephemeral engine in a temporary directory.
func NewMemoryEngine() (*F4KVS, error) {
	handle := C.f4kvs_engine_new()
	if handle == nil {
		return nil, fmt.Errorf("f4kvs memory engine: %s", lastError())
	}
	return &F4KVS{handle: handle}, nil
}

// NewPersistentEngine opens a persistent engine at path.
func NewPersistentEngine(path string) (*F4KVS, error) {
	cpath := C.CString(path)
	defer C.free(unsafe.Pointer(cpath))

	handle := C.f4kvs_engine_open(cpath)
	if handle == nil {
		return nil, fmt.Errorf("f4kvs open %s: %s", path, lastError())
	}
	return &F4KVS{handle: handle}, nil
}

// BeginTransaction starts a new staged transaction.
func (e *F4KVS) BeginTransaction() *Transaction {
	return &Transaction{engine: e}
}

func (e *F4KVS) Get(key string) (string, error) {
	value, err := e.GetBytes(key)
	if err != nil {
		return "", err
	}
	return string(value), nil
}

func (e *F4KVS) Put(key, value string) error {
	return e.PutBytes(key, []byte(value))
}

func (e *F4KVS) GetBytes(key string) ([]byte, error) {
	e.mu.Lock()
	defer e.mu.Unlock()
	if e.closed || e.handle == nil {
		return nil, ErrClosed
	}

	ckey := C.CString(key)
	defer C.free(unsafe.Pointer(ckey))

	var out *C.uint8_t
	var outLen C.size_t
	res := C.f4kvs_engine_get_bytes(e.handle, ckey, &out, &outLen)
	if res == C.F4KVS_ERROR_NOT_FOUND {
		return nil, ErrNotFound
	}
	if res != C.F4KVS_SUCCESS {
		return nil, fmt.Errorf("f4kvs get %q: %s", key, lastError())
	}
	defer C.f4kvs_bytes_free(out)

	if out == nil || outLen == 0 {
		return []byte{}, nil
	}
	return C.GoBytes(unsafe.Pointer(out), C.int(outLen)), nil
}

func (e *F4KVS) PutBytes(key string, value []byte) error {
	e.mu.Lock()
	defer e.mu.Unlock()
	if e.closed || e.handle == nil {
		return ErrClosed
	}

	ckey := C.CString(key)
	defer C.free(unsafe.Pointer(ckey))

	var ptr *C.uint8_t
	if len(value) > 0 {
		ptr = (*C.uint8_t)(unsafe.Pointer(&value[0]))
	}
	res := C.f4kvs_engine_put_bytes(e.handle, ckey, ptr, C.size_t(len(value)))
	if res != C.F4KVS_SUCCESS {
		return fmt.Errorf("f4kvs put %q: %s", key, lastError())
	}
	return nil
}

func (e *F4KVS) Delete(key string) error {
	e.mu.Lock()
	defer e.mu.Unlock()
	if e.closed || e.handle == nil {
		return ErrClosed
	}

	ckey := C.CString(key)
	defer C.free(unsafe.Pointer(ckey))

	res := C.f4kvs_engine_delete(e.handle, ckey)
	if res == C.F4KVS_ERROR_NOT_FOUND {
		return nil
	}
	if res != C.F4KVS_SUCCESS {
		return fmt.Errorf("f4kvs delete %q: %s", key, lastError())
	}
	return nil
}

func (e *F4KVS) GetAllKeys() []string {
	e.mu.Lock()
	defer e.mu.Unlock()
	if e.closed || e.handle == nil {
		return nil
	}
	return scanPrefixLocked(e, "")
}

func (e *F4KVS) BatchPut(items map[string]string) error {
	for key, value := range items {
		if err := e.Put(key, value); err != nil {
			return err
		}
	}
	return nil
}

func (e *F4KVS) BatchGetValues(keys []string) (map[string]string, error) {
	result := make(map[string]string, len(keys))
	for _, key := range keys {
		value, err := e.Get(key)
		if err != nil {
			return nil, err
		}
		result[key] = value
	}
	return result, nil
}

func (e *F4KVS) BatchGetBytes(keys []string) (map[string][]byte, error) {
	result := make(map[string][]byte, len(keys))
	for _, key := range keys {
		value, err := e.GetBytes(key)
		if err != nil {
			return nil, err
		}
		result[key] = value
	}
	return result, nil
}

func (e *F4KVS) BatchPutBytes(items map[string][]byte) error {
	for key, value := range items {
		if err := e.PutBytes(key, value); err != nil {
			return err
		}
	}
	return nil
}

func (e *F4KVS) BatchDelete(keys []string) error {
	if len(keys) == 0 {
		return nil
	}

	e.mu.Lock()
	defer e.mu.Unlock()
	if e.closed || e.handle == nil {
		return ErrClosed
	}

	cKeys := make([]*C.char, len(keys))
	for i, key := range keys {
		cKeys[i] = C.CString(key)
	}
	defer func() {
		for _, ck := range cKeys {
			C.free(unsafe.Pointer(ck))
		}
	}()

	res := C.f4kvs_engine_batch_delete(e.handle, (**C.char)(unsafe.Pointer(&cKeys[0])), C.size_t(len(keys)))
	if res != C.F4KVS_SUCCESS {
		return fmt.Errorf("f4kvs batch delete: %s", lastError())
	}
	return nil
}

func (e *F4KVS) Sync() error {
	e.mu.Lock()
	defer e.mu.Unlock()
	if e.closed || e.handle == nil {
		return ErrClosed
	}
	res := C.f4kvs_engine_compact(e.handle)
	if res != C.F4KVS_SUCCESS {
		return fmt.Errorf("f4kvs sync: %s", lastError())
	}
	return nil
}

func (e *F4KVS) Close() {
	e.mu.Lock()
	defer e.mu.Unlock()
	if e.handle == nil {
		return
	}
	_ = C.f4kvs_engine_close(e.handle)
	C.f4kvs_engine_free(e.handle)
	e.handle = nil
	e.closed = true
}

func scanPrefixLocked(e *F4KVS, prefix string) []string {
	cprefix := C.CString(prefix)
	defer C.free(unsafe.Pointer(cprefix))

	var result C.F4KvsScanResult
	res := C.f4kvs_engine_scan_prefix(e.handle, cprefix, &result)
	if res != C.F4KVS_SUCCESS {
		return nil
	}
	defer C.f4kvs_scan_result_free(&result)

	keys := make([]string, 0, int(result.count))
	for i := 0; i < int(result.count); i++ {
		pair := (*[1 << 30]C.F4KvsKVPair)(unsafe.Pointer(result.pairs))[i]
		keys = append(keys, C.GoString(pair.key))
	}
	return keys
}

func lastError() string {
	msg := C.f4kvs_get_last_error()
	if msg == nil {
		return "unknown error"
	}
	return C.GoString(msg)
}