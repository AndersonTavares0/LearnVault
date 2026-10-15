package config

import (
	"fmt"
	"os"
	"path/filepath"
	"strconv"
)

type Config struct {
	ListenAddr   string
	Port         string
	DataDir      string
	CacheDir     string
	LibraryDir   string
	DatabasePath string
	AdminUser    string
	AdminPass    string
	CookieSecure bool
}

func Load() (Config, error) {
	cfg := Config{
		ListenAddr:   env("LISTEN_ADDR", "0.0.0.0"),
		Port:         env("PORT", "8090"),
		DataDir:      env("DATA_DIR", "./data"),
		CacheDir:     env("CACHE_DIR", "./cache"),
		LibraryDir:   env("LIBRARY_DIR", "./library"),
		AdminUser:    os.Getenv("ADMIN_USERNAME"),
		AdminPass:    os.Getenv("ADMIN_PASSWORD"),
		CookieSecure: env("COOKIE_SECURE", "true") != "false",
	}
	port, err := strconv.Atoi(cfg.Port)
	if err != nil || port < 1 || port > 65535 {
		return Config{}, fmt.Errorf("PORT must be an integer between 1 and 65535")
	}
	cfg.DatabasePath = filepath.Join(cfg.DataDir, "learnvault.sqlite")
	return cfg, nil
}

func env(key, fallback string) string {
	if value := os.Getenv(key); value != "" {
		return value
	}
	return fallback
}
