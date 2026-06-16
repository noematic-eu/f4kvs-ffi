package f4kvs

import "errors"

var (
	ErrNotFound     = errors.New("key not found")
	ErrClosed       = errors.New("engine is closed")
	ErrTxnCommitted = errors.New("transaction already committed")
	ErrTxnRolledBack = errors.New("transaction already rolled back")
)