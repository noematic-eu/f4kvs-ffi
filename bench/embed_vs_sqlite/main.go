// Product-shaped benchmark: f4kvs-ffi vs SQLite (modernc.org/sqlite).
//
// Durability-matched column (fair, per-commit):
//   - f4kvs_wal_segment — segment WAL + sync_all per put (current default)
//   - f4kvs_wal_frame    — frame WAL + sync_data per put (SQLite-like)
//   - sqlite_wal_full    — journal_mode=WAL, synchronous=FULL, one commit per put
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
	Phase      string  `json:"phase"`
	Profile    string  `json:"profile"`
	Ops        int     `json:"ops"`
	Ms         float64 `json:"ms"`
	OpsPerS    float64 `json:"ops_per_s"`
	Durability string  `json:"durability,omitempty"`
	Extra      string  `json:"extra,omitempty"`
	// MetricOnly/Value/Unit: when MetricOnly is set, DE export emits a single long-format
	// metric row instead of the default duration_ms + ops_per_s pair (used by restart).
	MetricOnly string  `json:"metric,omitempty"`
	Value      float64 `json:"value,omitempty"`
	Unit       string  `json:"unit,omitempty"`
}

type report struct {
	Host           string        `json:"host"`
	Memoirs        int           `json:"memoirs"`
	Chunks         int           `json:"chunks"`
	MemoirB        int           `json:"memoir_bytes"`
	ChunkB         int           `json:"chunk_bytes"`
	RandomGet      int           `json:"random_gets"`
	FairCompare    string        `json:"fair_compare"`
	BatchedCompare string        `json:"batched_compare"`
	Results        []phaseResult `json:"results"`
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
	seed := flag.Int("seed", 42, "deterministic seed (keys are memoir:%04d / chunk:…; payloads use seed fill)")
	runsRoot := flag.String("runs-root", "", "if set, write DE layout under runs-root/{run_id}/ (manifest + results.jsonl)")
	tierFlag := flag.String("tier", "", "micro|meso|macro (auto from chunks if empty)")
	profilesFlag := flag.String("profiles", "all", "profile set: all | product | comma-separated names (see README)")
	// Per-commit 100k PutBytes + FlushWAL under group-commit can hang/stall (observed 2026-07-23).
	// Above this count, chunk ingest uses BatchPut / batched tx only (product-shaped meso path).
	maxPerCommitChunks := flag.Int("max-per-commit-chunks", 10_000, "skip per-commit chunk puts above this count (0=never skip)")
	out := flag.String("out", "", "optional legacy flat JSON report path (also written as report.legacy.json in run dir)")
	runIDFlag := flag.String("run-id", "", "optional run_id (default: UTC compact ISO)")
	flag.Parse()

	want, err := parseProfiles(*profilesFlag)
	if err != nil {
		fatal(err)
	}
	// Expand "all" so include-relaxed can drop sqlite_wal_normal cleanly.
	if want["all"] {
		want = map[string]bool{}
		for _, p := range knownProfiles {
			want[p] = true
		}
	}
	if !*includeRelaxed {
		delete(want, "sqlite_wal_normal")
	}

	payload := samplePayloadSeeded(*memoirBytes, int64(*seed))
	chunkPayload := samplePayloadSeeded(*chunkBytes, int64(*seed)+1)

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

	tier := *tierFlag
	if tier == "" {
		tier = DeriveTier(*chunks)
	}

	cwd, _ := os.Getwd()
	repoRoot := FindRepoRoot(cwd)
	runID := *runIDFlag
	if runID == "" {
		runID = NewRunID(time.Now())
	}

	profileList := selectedProfileNames(want)
	fmt.Fprintf(os.Stderr, "profiles=%s tier=%s chunks=%d\n", strings.Join(profileList, ","), tier, *chunks)

	var writer *RunWriter
	if *runsRoot != "" {
		man := RunManifest{
			Tier: tier,
			Seed: *seed,
			Git:  CollectGitInfo(repoRoot),
			Host: CollectHostInfo(),
			Scale: ScaleInfo{
				Memoirs:     *memoirs,
				Chunks:      *chunks,
				MemoirBytes: *memoirBytes,
				ChunkBytes:  *chunkBytes,
				RandomGets:  *randomGets,
			},
			Harness: HarnessInfo{
				Path: harnessPath,
				Command: FormatHarnessCommand(
					*memoirs, *chunks, *memoirBytes, *chunkBytes, *randomGets, *seed, *includeRelaxed, *profilesFlag,
				),
			},
		}
		writer, err = NewRunWriter(*runsRoot, runID, man)
		if err != nil {
			fatal(err)
		}
		fmt.Fprintf(os.Stderr, "RUN_DIR_PARTIAL=%s\n", writer.PartialDir)
	}

	rep := report{
		Host:           fmt.Sprintf("%s/%s", runtime.GOOS, runtime.GOARCH),
		Memoirs:        *memoirs,
		Chunks:         *chunks,
		MemoirB:        *memoirBytes,
		ChunkB:         *chunkBytes,
		RandomGet:      *randomGets,
		FairCompare:    "f4kvs_wal_segment vs f4kvs_wal_frame vs sqlite_wal_full (per-commit puts)",
		BatchedCompare: "f4kvs_wal_segment vs sqlite_wal_full (chunk_batch_put_batched)",
	}

	run := func(name string) bool { return wantProfile(want, name) }

	skipPerCommitChunks := *maxPerCommitChunks > 0 && len(chunkKeys) > *maxPerCommitChunks
	if skipPerCommitChunks {
		fmt.Fprintf(os.Stderr, "note: chunks=%d > max-per-commit-chunks=%d → BatchPut/batched-tx path only (no per-commit chunk put)\n",
			len(chunkKeys), *maxPerCommitChunks)
	}

	if run("f4kvs_wal_segment") {
		fmt.Fprintf(os.Stderr, "=== fair: f4kvs_wal_segment (sync_all per put) ===\n")
		rep.Results = append(rep.Results, benchF4KVS(
			filepath.Join(tmp, "f4kvs_segment"),
			"f4kvs_wal_segment",
			"Segment WAL + WalSyncMode::Fsync (sync_all per put)",
			nil,
			memoirKeys, chunkKeys, payload, chunkPayload, *randomGets, skipPerCommitChunks,
		)...)
	}

	if run("f4kvs_wal_frame") {
		fmt.Fprintf(os.Stderr, "=== fair: f4kvs_wal_frame (sync_data per put) ===\n")
		rep.Results = append(rep.Results, benchF4KVS(
			filepath.Join(tmp, "f4kvs_frame"),
			"f4kvs_wal_frame",
			"Frame WAL + WalSyncMode::Fsync (sync_data per put)",
			&f4kvs.OpenOptions{WalEngine: f4kvs.WalEngineFrame},
			memoirKeys, chunkKeys, payload, chunkPayload, *randomGets, skipPerCommitChunks,
		)...)
	}

	if run("f4kvs_wal_indexed") {
		fmt.Fprintf(os.Stderr, "=== fair: f4kvs_wal_indexed (WAL v2 pre-allocated frames + wal.idx) ===\n")
		rep.Results = append(rep.Results, benchF4KVS(
			filepath.Join(tmp, "f4kvs_indexed"),
			"f4kvs_wal_indexed",
			"Indexed WAL v2 — per-frame micro-files + wal.idx (wal_engine=2)",
			&f4kvs.OpenOptions{WalEngine: f4kvs.WalEngineIndexed},
			memoirKeys, chunkKeys, payload, chunkPayload, *randomGets, skipPerCommitChunks,
		)...)
	}

	gc10Opts := &f4kvs.OpenOptions{
		GroupCommitEnabled:   true,
		GroupCommitMaxWaitMs: 10,
	}
	if run("f4kvs_group_commit_10ms") {
		fmt.Fprintf(os.Stderr, "=== f4kvs group commit: 10ms window (per PutBytes, amortized fsync) ===\n")
		rep.Results = append(rep.Results, benchF4KVS(
			filepath.Join(tmp, "f4kvs_gc10"),
			"f4kvs_group_commit_10ms",
			"WAL Fsync + group_commit 10ms window (async ack, durable within 10ms)",
			gc10Opts,
			memoirKeys, chunkKeys, payload, chunkPayload, *randomGets, skipPerCommitChunks,
		)...)
	}

	if run("f4kvs_group_commit_idle") {
		fmt.Fprintf(os.Stderr, "=== f4kvs amortized: 50ms window + 100ms idle flush (wal_durability=1) ===\n")
		rep.Results = append(rep.Results, benchF4KVS(
			filepath.Join(tmp, "f4kvs_gc_idle"),
			"f4kvs_group_commit_idle",
			"Amortized WAL — 50ms max wait + 100ms idle flush (wal_durability=1)",
			&f4kvs.OpenOptions{
				WalDurability:          f4kvs.WalDurabilityAmortized,
				GroupCommitMaxWaitMs:   50,
				GroupCommitIdleFlushMs: 100,
			},
			memoirKeys, chunkKeys, payload, chunkPayload, *randomGets, skipPerCommitChunks,
		)...)
	}

	sqliteProfiles := []sqliteProfile{}
	if run("sqlite_wal_full") {
		sqliteProfiles = append(sqliteProfiles, sqliteProfile{
			Name:       "sqlite_wal_full",
			DSN:        sqliteDSN("WAL", "FULL"),
			Durability: "WAL + synchronous=FULL, per-commit put",
			PerCommit:  true,
			Extra:      "durability-matched",
		})
	}
	if run("sqlite_wal_normal") {
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
		// Force batched chunk path when scale skips per-commit.
		p := prof
		if skipPerCommitChunks {
			p.PerCommit = false
		}
		rep.Results = append(rep.Results, benchSQLite(
			path, p, memoirKeys, chunkKeys, payload, chunkPayload, *randomGets,
		)...)
	}

	// Integrity gate: close/reopen + row count for primary f4kvs product engine.
	// Prefer segment (reliable at meso); fall back to gc10 if only that ran.
	if run("f4kvs_wal_segment") {
		fmt.Fprintf(os.Stderr, "=== post_restart_row_count: f4kvs_wal_segment ===\n")
		rep.Results = append(rep.Results, benchF4KVSPostRestart(
			filepath.Join(tmp, "restart_f4kvs_segment"),
			"f4kvs_wal_segment",
			nil,
			memoirKeys, chunkKeys, payload, chunkPayload,
		)...)
	} else if run("f4kvs_group_commit_10ms") {
		fmt.Fprintf(os.Stderr, "=== post_restart_row_count: f4kvs_group_commit_10ms ===\n")
		rep.Results = append(rep.Results, benchF4KVSPostRestart(
			filepath.Join(tmp, "restart_f4kvs_gc10"),
			"f4kvs_group_commit_10ms",
			gc10Opts,
			memoirKeys, chunkKeys, payload, chunkPayload,
		)...)
	}

	if run("sqlite_wal_full") {
		full := sqliteProfile{
			Name:       "sqlite_wal_full",
			DSN:        sqliteDSN("WAL", "FULL"),
			Durability: "WAL + synchronous=FULL, per-commit put",
			PerCommit:  true,
			Extra:      "durability-matched",
		}
		fmt.Fprintf(os.Stderr, "=== post_restart_row_count: sqlite_wal_full ===\n")
		rep.Results = append(rep.Results, benchSQLitePostRestart(
			filepath.Join(tmp, "restart_sqlite_full.db"),
			full,
			memoirKeys, chunkKeys, payload, chunkPayload,
		)...)
	}

	if len(rep.Results) == 0 {
		fatal(fmt.Errorf("no profiles selected (profiles=%q)", *profilesFlag))
	}

	printTable(rep.Results)

	if *out != "" {
		writeJSON(*out, rep)
	}

	if writer != nil {
		writer.AppendPhaseResults(rep.Results)
		engines := UniqueEngines(rep.Results)
		finalDir, err := writer.Finalize(&rep, engines)
		if err != nil {
			fatal(err)
		}
		fmt.Fprintf(os.Stderr, "RUN_DIR=%s\n", finalDir)
		fmt.Fprintf(os.Stderr, "OK manifest.json results.jsonl (lines=%d)\n", writer.LineCount())
		if writer.HasIntegrityFailure() {
			fmt.Fprintf(os.Stderr, "FATAL: post_restart integrity_ok=0 (see results.jsonl)\n")
			os.Exit(1)
		}
	} else if hasIntegrityFail(rep.Results) {
		fmt.Fprintf(os.Stderr, "FATAL: post_restart integrity_ok=0\n")
		os.Exit(1)
	}
}

