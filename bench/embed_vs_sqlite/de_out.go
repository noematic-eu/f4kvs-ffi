package main

import (
	"bufio"
	"encoding/json"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"runtime"
	"strconv"
	"strings"
	"time"
)

const (
	schemaVersion = "bench-schema-v1"
	flowID        = "rag_chunk_ingest_v1"
	harnessPath   = "bench/embed_vs_sqlite"
)

// RunManifest is the single-object metadata for a DE bench run (bench-schema-v1).
type RunManifest struct {
	SchemaVersion string      `json:"schema_version"`
	RunID         string      `json:"run_id"`
	FlowID        string      `json:"flow_id"`
	Tier          string      `json:"tier"`
	Seed          int         `json:"seed"`
	StartedAt     string      `json:"started_at"`
	FinishedAt    string      `json:"finished_at"`
	Git           GitInfo     `json:"git"`
	Host          HostInfo    `json:"host"`
	Scale         ScaleInfo   `json:"scale"`
	Engines       []string    `json:"engines"`
	Harness       HarnessInfo `json:"harness"`
	Notes         string      `json:"notes"`
}

type GitInfo struct {
	F4KVSFFI string `json:"f4kvs_ffi"`
	F4KVSLSM string `json:"f4kvs_lsm"`
	Dirty    bool   `json:"dirty"`
}

type HostInfo struct {
	OS       string `json:"os"`
	Arch     string `json:"arch"`
	Hostname string `json:"hostname"`
	Go       string `json:"go"`
	CPUModel string `json:"cpu_model"`
	CPUCores int    `json:"cpu_cores"`
}

type ScaleInfo struct {
	Memoirs     int `json:"memoirs"`
	Chunks      int `json:"chunks"`
	MemoirBytes int `json:"memoir_bytes"`
	ChunkBytes  int `json:"chunk_bytes"`
	RandomGets  int `json:"random_gets"`
}

type HarnessInfo struct {
	Path    string `json:"path"`
	Command string `json:"command"`
}

// ResultLine is one long-format measurement (one JSONL row).
type ResultLine struct {
	RunID  string  `json:"run_id"`
	Engine string  `json:"engine"`
	Phase  string  `json:"phase"`
	Metric string  `json:"metric"`
	Value  float64 `json:"value"`
	Ops    int     `json:"ops"`
	Unit   string  `json:"unit"`
	Notes  string  `json:"notes"`
}

// RunWriter stages a DE run under runsRoot/{runID}.partial then renames to runsRoot/{runID}.
type RunWriter struct {
	RunsRoot   string
	RunID      string
	PartialDir string
	FinalDir   string
	Manifest   RunManifest
	lines      []ResultLine
	started    time.Time
}

// NewRunWriter creates runsRoot/{runID}.partial/ for atomic finalize.
func NewRunWriter(runsRoot, runID string, manifest RunManifest) (*RunWriter, error) {
	if runsRoot == "" {
		return nil, fmt.Errorf("runs-root is empty")
	}
	if runID == "" {
		return nil, fmt.Errorf("run_id is empty")
	}
	partial := filepath.Join(runsRoot, runID+".partial")
	final := filepath.Join(runsRoot, runID)
	if err := os.RemoveAll(partial); err != nil {
		return nil, err
	}
	if err := os.MkdirAll(partial, 0o755); err != nil {
		return nil, err
	}
	manifest.SchemaVersion = schemaVersion
	manifest.RunID = runID
	manifest.FlowID = flowID
	if manifest.Harness.Path == "" {
		manifest.Harness.Path = harnessPath
	}
	return &RunWriter{
		RunsRoot:   runsRoot,
		RunID:      runID,
		PartialDir: partial,
		FinalDir:   final,
		Manifest:   manifest,
		started:    time.Now().UTC(),
	}, nil
}

// AppendPhaseResults maps legacy phaseResult rows into long-format lines.
func (w *RunWriter) AppendPhaseResults(results []phaseResult) {
	for _, r := range results {
		w.lines = append(w.lines, phaseResultToLong(w.RunID, r)...)
	}
}

// AppendLine adds a single long-format measurement.
func (w *RunWriter) AppendLine(line ResultLine) {
	if line.RunID == "" {
		line.RunID = w.RunID
	}
	w.lines = append(w.lines, line)
}

