// Product-shaped benchmark: f4kvs-ffi vs SQLite (modernc.org/sqlite).
//
// Durability-matched column (fair):
//   - f4kvs_wal_fsync  — engine default (WAL + WalSyncMode::Fsync per put)
//   - sqlite_wal_full  — journal_mode=WAL, synchronous=FULL, one commit per put
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
	FairCompare string        `json:"fair_compare"`
	Results     []phaseResult `json:"results"`
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
		FairCompare: "f4kvs_wal_fsync vs sqlite_wal_full (per-commit puts)",
	}

	fmt.Fprintf(os.Stderr, "=== fair: f4kvs_wal_fsync vs sqlite_wal_full (per-commit) ===\n")
	rep.Results = append(rep.Results, benchF4KVS(
		filepath.Join(tmp, "f4kvs_fsync"),
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

func benchF4KVS(dir string, memoirKeys, chunkKeys []string, memoirPayload, chunkPayload []byte, randomGets int) []phaseResult {
	const profile = "f4kvs_wal_fsync"
	const durability = "WAL + WalSyncMode::Fsync (engine default, per put)"
	var out []phaseResult

	engine, err := f4kvs.NewPersistentEngine(dir)
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
		if fair := fairRatioLine(byPhase[phase]); fair != "" {
			fmt.Printf("  → fair compare: %s\n", fair)
		}
	}
}

func fairRatioLine(rows []phaseResult) string {
	var f4, sql float64
	for _, r := range rows {
		if r.Profile == "f4kvs_wal_fsync" {
			f4 = r.Ms
		}
		if r.Profile == "sqlite_wal_full" {
			sql = r.Ms
		}
	}
	if f4 == 0 || sql == 0 {
		return ""
	}
	if f4 > sql {
		return fmt.Sprintf("sqlite_wal_full %.1f× faster than f4kvs_wal_fsync", f4/sql)
	}
	return fmt.Sprintf("f4kvs_wal_fsync %.1f× faster than sqlite_wal_full", sql/f4)
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