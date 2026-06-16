//go:build !cgo

package f4kvs

import "errors"

var ErrCGORequired = errors.New("f4kvs requires cgo")

type F4KVS struct{}

type Transaction struct{}

func NewMemoryEngine() (*F4KVS, error)        { return nil, ErrCGORequired }
func NewPersistentEngine(string) (*F4KVS, error) { return nil, ErrCGORequired }

func (e *F4KVS) BeginTransaction() *Transaction                     { return nil }
func (e *F4KVS) Get(string) (string, error)                         { return "", ErrCGORequired }
func (e *F4KVS) Put(string, string) error                           { return ErrCGORequired }
func (e *F4KVS) GetBytes(string) ([]byte, error)                    { return nil, ErrCGORequired }
func (e *F4KVS) PutBytes(string, []byte) error                      { return ErrCGORequired }
func (e *F4KVS) Delete(string) error                                  { return ErrCGORequired }
func (e *F4KVS) GetAllKeys() []string                                 { return nil }
func (e *F4KVS) BatchPut(map[string]string) error                     { return ErrCGORequired }
func (e *F4KVS) BatchGetValues([]string) (map[string]string, error)   { return nil, ErrCGORequired }
func (e *F4KVS) BatchGetBytes([]string) (map[string][]byte, error)     { return nil, ErrCGORequired }
func (e *F4KVS) BatchPutBytes(map[string][]byte) error                { return ErrCGORequired }
func (e *F4KVS) BatchDelete([]string) error                           { return ErrCGORequired }
func (e *F4KVS) Sync() error                                          { return ErrCGORequired }
func (e *F4KVS) Close()                                               {}

func (t *Transaction) Get(string) (string, error)      { return "", ErrCGORequired }
func (t *Transaction) Put(string, string) error        { return ErrCGORequired }
func (t *Transaction) GetBytes(string) ([]byte, error) { return nil, ErrCGORequired }
func (t *Transaction) PutBytes(string, []byte) error { return ErrCGORequired }
func (t *Transaction) Delete(string) error             { return ErrCGORequired }
func (t *Transaction) Commit() error                   { return ErrCGORequired }
func (t *Transaction) Rollback() error                 { return ErrCGORequired }