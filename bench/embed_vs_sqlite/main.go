// Product-shaped benchmark: f4kvs-ffi vs SQLite (modernc.org/sqlite).
//
// Durability-matched column (fair, per-commit):
//   - f4kvs_wal_fsync  — engine default (WAL + WalSyncMode::Fsync per put)
//   - sqlite_wal_full  — journal_mode=WAL, synchronous=FULL, one commit per put
//
// Batched ingest column (product-shaped, one durable unit per batch):
//   - chunk_batch_put_batched — f4kvs BatchPutBytes (one WAL fsync) vs sqlite batched tx
//
// Reference column (throughput-oriented, not durability-matched):
//   - sqlite_wal_normal — WAL + synchronous=NORMAL, batched transactions
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
	"strings"
	"time"

	f4kvs "github.com/noematic-eu/f4kvs-go"
	_ "modernc.org/sqlite"
)

type phaseResult struct {
	Phase    string  `json:"phase"`
	Profile  string  `json:"profile"`
	Ops      int     `json:"ops"`
	Ms       float64 `json:"ms"`
	OpsPerS  float64 `json:"ops_per_s"`
	Durability string `json:"durability,omitempty"`
	Extra    string  `json:"extra,omitempty"`
}

type report struct {
	Host        string        `json:"host"`
	Memoirs     int           `json:"memoirs"`
	Chunks      int           `json:"chunks"`
	MemoirB     int           `json:"memoir_bytes"`
	ChunkB      int           `json:"chunk_bytes"`
	RandomGet   int           `json:"random_gets"`
	FairCompare     string        `json:"fair_compare"`
	BatchedCompare  string        `json:"batched_compare"`
	Results         []phaseResult `json:"results"`
}

type sqliteProfile struct {
	Name       string
	DSN        string
	Durability string
	PerCommit  bool
	Extra      string
}

func main() {
	memoirs := flag.Int("memoirs", 50, "memoir blob count")
	chunks := flag.Int("chunks", 2000, "chunk count")
	memoirBytes := flag.Int("memoir-bytes", 200_000, "memoir blob size")
	chunkBytes := flag.Int("chunk-bytes", 4096, "chunk payload size")
	randomGets := flag.Int("random-gets", 500, "random point reads after ingest")
	includeRelaxed := flag.Bool("include-relaxed", true, "also run sqlite_wal_normal batched reference column")
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
		Host:        fmt.Sprintf("%s/%s", runtime.GOOS, runtime.GOARCH),
		Memoirs:     *memoirs,
		Chunks:      *chunks,
		MemoirB:     *memoirBytes,
		ChunkB:      *chunkBytes,
		RandomGet:   *randomGets,
		FairCompare:    "f4kvs_wal_fsync vs sqlite_wal_full (per-commit puts)",
		BatchedCompare: "f4kvs_wal_fsync vs sqlite_wal_full (chunk_batch_put_batched)",
	}

	fmt.Fprintf(os.Stderr, "=== fair: f4kvs_wal_fsync vs sqlite_wal_full (per-commit) ===\n")
	rep.Results = append(rep.Results, benchF4KVS(
		filepath.Join(tmp, "f4kvs_fsync"),
		"f4kvs_wal_fsync",
		"WAL + WalSyncMode::Fsync (engine default, per put)",
		nil,
		memoirKeys, chunkKeys, payload, chunkPayload, *randomGets,
	)...)

	fmt.Fprintf(os.Stderr, "=== f4kvs group commit: 10ms window (per PutBytes, amortized fsync) ===\n")
	rep.Results = append(rep.Results, benchF4KVS(
		filepath.Join(tmp, "f4kvs_gc10"),
		"f4kvs_group_commit_10ms",
		"WAL Fsync + group_commit 10ms window (async ack, durable within 10ms)",
		&f4kvs.OpenOptions{
			GroupCommitEnabled:   true,
			GroupCommitMaxWaitMs: 10,
		},
		memoirKeys, chunkKeys, payload, chunkPayload, *randomGets,
	)...)

	sqliteProfiles := []sqliteProfile{
		{
			Name:       "sqlite_wal_full",
			DSN:        sqliteDSN("WAL", "FULL"),
			Durability: "WAL + synchronous=FULL, per-commit put",
			PerCommit:  true,
			Extra:      "durability-matched",
		},
	}
	if *includeRelaxed {
		sqliteProfiles = append(sqliteProfiles, sqliteProfile{
			Name:       "sqlite_wal_normal",
			DSN:        sqliteDSN("WAL", "NORMAL"),
			Durability: "WAL + synchronous=NORMAL, batched tx",
			PerCommit:  false,
			Extra:      "reference (relaxed)",
		})
	}

	for i, prof := range sqliteProfiles {
		fmt.Fprintf(os.Stderr, "=== sqlite profile: %s ===\n", prof.Name)
		path := filepath.Join(tmp, fmt.Sprintf("sqlite_%d.db", i))
		rep.Results = append(rep.Results, benchSQLite(
			path, prof, memoirKeys, chunkKeys, payload, chunkPayload, *randomGets,
		)...)
	}

	printTable(rep.Results)
	if *out != "" {
		writeJSON(*out, rep)
	}
}

