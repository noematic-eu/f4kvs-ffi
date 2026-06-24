// Product-shaped benchmark: f4kvs-ffi vs SQLite (modernc.org/sqlite).
//
// Workloads mirror living-memoirs + ai-rag-agent storage patterns:
//   - memoir blobs (large JSON-like values)
//   - RAG chunk batch ingest
//   - prefix listing
//   - random point reads
package main

import (
	"crypto/rand"
	"database/sql"
	"encoding/json"
	"flag"
	"fmt"
	"os"
	"path/filepath"
	"runtime"
	"sort"
	"time"

	f4kvs "github.com/noematic-eu/f4kvs-go"
	_ "modernc.org/sqlite"
)

type phaseResult struct {
	Phase   string  `json:"phase"`
	Store   string  `json:"store"`
	Ops     int     `json:"ops"`
	Ms      float64 `json:"ms"`
	OpsPerS float64 `json:"ops_per_s"`
	Extra   string  `json:"extra,omitempty"`
}

type report struct {
	Host      string        `json:"host"`
	Memoirs   int           `json:"memoirs"`
	Chunks    int           `json:"chunks"`
	MemoirB   int           `json:"memoir_bytes"`
	ChunkB    int           `json:"chunk_bytes"`
	RandomGet int           `json:"random_gets"`
	Results   []phaseResult `json:"results"`
}

func main() {
	memoirs := flag.Int("memoirs", 100, "memoir blob count")
	chunks := flag.Int("chunks", 5000, "chunk count")
	memoirBytes := flag.Int("memoir-bytes", 200_000, "memoir blob size")
	chunkBytes := flag.Int("chunk-bytes", 4096, "chunk payload size")
	randomGets := flag.Int("random-gets", 1000, "random point reads after ingest")
	out := flag.String("out", "", "optional JSON report path")
	flag.Parse()

	payload := samplePayload(*memoirBytes)
	chunkPayload := samplePayload(*chunkBytes)

	memoirKeys := make([]string, *memoirs)
	for i := range memoirKeys {
		memoirKeys[i] = fmt.Sprintf("memoir:%04d", i)
	}
	chunkKeys := make([]string, *chunks)
	for i := range chunkKeys {
		chunkKeys[i] = fmt.Sprintf("chunk:legal:doc-%04d:chunk-%06d", i/10, i)
	}

	tmp, err := os.MkdirTemp("", "f4kvs-sqlite-bench-*")
	if err != nil {
		fatal(err)
	}
	defer os.RemoveAll(tmp)

	rep := report{
		Host:      fmt.Sprintf("%s/%s", runtime.GOOS, runtime.GOARCH),
		Memoirs:   *memoirs,
		Chunks:    *chunks,
		MemoirB:   *memoirBytes,
		ChunkB:    *chunkBytes,
		RandomGet: *randomGets,
	}

	f4Dir := filepath.Join(tmp, "f4kvs")
	sqlPath := filepath.Join(tmp, "kv.db")

	rep.Results = append(rep.Results, benchF4KVS(f4Dir, memoirKeys, chunkKeys, payload, chunkPayload, *randomGets)...)
	rep.Results = append(rep.Results, benchSQLite(sqlPath, memoirKeys, chunkKeys, payload, chunkPayload, *randomGets)...)

	printTable(rep.Results)
	if *out != "" {
		writeJSON(*out, rep)
	}
}