// Finalize writes manifest.json + results.jsonl, optional legacy report, then renames partial → final.
func (w *RunWriter) Finalize(legacy *report, engines []string) (string, error) {
	finished := time.Now().UTC()
	w.Manifest.StartedAt = w.started.Format(time.RFC3339)
	w.Manifest.FinishedAt = finished.Format(time.RFC3339)
	if len(engines) > 0 {
		w.Manifest.Engines = engines
	}

	manPath := filepath.Join(w.PartialDir, "manifest.json")
	manBytes, err := json.MarshalIndent(w.Manifest, "", "  ")
	if err != nil {
		return "", err
	}
	if err := os.WriteFile(manPath, manBytes, 0o644); err != nil {
		return "", err
	}

	resPath := filepath.Join(w.PartialDir, "results.jsonl")
	f, err := os.Create(resPath)
	if err != nil {
		return "", err
	}
	bw := bufio.NewWriter(f)
	for _, line := range w.lines {
		b, err := json.Marshal(line)
		if err != nil {
			f.Close()
			return "", err
		}
		if _, err := bw.Write(b); err != nil {
			f.Close()
			return "", err
		}
		if err := bw.WriteByte('\n'); err != nil {
			f.Close()
			return "", err
		}
	}
	if err := bw.Flush(); err != nil {
		f.Close()
		return "", err
	}
	if err := f.Close(); err != nil {
		return "", err
	}

	if legacy != nil {
		legPath := filepath.Join(w.PartialDir, "report.legacy.json")
		b, err := json.MarshalIndent(legacy, "", "  ")
		if err != nil {
			return "", err
		}
		if err := os.WriteFile(legPath, b, 0o644); err != nil {
			return "", err
		}
	}

	// Atomic publish: remove any prior final dir, then rename partial → final.
	_ = os.RemoveAll(w.FinalDir)
	if err := os.Rename(w.PartialDir, w.FinalDir); err != nil {
		return "", err
	}
	return w.FinalDir, nil
}

// LineCount returns how many JSONL rows are buffered.
func (w *RunWriter) LineCount() int {
	return len(w.lines)
}

// HasIntegrityFailure reports whether any integrity_ok metric is 0.
func (w *RunWriter) HasIntegrityFailure() bool {
	for _, line := range w.lines {
		if line.Phase == "post_restart_row_count" && line.Metric == "integrity_ok" && line.Value == 0 {
			return true
		}
	}
	return false
}

func phaseResultToLong(runID string, r phaseResult) []ResultLine {
	notes := r.Extra
	if notes == "" {
		notes = r.Durability
	}

	// Custom single-metric rows (e.g. post_restart_row_count).
	if r.MetricOnly != "" {
		unit := r.Unit
		if unit == "" {
			unit = metricUnit(r.MetricOnly)
		}
		return []ResultLine{{
			RunID:  runID,
			Engine: r.Profile,
			Phase:  r.Phase,
			Metric: r.MetricOnly,
			Value:  r.Value,
			Ops:    r.Ops,
			Unit:   unit,
			Notes:  notes,
		}}
	}

	// Default throughput phase → duration_ms + ops_per_s.
	return []ResultLine{
		{
			RunID:  runID,
			Engine: r.Profile,
			Phase:  r.Phase,
			Metric: "duration_ms",
			Value:  r.Ms,
			Ops:    r.Ops,
			Unit:   "ms",
			Notes:  notes,
		},
		{
			RunID:  runID,
			Engine: r.Profile,
			Phase:  r.Phase,
			Metric: "ops_per_s",
			Value:  r.OpsPerS,
			Ops:    r.Ops,
			Unit:   "ops/s",
			Notes:  notes,
		},
	}
}

func metricUnit(metric string) string {
	switch metric {
	case "duration_ms":
		return "ms"
	case "ops_per_s":
		return "ops/s"
	case "row_count", "expected_row_count":
		return "count"
	case "integrity_ok":
		return "bool"
	case "bytes_on_disk":
		return "bytes"
	default:
		return ""
	}
}

// NewRunID returns UTC compact ISO-like id: 20260718T102530Z
func NewRunID(t time.Time) string {
	return t.UTC().Format("20060102T150405Z")
}

// DeriveTier maps chunk count to micro/meso/macro.
func DeriveTier(chunks int) string {
	switch {
	case chunks <= 2000:
		return "micro"
	case chunks <= 100_000:
		return "meso"
	default:
		return "macro"
	}
}

// CollectGitInfo fills f4kvs-ffi / f4kvs-lsm shas and dirty bit.
func CollectGitInfo(repoRoot string) GitInfo {
	info := GitInfo{
		F4KVSFFI: gitShortSHA(repoRoot),
		Dirty:    gitDirty(repoRoot),
	}
	if ref := strings.TrimSpace(os.Getenv("F4KVS_LSM_REF")); ref != "" {
		info.F4KVSLSM = ref
		return info
	}
	// Prefer local sibling checkout (path patch), else Cargo.toml tag.
	lsmSibling := filepath.Clean(filepath.Join(repoRoot, "..", "f4kvs-lsm"))
	if sha := gitShortSHA(lsmSibling); sha != "" && sha != "unknown" {
		info.F4KVSLSM = sha
		return info
	}
	if tag := cargoLSMPin(filepath.Join(repoRoot, "Cargo.toml")); tag != "" {
		info.F4KVSLSM = tag
		return info
	}
	info.F4KVSLSM = "unknown"
	return info
}