func hasIntegrityFail(results []phaseResult) bool {
	for _, r := range results {
		if r.Phase == "post_restart_row_count" && r.MetricOnly == "integrity_ok" && r.Value == 0 {
			return true
		}
	}
	return false
}

// Known durability profiles (bench-schema-v1 engine names).
var knownProfiles = []string{
	"f4kvs_wal_segment",
	"f4kvs_wal_frame",
	"f4kvs_wal_indexed",
	"f4kvs_group_commit_10ms",
	"f4kvs_group_commit_idle",
	"sqlite_wal_full",
	"sqlite_wal_normal",
}

// productProfiles is the meso-friendly set: product path + SQLite peer (+ integrity).
// Prefer segment WAL (sync per BatchPut) over group_commit_10ms for meso scale:
// group-commit + large BatchPut/FlushWAL was observed to stall (cond wait, 0% CPU) at ≥10k–100k keys.
var productProfiles = []string{
	"f4kvs_wal_segment",
	"sqlite_wal_full",
}

// parseProfiles expands "all" | "product" | comma-separated names into a set.
// The special key "all" means every known profile.
func parseProfiles(s string) (map[string]bool, error) {
	s = strings.TrimSpace(s)
	if s == "" || s == "all" {
		return map[string]bool{"all": true}, nil
	}
	out := map[string]bool{}
	if s == "product" {
		for _, p := range productProfiles {
			out[p] = true
		}
		return out, nil
	}
	known := map[string]bool{}
	for _, p := range knownProfiles {
		known[p] = true
	}
	for _, part := range strings.Split(s, ",") {
		name := strings.TrimSpace(part)
		if name == "" {
			continue
		}
		if name == "all" {
			return map[string]bool{"all": true}, nil
		}
		if name == "product" {
			for _, p := range productProfiles {
				out[p] = true
			}
			continue
		}
		if !known[name] {
			return nil, fmt.Errorf("unknown profile %q (want all|product|%s)", name, strings.Join(knownProfiles, ","))
		}
		out[name] = true
	}
	if len(out) == 0 {
		return nil, fmt.Errorf("empty profiles after parse")
	}
	return out, nil
}

