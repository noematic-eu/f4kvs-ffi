//go:build cgo

package f4kvs_test

import (
	"fmt"
	"sync"
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

func TestConcurrentGetsSameEngine(t *testing.T) {
	engine, err := f4kvs.NewMemoryEngine()
	if err != nil {
		t.Fatal(err)
	}
	defer engine.Close()

	const n = 64
	for i := 0; i < n; i++ {
		key := fmt.Sprintf("k/%02d", i)
		if err := engine.Put(key, "v"); err != nil {
			t.Fatal(err)
		}
	}

	var wg sync.WaitGroup
	errCh := make(chan error, n)
	for i := 0; i < n; i++ {
		wg.Add(1)
		i := i
		go func() {
			defer wg.Done()
			key := fmt.Sprintf("k/%02d", i)
			got, err := engine.Get(key)
			if err != nil {
				errCh <- err
				return
			}
			if got != "v" {
				errCh <- fmt.Errorf("get %q = %q", key, got)
			}
		}()
	}
	wg.Wait()
	close(errCh)
	for err := range errCh {
		t.Fatal(err)
	}

	engine.Close()
	if _, err := engine.Get("k/00"); err != f4kvs.ErrClosed {
		t.Fatalf("get after close: %v", err)
	}
}