func benchF4KVS(dir string, memoirKeys, chunkKeys []string, memoirPayload, chunkPayload []byte, randomGets int) []phaseResult {
	var out []phaseResult
	fmt.Fprintf(os.Stderr, "[f4kvs] starting...\n")

	engine, err := f4kvs.NewPersistentEngine(dir)
	if err != nil {
		fatal(err)
	}
	defer engine.Close()

	fmt.Fprintf(os.Stderr, "[f4kvs] memoir_batch_put (%d)...\n", len(memoirKeys))
	t0 := time.Now()
	for _, key := range memoirKeys {
		if err := engine.PutBytes(key, memoirPayload); err != nil {
			fatal(err)
		}
	}
	out = append(out, result("memoir_batch_put", "f4kvs", len(memoirKeys), time.Since(t0), ""))

	t0 = time.Now()
	for i := 0; i < randomGets; i++ {
		key := memoirKeys[i%len(memoirKeys)]
		if _, err := engine.GetBytes(key); err != nil {
			fatal(err)
		}
	}
	out = append(out, result("memoir_random_get", "f4kvs", randomGets, time.Since(t0), ""))

	fmt.Fprintf(os.Stderr, "[f4kvs] chunk_batch_put (%d)...\n", len(chunkKeys))
	t0 = time.Now()
	for _, key := range chunkKeys {
		if err := engine.PutBytes(key, chunkPayload); err != nil {
			fatal(err)
		}
	}
	out = append(out, result("chunk_batch_put", "f4kvs", len(chunkKeys), time.Since(t0), ""))

	t0 = time.Now()
	keys := engine.ScanPrefixKeys("chunk:legal:")
	scanMs := time.Since(t0)
	out = append(out, result("chunk_prefix_scan", "f4kvs", len(keys), scanMs, fmt.Sprintf("keys=%d", len(keys))))

	t0 = time.Now()
	for i := 0; i < randomGets; i++ {
		key := chunkKeys[i%len(chunkKeys)]
		if _, err := engine.GetBytes(key); err != nil {
			fatal(err)
		}
	}
	out = append(out, result("chunk_random_get", "f4kvs", randomGets, time.Since(t0), ""))

	return out
}

func benchSQLite(path string, memoirKeys, chunkKeys []string, memoirPayload, chunkPayload []byte, randomGets int) []phaseResult {
	var out []phaseResult
	fmt.Fprintf(os.Stderr, "[sqlite] starting...\n")

	db, err := sql.Open("sqlite", path+"?_pragma=journal_mode(WAL)&_pragma=synchronous(NORMAL)")
	if err != nil {
		fatal(err)
	}
	defer db.Close()

	if _, err := db.Exec(`CREATE TABLE kv (
		key TEXT PRIMARY KEY,
		value BLOB NOT NULL
	) WITHOUT ROWID`); err != nil {
		fatal(err)
	}

	tx, err := db.Begin()
	if err != nil {
		fatal(err)
	}
	stmt, err := tx.Prepare(`INSERT INTO kv (key, value) VALUES (?, ?)`)
	if err != nil {
		fatal(err)
	}

	t0 := time.Now()
	for _, key := range memoirKeys {
		if _, err := stmt.Exec(key, memoirPayload); err != nil {
			fatal(err)
		}
	}
	if err := tx.Commit(); err != nil {
		fatal(err)
	}
	out = append(out, result("memoir_batch_put", "sqlite", len(memoirKeys), time.Since(t0), "WAL batch"))

	t0 = time.Now()
	for i := 0; i < randomGets; i++ {
		key := memoirKeys[i%len(memoirKeys)]
		var blob []byte
		if err := db.QueryRow(`SELECT value FROM kv WHERE key = ?`, key).Scan(&blob); err != nil {
			fatal(err)
		}
	}
	out = append(out, result("memoir_random_get", "sqlite", randomGets, time.Since(t0), ""))

	tx, err = db.Begin()
	if err != nil {
		fatal(err)
	}
	stmt, err = tx.Prepare(`INSERT INTO kv (key, value) VALUES (?, ?)`)
	if err != nil {
		fatal(err)
	}

	t0 = time.Now()
	for _, key := range chunkKeys {
		if _, err := stmt.Exec(key, chunkPayload); err != nil {
			fatal(err)
		}
	}
	if err := tx.Commit(); err != nil {
		fatal(err)
	}
	out = append(out, result("chunk_batch_put", "sqlite", len(chunkKeys), time.Since(t0), "WAL batch"))

	t0 = time.Now()
	rows, err := db.Query(`SELECT key FROM kv WHERE key LIKE ?`, "chunk:legal:%")
	if err != nil {
		fatal(err)
	}
	var keys []string
	for rows.Next() {
		var k string
		if err := rows.Scan(&k); err != nil {
			fatal(err)
		}
		keys = append(keys, k)
	}
	rows.Close()
	out = append(out, result("chunk_prefix_scan", "sqlite", len(keys), time.Since(t0), fmt.Sprintf("keys=%d", len(keys))))

	t0 = time.Now()
	for i := 0; i < randomGets; i++ {
		key := chunkKeys[i%len(chunkKeys)]
		var blob []byte
		if err := db.QueryRow(`SELECT value FROM kv WHERE key = ?`, key).Scan(&blob); err != nil {
			fatal(err)
		}
	}
	out = append(out, result("chunk_random_get", "sqlite", randomGets, time.Since(t0), ""))

	return out
}