func gitShortSHA(dir string) string {
	if dir == "" {
		return "unknown"
	}
	cmd := exec.Command("git", "-C", dir, "rev-parse", "--short", "HEAD")
	out, err := cmd.Output()
	if err != nil {
		return "unknown"
	}
	return strings.TrimSpace(string(out))
}

func gitDirty(dir string) bool {
	if dir == "" {
		return false
	}
	cmd := exec.Command("git", "-C", dir, "status", "--porcelain")
	out, err := cmd.Output()
	if err != nil {
		return false
	}
	return len(strings.TrimSpace(string(out))) > 0
}

// cargoLSMPin extracts tag = "vX.Y.Z" from f4kvs-lsm git dep lines in Cargo.toml.
func cargoLSMPin(cargoPath string) string {
	b, err := os.ReadFile(cargoPath)
	if err != nil {
		return ""
	}
	for _, line := range strings.Split(string(b), "\n") {
		if !strings.Contains(line, "f4kvs-lsm") && !strings.Contains(line, "github.com/noematic-eu/f4kvs-lsm") {
			continue
		}
		if i := strings.Index(line, `tag = "`); i >= 0 {
			rest := line[i+len(`tag = "`):]
			if j := strings.Index(rest, `"`); j >= 0 {
				return rest[:j]
			}
		}
	}
	return ""
}

// CollectHostInfo fills OS/arch/hostname/go/cpu.
func CollectHostInfo() HostInfo {
	hn, _ := os.Hostname()
	return HostInfo{
		OS:       runtime.GOOS,
		Arch:     runtime.GOARCH,
		Hostname: hn,
		Go:       runtime.Version(),
		CPUModel: cpuModel(),
		CPUCores: runtime.NumCPU(),
	}
}

func cpuModel() string {
	switch runtime.GOOS {
	case "darwin":
		out, err := exec.Command("sysctl", "-n", "machdep.cpu.brand_string").Output()
		if err == nil {
			return strings.TrimSpace(string(out))
		}
	case "linux":
		b, err := os.ReadFile("/proc/cpuinfo")
		if err == nil {
			for _, line := range strings.Split(string(b), "\n") {
				if strings.HasPrefix(line, "model name") {
					parts := strings.SplitN(line, ":", 2)
					if len(parts) == 2 {
						return strings.TrimSpace(parts[1])
					}
				}
			}
		}
	}
	return "unknown"
}

// FindRepoRoot walks up from start looking for Cargo.toml + crates/f4kvs-ffi.
func FindRepoRoot(start string) string {
	dir := start
	for {
		if fileExists(filepath.Join(dir, "Cargo.toml")) && dirExists(filepath.Join(dir, "crates", "f4kvs-ffi")) {
			return dir
		}
		parent := filepath.Dir(dir)
		if parent == dir {
			return start
		}
		dir = parent
	}
}

func fileExists(p string) bool {
	st, err := os.Stat(p)
	return err == nil && !st.IsDir()
}

func dirExists(p string) bool {
	st, err := os.Stat(p)
	return err == nil && st.IsDir()
}

// UniqueEngines preserves first-seen engine/profile order from results.
func UniqueEngines(results []phaseResult) []string {
	seen := map[string]bool{}
	var out []string
	for _, r := range results {
		if r.Profile == "" || seen[r.Profile] {
			continue
		}
		seen[r.Profile] = true
		out = append(out, r.Profile)
	}
	return out
}

// FormatHarnessCommand rebuilds a readable command string for the manifest.
func FormatHarnessCommand(memoirs, chunks, memoirBytes, chunkBytes, randomGets, seed int, includeRelaxed bool, profiles string) string {
	parts := []string{
		"go run .",
		"-memoirs=" + strconv.Itoa(memoirs),
		"-chunks=" + strconv.Itoa(chunks),
		"-memoir-bytes=" + strconv.Itoa(memoirBytes),
		"-chunk-bytes=" + strconv.Itoa(chunkBytes),
		"-random-gets=" + strconv.Itoa(randomGets),
		"-seed=" + strconv.Itoa(seed),
	}
	if profiles != "" && profiles != "all" {
		parts = append(parts, "-profiles="+profiles)
	}
	if !includeRelaxed {
		parts = append(parts, "-include-relaxed=false")
	}
	return strings.Join(parts, " ")
}
