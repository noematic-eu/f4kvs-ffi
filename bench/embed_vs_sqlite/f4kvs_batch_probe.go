//go:build ignore

package main

import (
	"fmt"
	"os"
	"time"

	f4kvs "github.com/noematic-eu/f4kvs-go"
)

func main() {
	n := 2000
	payload := make([]byte, 4096)
	for i := range payload {
		payload[i] = 'x'
	}
	keys := make([]string, n)
	items := make(map[string][]byte, n)
	for i := 0; i < n; i++ {
		k := fmt.Sprintf("chunk:legal:doc-%04d:chunk-%06d", i/10, i)
		keys[i] = k
		items[k] = payload
	}

	dir, err := os.MkdirTemp("", "f4kvs-probe-*")
	if err != nil {
		panic(err)
	}
	defer os.RemoveAll(dir)

	engine, err := f4kvs.NewPersistentEngine(dir)
	if err != nil {
		panic(err)
	}
	defer engine.Close()

	t0 := time.Now()
	for _, key := range keys {
		if err := engine.PutBytes(key, payload); err != nil {
			panic(err)
		}
	}
	fmt.Printf("put_bytes per-commit: %.1f ms (%.3f ms/op)\n", msSince(t0), msSince(t0)/float64(n))

	engine2, err := f4kvs.NewPersistentEngine(dir + "-batch")
	if err != nil {
		panic(err)
	}
	defer engine2.Close()

	t0 = time.Now()
	if err := engine2.BatchPutBytes(items); err != nil {
		panic(err)
	}
	fmt.Printf("batch_put_bytes:      %.1f ms (%.3f ms/op)\n", msSince(t0), msSince(t0)/float64(n))
}

func msSince(t time.Time) float64 {
	return time.Since(t).Seconds() * 1000
}