func result(phase, store string, ops int, d time.Duration, extra string) phaseResult {
	ms := float64(d.Microseconds()) / 1000.0
	opsPerS := 0.0
	if ms > 0 {
		opsPerS = float64(ops) / (ms / 1000.0)
	}
	return phaseResult{Phase: phase, Store: store, Ops: ops, Ms: ms, OpsPerS: opsPerS, Extra: extra}
}

func samplePayload(n int) []byte {
	if n <= 0 {
		return nil
	}
	// JSON-like blob — O(n), stable size (memoir/chunk envelope).
	head := []byte(`{"v":1,"title":"bench","body":"`)
	tail := []byte(`"}`)
	out := make([]byte, n)
	copy(out, head)
	fill := n - len(head) - len(tail)
	if fill < 0 {
		return out[:n]
	}
	for i := 0; i < fill; i++ {
		out[len(head)+i] = 'a' + byte((i*17+int(randByte()))%26)
	}
	copy(out[len(head)+fill:], tail)
	return out
}

func randByte() byte {
	var b [1]byte
	_, _ = rand.Read(b[:])
	return b[0]
}

func printTable(results []phaseResult) {
	byPhase := map[string][]phaseResult{}
	for _, r := range results {
		byPhase[r.Phase] = append(byPhase[r.Phase], r)
	}
	phases := make([]string, 0, len(byPhase))
	for p := range byPhase {
		phases = append(phases, p)
	}
	sort.Strings(phases)

	fmt.Println()
	fmt.Println("=== f4kvs-ffi vs SQLite (product-shaped workloads) ===")
	fmt.Printf("%-22s %-8s %8s %12s %12s %s\n", "phase", "store", "ops", "ms", "ops/s", "notes")
	for _, phase := range phases {
		for _, r := range byPhase[phase] {
			fmt.Printf("%-22s %-8s %8d %12.1f %12.0f %s\n", r.Phase, r.Store, r.Ops, r.Ms, r.OpsPerS, r.Extra)
		}
		ratio := ratioLine(byPhase[phase])
		if ratio != "" {
			fmt.Printf("  → %s\n", ratio)
		}
	}
}

func ratioLine(rows []phaseResult) string {
	var f4, sql float64
	for _, r := range rows {
		if r.Store == "f4kvs" {
			f4 = r.Ms
		}
		if r.Store == "sqlite" {
			sql = r.Ms
		}
	}
	if f4 == 0 || sql == 0 {
		return ""
	}
	if f4 > sql {
		return fmt.Sprintf("sqlite %.1f× faster", f4/sql)
	}
	return fmt.Sprintf("f4kvs %.1f× faster", sql/f4)
}

func writeJSON(path string, rep report) {
	b, err := json.MarshalIndent(rep, "", "  ")
	if err != nil {
		fatal(err)
	}
	if err := os.WriteFile(path, b, 0o644); err != nil {
		fatal(err)
	}
	fmt.Printf("\nWrote %s\n", path)
}

func fatal(err error) {
	fmt.Fprintf(os.Stderr, "fatal: %v\n", err)
	os.Exit(1)
}