func wantProfile(want map[string]bool, name string) bool {
	return want["all"] || want[name]
}

func selectedProfileNames(want map[string]bool) []string {
	if want["all"] {
		return append([]string{}, knownProfiles...)
	}
	var out []string
	for _, p := range knownProfiles {
		if want[p] {
			out = append(out, p)
		}
	}
	return out
}

func usesGroupCommit(opts *f4kvs.OpenOptions) bool {
	if opts == nil {
		return false
	}
	return opts.GroupCommitEnabled || opts.WalDurability == f4kvs.WalDurabilityAmortized
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
	skipPerCommitChunks bool,
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

	if !skipPerCommitChunks {
		fmt.Fprintf(os.Stderr, "[%s] chunk_batch_put (%d, per-commit)...\n", profile, len(chunkKeys))
		t0 = time.Now()
		for _, key := range chunkKeys {
			if err := engine.PutBytes(key, chunkPayload); err != nil {
				fatal(err)
			}
		}
		out = append(out, result("chunk_batch_put", profile, len(chunkKeys), time.Since(t0), durability, "per-commit"))

		if usesGroupCommit(opts) {
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
	} else {
		// Load main engine via chunked BatchPut so prefix scan / random get see product-scale data.
		// Engine hard-limit is 10_000 items per BatchPutBytes (DoS guard).
		// Bulk-import skips per-key SSTable probes — product RAG ingest shape.
		if err := engine.SetBulkImport(true); err != nil {
			fatal(err)
		}
		fmt.Fprintf(os.Stderr, "[%s] chunk_batch_put_batched into main engine (%d, chunked bulk)...\n", profile, len(chunkKeys))
		t0 = time.Now()
		if err := batchPutBytesChunked(engine, chunkKeys, chunkPayload); err != nil {
			fatal(err)
		}
		if err := engine.SetBulkImport(false); err != nil {
			fatal(err)
		}
		if usesGroupCommit(opts) {
			if err := engine.FlushWAL(); err != nil {
				fatal(err)
			}
		}
		out = append(out, result(
			"chunk_batch_put_batched", profile, len(chunkKeys), time.Since(t0), durability,
			"BatchPutBytes chunked≤10k + SetBulkImport (scale path)",
		))
	}

	// Side-dir batch/bulk only when we did not already time BatchPut on the main engine.
	if !skipPerCommitChunks {
		out = append(out, benchF4KVSChunkBatched(filepath.Join(dir, "chunk_batched"), profile, chunkKeys, chunkPayload)...)
		out = append(out, benchF4KVSChunkBulkImport(filepath.Join(dir, "chunk_bulk_import"), profile, chunkKeys, chunkPayload)...)
	} else {
		// Optional bulk_import side measurement at meso (separate dir, one extra 100k load).
		out = append(out, benchF4KVSChunkBulkImport(filepath.Join(dir, "chunk_bulk_import"), profile, chunkKeys, chunkPayload)...)
	}

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
	return benchF4KVSChunkBatchPut(dir, profile, chunkKeys, chunkPayload, false, "chunk_batch_put_batched", "BatchPutBytes")
}

func benchF4KVSChunkBulkImport(dir, profile string, chunkKeys []string, chunkPayload []byte) []phaseResult {
	return benchF4KVSChunkBatchPut(dir, profile, chunkKeys, chunkPayload, true, "chunk_batch_put_bulk_import", "BatchPutBytes + SetBulkImport(true)")
}

// maxBatchPutItems: engine DoS max is 10_000; use 500 for meso stability
// (deadlock_repro style; 10k×4KB batches were observed to stall after first slice).
const maxBatchPutItems = 500

func batchPutBytesChunked(engine *f4kvs.F4KVS, keys []string, payload []byte) error {
	for i := 0; i < len(keys); i += maxBatchPutItems {
		end := i + maxBatchPutItems
		if end > len(keys) {
			end = len(keys)
		}
		items := make(map[string][]byte, end-i)
		for _, key := range keys[i:end] {
			items[key] = payload
		}
		if err := engine.BatchPutBytes(items); err != nil {
			return err
		}
		fmt.Fprintf(os.Stderr, "  … batch_put %d/%d\n", end, len(keys))
	}
	return nil
}

func benchF4KVSChunkBatchPut(
	dir, profile string,
	chunkKeys []string,
	chunkPayload []byte,
	bulkImport bool,
	phase, note string,
) []phaseResult {
	const durability = "WAL + WalSyncMode::Fsync (one fsync per BatchPutBytes)"
	var out []phaseResult

	engine, err := f4kvs.NewPersistentEngine(dir)
	if err != nil {
		fatal(err)
	}
	defer engine.Close()

	if bulkImport {
		if err := engine.SetBulkImport(true); err != nil {
			fatal(err)
		}
	}

	fmt.Fprintf(os.Stderr, "[%s] %s (%d, %s)...\n", profile, phase, len(chunkKeys), note)
	t0 := time.Now()
	if err := batchPutBytesChunked(engine, chunkKeys, chunkPayload); err != nil {
		fatal(err)
	}
	out = append(out, result(phase, profile, len(chunkKeys), time.Since(t0), durability, note+" (chunked≤10k)"))

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

// metricResult emits a single long-format metric (DE export) and shows in the table.
func metricResult(phase, profile, metric string, value float64, ops int, unit, notes string) phaseResult {
	return phaseResult{
		Phase: phase, Profile: profile, Ops: ops,
		MetricOnly: metric, Value: value, Unit: unit, Extra: notes,
		// Populate Ms when metric is duration for table readability.
		Ms: func() float64 {
			if metric == "duration_ms" {
				return value
			}
			return 0
		}(),
	}
}

func samplePayload(n int) []byte {
	return samplePayloadSeeded(n, 0)
}

func samplePayloadSeeded(n int, seed int64) []byte {
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
	// Deterministic fill from seed (keys are already deterministic; this pins payload bytes).
	// Fall back to crypto entropy only when seed==0 and we want variety (legacy path).
	var next func() byte
	if seed != 0 {
		state := uint64(seed)
		next = func() byte {
			// xorshift64*
			state ^= state << 13
			state ^= state >> 7
			state ^= state << 17
			return byte(state % 26)
		}
	} else {
		next = func() byte {
			var b [1]byte
			_, _ = rand.Read(b[:])
			return b[0] % 26
		}
	}
	for i := 0; i < fill; i++ {
		out[len(head)+i] = 'a' + next()
	}
	copy(out[len(head)+fill:], tail)
	return out
}

// benchF4KVSPostRestart ingests memoirs+chunks, closes, reopens, counts keys.
func benchF4KVSPostRestart(
	dir, profile string,
	opts *f4kvs.OpenOptions,
	memoirKeys, chunkKeys []string,
	memoirPayload, chunkPayload []byte,
) []phaseResult {
	expected := len(memoirKeys) + len(chunkKeys)
	_ = os.RemoveAll(dir)

	engine, err := f4kvs.NewPersistentEngineWithOptions(dir, opts)
	if err != nil {
		fatal(err)
	}

	// Memoirs first (small), then chunks in ≤10k BatchPut slices (+ bulk-import at scale).
	if len(memoirKeys) > 0 {
		items := make(map[string][]byte, len(memoirKeys))
		for _, k := range memoirKeys {
			items[k] = memoirPayload
		}
		if err := engine.BatchPutBytes(items); err != nil {
			engine.Close()
			fatal(err)
		}
	}
	if len(chunkKeys) > maxBatchPutItems {
		if err := engine.SetBulkImport(true); err != nil {
			engine.Close()
			fatal(err)
		}
	}
	if err := batchPutBytesChunked(engine, chunkKeys, chunkPayload); err != nil {
		engine.Close()
		fatal(err)
	}
	if len(chunkKeys) > maxBatchPutItems {
		_ = engine.SetBulkImport(false)
	}
	if usesGroupCommit(opts) {
		if err := engine.FlushWAL(); err != nil {
			engine.Close()
			fatal(err)
		}
	}
	engine.Close()

	t0 := time.Now()
	reopened, err := f4kvs.NewPersistentEngineWithOptions(dir, opts)
	if err != nil {
		fatal(err)
	}
	memoirs := reopened.ScanPrefixKeys("memoir:")
	chunks := reopened.ScanPrefixKeys("chunk:")
	counted := len(memoirs) + len(chunks)
	elapsed := time.Since(t0)
	reopened.Close()

	ms := float64(elapsed.Microseconds()) / 1000.0
	integrity := 0.0
	if counted == expected {
		integrity = 1
	}
	notes := fmt.Sprintf("expected=%d counted=%d", expected, counted)
	fmt.Fprintf(os.Stderr, "[%s] post_restart_row_count: counted=%d expected=%d integrity_ok=%.0f (%.1f ms)\n",
		profile, counted, expected, integrity, ms)

	return []phaseResult{
		metricResult("post_restart_row_count", profile, "duration_ms", ms, 0, "ms", "reopen+count"),
		metricResult("post_restart_row_count", profile, "row_count", float64(counted), 0, "count", notes),
		metricResult("post_restart_row_count", profile, "expected_row_count", float64(expected), 0, "count", notes),
		metricResult("post_restart_row_count", profile, "integrity_ok", integrity, 0, "bool", "1=pass 0=fail"),
	}
}

// benchSQLitePostRestart mirrors f4kvs restart integrity for sqlite.
func benchSQLitePostRestart(
	path string,
	prof sqliteProfile,
	memoirKeys, chunkKeys []string,
	memoirPayload, chunkPayload []byte,
) []phaseResult {
	expected := len(memoirKeys) + len(chunkKeys)
	_ = os.Remove(path)

	dsn := strings.Replace(prof.DSN, "file:kv?", "file:"+path+"?", 1)
	db, err := sql.Open("sqlite", dsn)
	if err != nil {
		fatal(err)
	}
	db.SetMaxOpenConns(1)
	if _, err := db.Exec(`CREATE TABLE kv (
		key TEXT PRIMARY KEY,
		value BLOB NOT NULL
	) WITHOUT ROWID`); err != nil {
		db.Close()
		fatal(err)
	}
	// Batched ingest (integrity, not perf).
	if err := sqliteBatchPut(db, false, memoirKeys, memoirPayload); err != nil {
		db.Close()
		fatal(err)
	}
	if err := sqliteBatchPut(db, false, chunkKeys, chunkPayload); err != nil {
		db.Close()
		fatal(err)
	}
	if err := db.Close(); err != nil {
		fatal(err)
	}

	t0 := time.Now()
	db2, err := sql.Open("sqlite", dsn)
	if err != nil {
		fatal(err)
	}
	db2.SetMaxOpenConns(1)
	var counted int
	if err := db2.QueryRow(`SELECT COUNT(*) FROM kv`).Scan(&counted); err != nil {
		db2.Close()
		fatal(err)
	}
	elapsed := time.Since(t0)
	db2.Close()

	ms := float64(elapsed.Microseconds()) / 1000.0
	integrity := 0.0
	if counted == expected {
		integrity = 1
	}
	notes := fmt.Sprintf("expected=%d counted=%d", expected, counted)
	fmt.Fprintf(os.Stderr, "[%s] post_restart_row_count: counted=%d expected=%d integrity_ok=%.0f (%.1f ms)\n",
		prof.Name, counted, expected, integrity, ms)

	return []phaseResult{
		metricResult("post_restart_row_count", prof.Name, "duration_ms", ms, 0, "ms", "reopen+count"),
		metricResult("post_restart_row_count", prof.Name, "row_count", float64(counted), 0, "count", notes),
		metricResult("post_restart_row_count", prof.Name, "expected_row_count", float64(expected), 0, "count", notes),
		metricResult("post_restart_row_count", prof.Name, "integrity_ok", integrity, 0, "bool", "1=pass 0=fail"),
	}
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
			if byPhase[phase][i].Profile != byPhase[phase][j].Profile {
				return byPhase[phase][i].Profile < byPhase[phase][j].Profile
			}
			return byPhase[phase][i].MetricOnly < byPhase[phase][j].MetricOnly
		})
		for _, r := range byPhase[phase] {
			note := r.Extra
			if note == "" {
				note = r.Durability
			}
			if r.MetricOnly != "" {
				fmt.Printf("%-22s %-18s %8d %12.1f %12s %s=%g %s\n",
					r.Phase, r.Profile, r.Ops, r.Ms, r.Unit, r.MetricOnly, r.Value, note)
				continue
			}
			fmt.Printf("%-22s %-18s %8d %12.1f %12.0f %s\n", r.Phase, r.Profile, r.Ops, r.Ms, r.OpsPerS, note)
		}
		for _, cmp := range phaseCompares(byPhase, byPhase[phase], phase) {
			fmt.Printf("  → %s: %s\n", cmp.label, cmp.line)
		}
	}
}

