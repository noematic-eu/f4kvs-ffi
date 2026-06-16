//go:build cgo

package f4kvs

import "fmt"

type txnOpKind int

const (
	opPut txnOpKind = iota
	opDelete
)

type txnOp struct {
	kind  txnOpKind
	value []byte
}

// Transaction stages writes until Commit or Rollback.
type Transaction struct {
	engine     *F4KVS
	ops        map[string]txnOp
	committed  bool
	rolledBack bool
}

func (t *Transaction) Get(key string) (string, error) {
	value, err := t.GetBytes(key)
	if err != nil {
		return "", err
	}
	return string(value), nil
}

func (t *Transaction) Put(key, value string) error {
	return t.PutBytes(key, []byte(value))
}

func (t *Transaction) GetBytes(key string) ([]byte, error) {
	if err := t.checkActive(); err != nil {
		return nil, err
	}
	if op, ok := t.ops[key]; ok {
		if op.kind == opDelete {
			return nil, ErrNotFound
		}
		return append([]byte(nil), op.value...), nil
	}
	return t.engine.GetBytes(key)
}

func (t *Transaction) PutBytes(key string, value []byte) error {
	if err := t.checkActive(); err != nil {
		return err
	}
	if t.ops == nil {
		t.ops = make(map[string]txnOp)
	}
	t.ops[key] = txnOp{kind: opPut, value: append([]byte(nil), value...)}
	return nil
}

func (t *Transaction) Delete(key string) error {
	if err := t.checkActive(); err != nil {
		return err
	}
	if t.ops == nil {
		t.ops = make(map[string]txnOp)
	}
	t.ops[key] = txnOp{kind: opDelete}
	return nil
}

func (t *Transaction) Commit() error {
	if t.committed {
		return ErrTxnCommitted
	}
	if t.rolledBack {
		return ErrTxnRolledBack
	}

	for key, op := range t.ops {
		switch op.kind {
		case opPut:
			if err := t.engine.PutBytes(key, op.value); err != nil {
				return fmt.Errorf("commit put %q: %w", key, err)
			}
		case opDelete:
			if err := t.engine.Delete(key); err != nil {
				return fmt.Errorf("commit delete %q: %w", key, err)
			}
		}
	}

	t.committed = true
	t.ops = nil
	return nil
}

func (t *Transaction) Rollback() error {
	if t.committed {
		return ErrTxnCommitted
	}
	if t.rolledBack {
		return nil
	}
	t.rolledBack = true
	t.ops = nil
	return nil
}

func (t *Transaction) checkActive() error {
	if t.committed {
		return ErrTxnCommitted
	}
	if t.rolledBack {
		return ErrTxnRolledBack
	}
	return nil
}