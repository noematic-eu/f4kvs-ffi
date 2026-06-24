//go:build ignore

package main

import (
	"database/sql"
	"fmt"
	"os"
	"strings"
	"time"

	_ "modernc.org/sqlite"
)

func main() {
	n := 2000
	payload := make([]byte, 4096)
	for i := range payload {
		payload[i] = 'x'
	}

	profiles := []struct {
		name string
		dsn  string
	}{
		{"wal_full", "file:kv?_pragma=journal_mode(WAL)&_pragma=synchronous(FULL)"},
		{"wal_normal", "file:kv?_pragma=journal_mode(WAL)&_pragma=synchronous(NORMAL)"},
		{"delete_full", "file:kv?_pragma=journal_mode(DELETE)&_pragma=synchronous(FULL)"},
	}

	for _, prof := range profiles {
		dir, err := os.MkdirTemp("", "sqlite-probe-*")
		if err != nil {
			panic(err)
		}
		defer os.RemoveAll(dir)

		dsn := strings.Replace(prof.dsn, "file:kv?", "file:"+dir+"/kv?", 1)
		db, err := sql.Open("sqlite", dsn)
		if err != nil {
			panic(err)
		}
		db.SetMaxOpenConns(1)
		if _, err := db.Exec(`CREATE TABLE kv (key TEXT PRIMARY KEY, value BLOB NOT NULL) WITHOUT ROWID`); err != nil {
			panic(err)
		}

		t0 := time.Now()
		for i := 0; i < n; i++ {
			key := fmt.Sprintf("chunk:legal:doc-%04d:chunk-%06d", i/10, i)
			if _, err := db.Exec(`INSERT INTO kv (key, value) VALUES (?, ?)`, key, payload); err != nil {
				panic(err)
			}
		}
		ms := time.Since(t0).Seconds() * 1000
		fmt.Printf("%s: %.1f ms (%.3f ms/op)\n", prof.name, ms, ms/float64(n))
		db.Close()
	}
}