func sqliteDSN(journal, synchronous string) string {
	return fmt.Sprintf(
		"file:kv?_pragma=journal_mode(%s)&_pragma=synchronous(%s)&_pragma=foreign_keys(1)&_pragma=busy_timeout(5000)",
		journal, synchronous,
	)
}

func benchF4KVS(
	dir, profile, durability string,
	opts *f4kvs.OpenOptions,
	memoirKeys, chunkKeys []string,
	memoirPayload, chunkPayload []byte,
	randomGets int,
) []phaseResult {
	var out []phaseResult

	engine, err := f4kvs.NewPersistentEngineWithOptions(dir, opts)
	if err != nil {
		fatal(err)
	}
	defer engine.Close()

	fmt.Fprintf(os.Stderr, "[%s] memoir_batch_put (%d, per-commit)...\n", profile, len(memoirKeys))
	t0 := time.Now()
	for _, key := range memoirKeys {
		if err := engine.PutBytes(key, memoirPayload); err != nil {
			fatal(err)
		}
	}
	out = append(out, result("memoir_batch_put", profile, len(memoirKeys), time.Since(t0), durability, "per-commit"))

	t0 = time.Now()
	for i := 0; i < randomGets; i++ {
		key := memoirKeys[i%len(memoirKeys)]
		if _, err := engine.GetBytes(key); err != nil {
			fatal(err)
		}
	}
	out = append(out, result("memoir_random_get", profile, randomGets, time.Since(t0), durability, ""))

	fmt.Fprintf(os.Stderr, "[%s] chunk_batch_put (%d, per-commit)...\n", profile, len(chunkKeys))
	t0 = time.Now()
	for _, key := range chunkKeys {
		if err := engine.PutBytes(key, chunkPayload); err != nil {
			fatal(err)
		}
	}
	out = append(out, result("chunk_batch_put", profile, len(chunkKeys), time.Since(t0), durability, "per-commit"))

	if opts != nil && opts.GroupCommitEnabled {
		fmt.Fprintf(os.Stderr, "[%s] chunk_batch_put_flush (FlushWAL after async puts)...\n", profile)
		t0 = time.Now()
		if err := engine.FlushWAL(); err != nil {
			fatal(err)
		}
		out = append(out, result(
			"chunk_batch_put_flush", profile, len(chunkKeys), time.Since(t0), durability,
			"FlushWAL; durable within window, memtable not flushed",
		))

		fmt.Fprintf(os.Stderr, "[%s] chunk_batch_put_durable (puts + FlushWAL, end-to-end)...\n", profile)
		// Re-measure on fresh engine: puts then WAL flush in one timed block.
		out = append(out, benchF4KVSChunkDurable(dir, profile, durability, opts, chunkKeys, chunkPayload)...)
	}

	out = append(out, benchF4KVSChunkBatched(filepath.Join(dir, "chunk_batched"), profile, chunkKeys, chunkPayload)...)

	t0 = time.Now()
	keys := engine.ScanPrefixKeys("chunk:legal:")
	out = append(out, result("chunk_prefix_scan", profile, len(keys), time.Since(t0), durability, fmt.Sprintf("keys=%d", len(keys))))

	t0 = time.Now()
	for i := 0; i < randomGets; i++ {
		key := chunkKeys[i%len(chunkKeys)]
		if _, err := engine.GetBytes(key); err != nil {
			fatal(err)
		}
	}
	out = append(out, result("chunk_random_get", profile, randomGets, time.Since(t0), durability, ""))

	return out
}

func benchF4KVSChunkDurable(
	dir, profile, durability string,
	opts *f4kvs.OpenOptions,
	chunkKeys []string,
	chunkPayload []byte,
) []phaseResult {
	var out []phaseResult
	durableDir := filepath.Join(dir, "chunk_durable")
	_ = os.RemoveAll(durableDir)

	engine, err := f4kvs.NewPersistentEngineWithOptions(durableDir, opts)
	if err != nil {
		fatal(err)
	}
	defer engine.Close()

	t0 := time.Now()
	for _, key := range chunkKeys {
		if err := engine.PutBytes(key, chunkPayload); err != nil {
			fatal(err)
		}
	}
	if err := engine.FlushWAL(); err != nil {
		fatal(err)
	}
	out = append(out, result(
		"chunk_batch_put_durable", profile, len(chunkKeys), time.Since(t0), durability,
		"puts + FlushWAL end-to-end",
	))
	return out
}

