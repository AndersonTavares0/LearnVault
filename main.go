package main

import (
	"context"
	"embed"
	"errors"
	"io"
	"io/fs"
	"log"
	"net/http"
	"os"
	"os/signal"
	"path/filepath"
	"strings"
	"syscall"
	"time"

	"learnvault/internal/config"
	"learnvault/internal/httpapi"
	"learnvault/internal/library"
	"learnvault/internal/storage"
)

//go:embed web/build/*
var webBuild embed.FS

func main() {
	cfg, err := config.Load()
	if err != nil {
		log.Fatal(err)
	}
	root, err := library.Open(cfg.LibraryDir)
	if err != nil {
		log.Fatalf("open read-only library mount: %v", err)
	}
	for _, dir := range []string{cfg.DataDir, cfg.CacheDir} {
		if err := os.MkdirAll(dir, 0o700); err != nil {
			log.Fatal(err)
		}
	}
	db, err := storage.Open(context.Background(), cfg.DatabasePath)
	if err != nil {
		log.Fatal(err)
	}
	defer db.Close()

	hasAdmin, err := db.HasAdmin(context.Background())
	if err != nil {
		log.Fatal(err)
	}
	if !hasAdmin {
		if cfg.AdminUser == "" || cfg.AdminPass == "" {
			log.Fatal("first run requires ADMIN_USERNAME and ADMIN_PASSWORD (minimum 12 characters)")
		}
		if err := db.BootstrapAdmin(context.Background(), cfg.AdminUser, cfg.AdminPass); err != nil {
			log.Fatal(err)
		}
		log.Printf("bootstrapped initial admin account %q", cfg.AdminUser)
	}

	staticFiles, err := fs.Sub(webBuild, "web/build")
	if err != nil {
		log.Fatal(err)
	}
	api := httpapi.NewWithLibrary(db, cfg.CookieSecure, root)
	mux := http.NewServeMux()
	mux.Handle("/api/", api)
	mux.HandleFunc("/", spaHandler(staticFiles))
	server := &http.Server{Addr: cfg.ListenAddr + ":" + cfg.Port, Handler: securityHeaders(mux), ReadHeaderTimeout: 5 * time.Second, IdleTimeout: 60 * time.Second}

	ctx, stop := signal.NotifyContext(context.Background(), os.Interrupt, syscall.SIGTERM)
	defer stop()
	go func() {
		<-ctx.Done()
		shutdownCtx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
		defer cancel()
		_ = server.Shutdown(shutdownCtx)
	}()
	log.Printf("LearnVault listening on %s", server.Addr)
	if err := server.ListenAndServe(); err != nil && !errors.Is(err, http.ErrServerClosed) {
		log.Fatal(err)
	}
}

func spaHandler(files fs.FS) http.HandlerFunc {
	fileServer := http.FileServer(http.FS(files))
	return func(w http.ResponseWriter, r *http.Request) {
		if strings.HasPrefix(r.URL.Path, "/api/") {
			http.NotFound(w, r)
			return
		}
		path := strings.TrimPrefix(filepath.ToSlash(filepath.Clean("/"+r.URL.Path)), "/")
		if path == "." {
			path = ""
		}
		if path != "" {
			if f, err := files.Open(path); err == nil {
				_ = f.Close()
				fileServer.ServeHTTP(w, r)
				return
			}
			if strings.Contains(filepath.Base(path), ".") {
				http.NotFound(w, r)
				return
			}
		}
		fallback, err := files.Open("200.html")
		if err != nil {
			http.Error(w, "frontend assets unavailable", http.StatusServiceUnavailable)
			return
		}
		defer fallback.Close()
		contents, err := io.ReadAll(fallback)
		if err != nil {
			http.Error(w, "frontend assets unavailable", http.StatusInternalServerError)
			return
		}
		w.Header().Set("Content-Type", "text/html; charset=utf-8")
		w.WriteHeader(http.StatusOK)
		_, _ = w.Write(contents)
	}
}

func securityHeaders(next http.Handler) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("X-Content-Type-Options", "nosniff")
		w.Header().Set("X-Frame-Options", "DENY")
		w.Header().Set("Referrer-Policy", "no-referrer")
		next.ServeHTTP(w, r)
	})
}
