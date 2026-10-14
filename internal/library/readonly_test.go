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

func TestRootPathAndOpenFileStayInsideLibrary(t *testing.T) {
	dir := t.TempDir()
	inside := filepath.Join(dir, "lesson.mp4")
	if err := os.WriteFile(inside, []byte("video fixture"), 0o400); err != nil {
		t.Fatal(err)
	}
	outside := filepath.Join(t.TempDir(), "secret.txt")
	if err := os.WriteFile(outside, []byte("outside"), 0o400); err != nil {
		t.Fatal(err)
	}
	if err := os.Symlink(outside, filepath.Join(dir, "escape")); err != nil {
		t.Skipf("symlink unavailable: %v", err)
	}

	root, err := Open(dir)
	if err != nil {
		t.Fatal(err)
	}
	absolute, err := filepath.Abs(dir)
	if err != nil {
		t.Fatal(err)
	}
	if root.Path() != absolute {
		t.Fatalf("Path() = %q; want %q", root.Path(), absolute)
	}

	file, err := root.OpenFile("lesson.mp4")
	if err != nil {
		t.Fatal(err)
	}
	defer file.Close()
	contents, err := os.ReadFile(file.Name())
	if err != nil || string(contents) != "video fixture" {
		t.Fatalf("OpenFile() contents = %q, %v", contents, err)
	}
	if _, err := root.OpenFile("../secret.txt"); err == nil {
		t.Fatal("OpenFile() allowed traversal outside the library")
	}
	if _, err := root.OpenFile("escape"); err == nil {
		t.Fatal("OpenFile() followed a symlink outside the library")
	}
}
