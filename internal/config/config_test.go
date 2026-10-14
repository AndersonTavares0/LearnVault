package config

import "testing"

func TestLoadUsesDefaultsAndEnvironment(t *testing.T) {
	t.Setenv("PORT", "9180")
	t.Setenv("LISTEN_ADDR", "127.0.0.1")
	t.Setenv("DATA_DIR", t.TempDir())
	t.Setenv("CACHE_DIR", t.TempDir())
	t.Setenv("LIBRARY_DIR", t.TempDir())

	cfg, err := Load()
	if err != nil {
		t.Fatal(err)
	}
	if cfg.Port != "9180" || cfg.ListenAddr != "127.0.0.1" {
		t.Fatalf("environment not applied: %+v", cfg)
	}
	if cfg.DatabasePath == "" || cfg.LibraryDir == "" {
		t.Fatalf("missing derived config: %+v", cfg)
	}
}

func TestLoadRejectsInvalidPort(t *testing.T) {
	t.Setenv("PORT", "70000")
	if _, err := Load(); err == nil {
		t.Fatal("expected invalid port to be rejected")
	}
}