func benchF4KVSChunkBatched(dir, profile string, chunkKeys []string, chunkPayload []byte) []phaseResult {
	const durability = "WAL + WalSyncMode::Fsync (one fsync per BatchPutBytes)"
	var out []phaseResult

	engine, err := f4kvs.NewPersistentEngine(dir)
	if err != nil {
		fatal(err)
	}
	defer engine.Close()

	items := make(map[string][]byte, len(chunkKeys))
	for _, key := range chunkKeys {
		items[key] = chunkPayload
	}

	fmt.Fprintf(os.Stderr, "[%s] chunk_batch_put_batched (%d, BatchPutBytes)...\n", profile, len(chunkKeys))
	t0 := time.Now()
	if err := engine.BatchPutBytes(items); err != nil {
		fatal(err)
	}
	out = append(out, result("chunk_batch_put_batched", profile, len(chunkKeys), time.Since(t0), durability, "BatchPutBytes"))

	return out
}

func benchSQLite(path string, prof sqliteProfile, memoirKeys, chunkKeys []string, memoirPayload, chunkPayload []byte, randomGets int) []phaseResult {
	var out []phaseResult

	// modernc sqlite DSN: replace file placeholder with real path
	dsn := strings.Replace(prof.DSN, "file:kv?", "file:"+path+"?", 1)
	db, err := sql.Open("sqlite", dsn)
	if err != nil {
		fatal(err)
	}
	defer db.Close()
	db.SetMaxOpenConns(1)

	if _, err := db.Exec(`CREATE TABLE kv (
		key TEXT PRIMARY KEY,
		value BLOB NOT NULL
	) WITHOUT ROWID`); err != nil {
		fatal(err)
	}

	putNote := "per-commit"
	if !prof.PerCommit {
		putNote = "batched tx"
	}

	fmt.Fprintf(os.Stderr, "[%s] memoir_batch_put (%d, %s)...\n", prof.Name, len(memoirKeys), putNote)
	t0 := time.Now()
	if err := sqliteBatchPut(db, prof.PerCommit, memoirKeys, memoirPayload); err != nil {
		fatal(err)
	}
	out = append(out, result("memoir_batch_put", prof.Name, len(memoirKeys), time.Since(t0), prof.Durability, prof.Extra+"; "+putNote))

	t0 = time.Now()
	for i := 0; i < randomGets; i++ {
		key := memoirKeys[i%len(memoirKeys)]
		var blob []byte
		if err := db.QueryRow(`SELECT value FROM kv WHERE key = ?`, key).Scan(&blob); err != nil {
			fatal(err)
		}
	}
	out = append(out, result("memoir_random_get", prof.Name, randomGets, time.Since(t0), prof.Durability, ""))

	fmt.Fprintf(os.Stderr, "[%s] chunk_batch_put (%d, %s)...\n", prof.Name, len(chunkKeys), putNote)
	t0 = time.Now()
	if err := sqliteBatchPut(db, prof.PerCommit, chunkKeys, chunkPayload); err != nil {
		fatal(err)
	}
	out = append(out, result("chunk_batch_put", prof.Name, len(chunkKeys), time.Since(t0), prof.Durability, prof.Extra+"; "+putNote))

	out = append(out, benchSQLiteChunkBatched(path+"_chunk_batched", prof, chunkKeys, chunkPayload)...)

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
	out = append(out, result("chunk_prefix_scan", prof.Name, len(keys), time.Since(t0), prof.Durability, fmt.Sprintf("keys=%d", len(keys))))

	t0 = time.Now()
	for i := 0; i < randomGets; i++ {
		key := chunkKeys[i%len(chunkKeys)]
		var blob []byte
		if err := db.QueryRow(`SELECT value FROM kv WHERE key = ?`, key).Scan(&blob); err != nil {
			fatal(err)
		}
	}
	out = append(out, result("chunk_random_get", prof.Name, randomGets, time.Since(t0), prof.Durability, ""))

	return out
}