type compareLine struct {
	label string
	line  string
}

func phaseCompares(byPhase map[string][]phaseResult, rows []phaseResult, phase string) []compareLine {
	var out []compareLine
	if line := ratioLine(rows, "f4kvs_wal_segment", "sqlite_wal_full"); line != "" {
		label := "fair compare (segment)"
		if phase == "chunk_batch_put_batched" || phase == "chunk_batch_put_bulk_import" {
			label = "batched compare"
		}
		out = append(out, compareLine{label: label, line: line})
	}
	if line := ratioLine(rows, "f4kvs_wal_indexed", "sqlite_wal_full"); line != "" {
		label := "fair compare (indexed v2)"
		if phase == "chunk_batch_put_batched" || phase == "chunk_batch_put_bulk_import" {
			label = "batched compare (indexed)"
		}
		out = append(out, compareLine{label: label, line: line})
	}
	if phase == "chunk_batch_put" {
		if seg, idx := phaseMs(rows, "f4kvs_wal_segment"), phaseMs(rows, "f4kvs_wal_indexed"); seg > 0 && idx > 0 {
			out = append(out, compareLine{
				label: "indexed v2 vs segment",
				line:  speedRatioLine("f4kvs_wal_indexed", idx, "f4kvs_wal_segment", seg),
			})
		}
	}
	if line := ratioLine(rows, "f4kvs_wal_frame", "sqlite_wal_full"); line != "" {
		out = append(out, compareLine{label: "fair compare (frame)", line: line})
	}
	if line := ratioLine(rows, "f4kvs_wal_segment", "f4kvs_wal_frame"); line != "" {
		out = append(out, compareLine{label: "segment vs frame", line: line})
	}
	if phase == "chunk_batch_put_bulk_import" {
		for _, profile := range []string{"f4kvs_wal_segment", "f4kvs_wal_frame", "f4kvs_group_commit_10ms"} {
			batched := phaseMs(byPhase["chunk_batch_put_batched"], profile)
			bulk := phaseMs(rows, profile)
			if batched > 0 && bulk > 0 {
				out = append(out, compareLine{
					label: "bulk-import vs batch (" + profile + ")",
					line:  speedRatioLine("chunk_batch_put_batched", batched, "chunk_batch_put_bulk_import", bulk),
				})
			}
		}
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

func phaseMs(rows []phaseResult, profile string) float64 {
	for _, r := range rows {
		if r.Profile == profile {
			return r.Ms
		}
	}
	return 0
}

func speedRatioLine(fasterName string, fasterMs float64, slowerName string, slowerMs float64) string {
	if fasterMs == 0 || slowerMs == 0 {
		return ""
	}
	if fasterMs > slowerMs {
		return fmt.Sprintf("%s %.1f× faster than %s", slowerName, fasterMs/slowerMs, fasterName)
	}
	return fmt.Sprintf("%s %.1f× faster than %s", fasterName, slowerMs/fasterMs, slowerName)
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
