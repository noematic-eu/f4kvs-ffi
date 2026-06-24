//go:build cgo

package f4kvs_test

import (
	"testing"

	f4kvs "github.com/noematic-eu/f4kvs-go"
)

func TestBatchPutBytesAndTransactionCommit(t *testing.T) {
	engine, err := f4kvs.NewMemoryEngine()
	if err != nil {
		t.Fatal(err)
	}
	defer engine.Close()

	items := map[string][]byte{
		"chunk:doc-0001:chunk-000001": []byte("payload-a"),
		"chunk:doc-0001:chunk-000002": []byte("payload-b"),
	}
	if err := engine.BatchPutBytes(items); err != nil {
		t.Fatalf("batch put: %v", err)
	}
	for key, want := range items {
		got, err := engine.GetBytes(key)
		if err != nil {
			t.Fatalf("get %q: %v", key, err)
		}
		if string(got) != string(want) {
			t.Fatalf("get %q = %q, want %q", key, got, want)
		}
	}

	txn := engine.BeginTransaction()
	if err := txn.PutBytes("chunk:doc-0002:chunk-000001", []byte("txn-payload")); err != nil {
		t.Fatal(err)
	}
	if err := txn.Delete("chunk:doc-0001:chunk-000001"); err != nil {
		t.Fatal(err)
	}
	if err := txn.Commit(); err != nil {
		t.Fatalf("commit: %v", err)
	}

	if _, err := engine.GetBytes("chunk:doc-0001:chunk-000001"); err != f4kvs.ErrNotFound {
		t.Fatalf("deleted key still present: %v", err)
	}
	got, err := engine.GetBytes("chunk:doc-0002:chunk-000001")
	if err != nil {
		t.Fatal(err)
	}
	if string(got) != "txn-payload" {
		t.Fatalf("txn put = %q", got)
	}
}