func benchSQLiteChunkBatched(path string, prof sqliteProfile, chunkKeys []string, chunkPayload []byte) []phaseResult {
	var out []phaseResult

	dsn := strings.Replace(prof.DSN, "file:kv?", "file:"+path+"?", 1)
	db, err := sql.Open("sqlite", dsn)
	if err != nil {
		fatal(err)
	}
	defer db.Close()
	db.SetMaxOpenConns(1)

	if _, err := db.Exec(`CREATE TABLE kv (
		key TEXT PRIMARY KEY,
		value BLOB NOT NULL
	) WITHOUT ROWID`); err != nil {
		fatal(err)
	}

	fmt.Fprintf(os.Stderr, "[%s] chunk_batch_put_batched (%d, batched tx)...\n", prof.Name, len(chunkKeys))
	t0 := time.Now()
	if err := sqliteBatchPut(db, false, chunkKeys, chunkPayload); err != nil {
		fatal(err)
	}
	out = append(out, result(
		"chunk_batch_put_batched", prof.Name, len(chunkKeys), time.Since(t0),
		prof.Durability, prof.Extra+"; batched tx",
	))

	return out
}

func sqliteBatchPut(db *sql.DB, perCommit bool, keys []string, payload []byte) error {
	if perCommit {
		for _, key := range keys {
			if _, err := db.Exec(`INSERT INTO kv (key, value) VALUES (?, ?)`, key, payload); err != nil {
				return err
			}
		}
		return nil
	}
	tx, err := db.Begin()
	if err != nil {
		return err
	}
	stmt, err := tx.Prepare(`INSERT INTO kv (key, value) VALUES (?, ?)`)
	if err != nil {
		return err
	}
	for _, key := range keys {
		if _, err := stmt.Exec(key, payload); err != nil {
			return err
		}
	}
	return tx.Commit()
}

func result(phase, profile string, ops int, d time.Duration, durability, extra string) phaseResult {
	ms := float64(d.Microseconds()) / 1000.0
	opsPerS := 0.0
	if ms > 0 {
		opsPerS = float64(ops) / (ms / 1000.0)
	}
	return phaseResult{
		Phase: phase, Profile: profile, Ops: ops, Ms: ms, OpsPerS: opsPerS,
		Durability: durability, Extra: extra,
	}
}

func samplePayload(n int) []byte {
	if n <= 0 {
		return nil
	}
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
	fmt.Printf("%-22s %-18s %8s %12s %12s %s\n", "phase", "profile", "ops", "ms", "ops/s", "notes")
	for _, phase := range phases {
		sort.Slice(byPhase[phase], func(i, j int) bool {
			return byPhase[phase][i].Profile < byPhase[phase][j].Profile
		})
		for _, r := range byPhase[phase] {
			note := r.Extra
			if note == "" {
				note = r.Durability
			}
			fmt.Printf("%-22s %-18s %8d %12.1f %12.0f %s\n", r.Phase, r.Profile, r.Ops, r.Ms, r.OpsPerS, note)
		}
		for _, cmp := range phaseCompares(byPhase[phase], phase) {
			fmt.Printf("  → %s: %s\n", cmp.label, cmp.line)
		}
	}
}

type compareLine struct {
	label string
	line  string
}

func phaseCompares(rows []phaseResult, phase string) []compareLine {
	var out []compareLine
	if line := ratioLine(rows, "f4kvs_wal_fsync", "sqlite_wal_full"); line != "" {
		label := "fair compare"
		if phase == "chunk_batch_put_batched" {
			label = "batched compare"
		}
		out = append(out, compareLine{label: label, line: line})
	}
	if line := ratioLine(rows, "f4kvs_group_commit_10ms", "sqlite_wal_full"); line != "" {
		out = append(out, compareLine{
			label: "group-commit compare",
			line:  line,
		})
	}
	if phase == "chunk_batch_put_durable" {
		if line := ratioLine(rows, "f4kvs_group_commit_10ms", "sqlite_wal_full"); line != "" {
			out = append(out, compareLine{
				label: "durable ingest compare",
				line:  line,
			})
		}
	}
	if phase == "chunk_batch_put_flush" {
		if line := ratioLine(rows, "f4kvs_group_commit_10ms", "sqlite_wal_full"); line != "" {
			out = append(out, compareLine{
				label: "wal-flush compare",
				line:  line,
			})
		}
	}
	return out
}

func ratioLine(rows []phaseResult, f4Profile, sqlProfile string) string {
	var f4, sql float64
	for _, r := range rows {
		if r.Profile == f4Profile {
			f4 = r.Ms
		}
		if r.Profile == sqlProfile {
			sql = r.Ms
		}
	}
	if f4 == 0 || sql == 0 {
		return ""
	}
	if f4 > sql {
		return fmt.Sprintf("%s %.1f× faster than %s", sqlProfile, f4/sql, f4Profile)
	}
	return fmt.Sprintf("%s %.1f× faster than %s", f4Profile, sql/f4, sqlProfile)
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