package library

import (
	"os"
	"path/filepath"
	"testing"
)

func TestReadOnlyLibraryAllowsReadsAndRejectsWrites(t *testing.T) {
	dir := t.TempDir()
	file := filepath.Join(dir, "fixture.txt")
	if err := os.WriteFile(file, []byte("fixture"), 0o400); err != nil {
		t.Fatal(err)
	}
	root, err := Open(dir)
	if err != nil {
		t.Fatal(err)
	}
	got, err := root.ReadFile("fixture.txt")
	if err != nil || string(got) != "fixture" {
		t.Fatalf("read failed: %q %v", got, err)
	}
	if err := root.WriteFile("should-not-exist.txt", []byte("no")); err == nil {
		t.Fatal("write succeeded through read-only library abstraction")
	}
	if _, err := os.Stat(filepath.Join(dir, "should-not-exist.txt")); !os.IsNotExist(err) {
		t.Fatalf("unexpected write on disk: %v", err)
	}
}
