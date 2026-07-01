// FFI read tax: compare Go GetBytes (CGO) vs native Rust get on the same on-disk store.
//
// Workflow:
//   1. Go prepares DB (BatchPutBytes, optional bulk import) and runs FFI random reads.
//   2. Close engine; native Rust example opens the same dir and runs matching reads.
package main

import (
	"encoding/json"
	"flag"
	"fmt"
	"os"
	"runtime"
	"time"

	f4kvs "github.com/noematic-eu/f4kvs-go"
)

type report struct {
	Host        string  `json:"host"`
	Dir         string  `json:"dir"`
	Chunks      int     `json:"chunks"`
	ChunkBytes  int     `json:"chunk_bytes"`
	RandomGets  int     `json:"random_gets"`
	BulkImport  bool    `json:"bulk_import"`
	IngestMs    float64 `json:"ingest_ms"`
	FFIReadMs   float64 `json:"ffi_read_ms"`
	FFIReadOpsS float64 `json:"ffi_read_ops_per_s"`
}

func main() {
	dir := flag.String("dir", "", "persistent engine dir (created if empty)")
	chunks := flag.Int("chunks", 2000, "chunk count to load")
	chunkBytes := flag.Int("chunk-bytes", 4096, "chunk payload size")
	randomGets := flag.Int("random-gets", 500, "random point reads")
	bulkImport := flag.Bool("bulk-import", false, "enable SetBulkImport before BatchPutBytes")
	prepareOnly := flag.Bool("prepare-only", false, "ingest only; skip FFI reads")
	readOnly := flag.Bool("read-only", false, "FFI reads only on existing dir (skip ingest)")
	out := flag.String("out", "", "optional JSON report path")
	flag.Parse()

	payload := samplePayload(*chunkBytes)
	chunkKeys := make([]string, *chunks)
	items := make(map[string][]byte, *chunks)
	for i := range chunkKeys {
		key := fmt.Sprintf("chunk:legal:doc-%04d:chunk-%06d", i/10, i)
		chunkKeys[i] = key
		items[key] = payload
	}

	storeDir := *dir
	if storeDir == "" {
		tmp, err := os.MkdirTemp("", "f4kvs-read-tax-*")
		if err != nil {
			fatal(err)
		}
		storeDir = tmp
	} else if err := os.MkdirAll(storeDir, 0o755); err != nil {
		fatal(err)
	}

	var ingestMs float64
	var ffiMs float64

	if !*readOnly {
		engine, err := f4kvs.NewPersistentEngine(storeDir)
		if err != nil {
			fatal(err)
		}

		if *bulkImport {
			if err := engine.SetBulkImport(true); err != nil {
				fatal(err)
			}
		}

		fmt.Fprintf(os.Stderr, "ingest %d chunks (bulk_import=%v)...\n", *chunks, *bulkImport)
		t0 := time.Now()
		if err := engine.BatchPutBytes(items); err != nil {
			fatal(err)
		}
		ingestMs = msSince(t0)
		engine.Close()
	}

	if *prepareOnly {
		fmt.Printf("dir=%s\n", storeDir)
		fmt.Printf("ingest_ms=%.1f bulk_import=%v\n", ingestMs, *bulkImport)
		return
	}

	engine, err := f4kvs.NewPersistentEngine(storeDir)
	if err != nil {
		fatal(err)
	}
	defer engine.Close()

	fmt.Fprintf(os.Stderr, "ffi random_get %d ops...\n", *randomGets)
	t0 := time.Now()
	for i := 0; i < *randomGets; i++ {
		key := chunkKeys[i%len(chunkKeys)]
		if _, err := engine.GetBytes(key); err != nil {
			fatal(err)
		}
	}
	ffiMs = msSince(t0)

	rep := report{
		Host:        fmt.Sprintf("%s/%s", runtime.GOOS, runtime.GOARCH),
		Dir:         storeDir,
		Chunks:      *chunks,
		ChunkBytes:  *chunkBytes,
		RandomGets:  *randomGets,
		BulkImport:  *bulkImport,
		IngestMs:    ingestMs,
		FFIReadMs:   ffiMs,
		FFIReadOpsS: opsPerS(*randomGets, ffiMs),
	}

	fmt.Printf("dir=%s\n", storeDir)
	fmt.Printf("ingest_ms=%.1f bulk_import=%v\n", ingestMs, *bulkImport)
	fmt.Printf("ffi_read_ms=%.1f ops/s=%.0f\n", ffiMs, rep.FFIReadOpsS)

	if *out != "" {
		b, err := json.MarshalIndent(rep, "", "  ")
		if err != nil {
			fatal(err)
		}
		if err := os.WriteFile(*out, b, 0o644); err != nil {
			fatal(err)
		}
		fmt.Printf("wrote %s\n", *out)
	}
}

func samplePayload(n int) []byte {
	out := make([]byte, n)
	for i := range out {
		out[i] = byte('a' + (i*17)%26)
	}
	return out
}

func msSince(t time.Time) float64 {
	return float64(time.Since(t).Microseconds()) / 1000.0
}

func opsPerS(ops int, ms float64) float64 {
	if ms <= 0 {
		return 0
	}
	return float64(ops) / (ms / 1000.0)
}

func fatal(err error) {
	fmt.Fprintf(os.Stderr, "fatal: %v\n", err)
	os.Exit(1)
}