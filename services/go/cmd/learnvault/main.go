package main

import (
	"context"
	"embed"
	"errors"
	"fmt"
	"io"
	"io/fs"
	"log"
	"net"
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
var version = "dev"

func main() {
	if printVersion(os.Args, os.Stdout) {
		return
	}
	ctx, stop := signal.NotifyContext(context.Background(), os.Interrupt, syscall.SIGTERM)
	defer stop()
	cfg, err := config.Load()
	if err != nil {
		log.Fatal(err)
	}
	if err := run(ctx, cfg, net.Listen); err != nil {
		log.Fatal(err)
	}
}

func printVersion(args []string, out io.Writer) bool {
	if len(args) == 2 && args[1] == "--version" {
		fmt.Fprintln(out, version)
		return true
	}
	return false
}

func run(ctx context.Context, cfg config.Config, listen func(string, string) (net.Listener, error)) error {
	root, err := library.Open(cfg.LibraryDir)
	if err != nil {
		return err
	}
	for _, dir := range []string{cfg.DataDir, cfg.CacheDir} {
		if err := os.MkdirAll(dir, 0o700); err != nil {
			return err
		}
	}
	db, err := storage.Open(ctx, cfg.DatabasePath)
	if err != nil {
		return err
	}
	defer db.Close()

	hasAdmin, err := db.HasAdmin(ctx)
	if err != nil {
		return err
	}
	if !hasAdmin {
		if cfg.AdminUser == "" { cfg.AdminUser = "admin" }
		if cfg.AdminPass == "" { cfg.AdminPass = "admin" }
		if err := db.BootstrapAdmin(ctx, cfg.AdminUser, cfg.AdminPass); err != nil {
			return err
		}
		log.Printf("bootstrapped initial admin account %q", cfg.AdminUser)
	}

	staticFiles, err := fs.Sub(webBuild, "web/build")
	if err != nil {
		return err
	}
	api := httpapi.NewWithLibraryVersion(db, cfg.CookieSecure, root, version)
	mux := http.NewServeMux()
	mux.Handle("/api/", api)
	mux.HandleFunc("/", spaHandler(staticFiles))
	address := cfg.ListenAddr + ":" + cfg.Port
	listener, err := listen("tcp", address)
	if err != nil {
		return err
	}
	server := &http.Server{Addr: address, Handler: securityHeaders(mux), ReadHeaderTimeout: 5 * time.Second, IdleTimeout: 60 * time.Second}

	serverCtx, stop := context.WithCancel(ctx)
	defer stop()
	go func() {
		<-serverCtx.Done()
		shutdownCtx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
		defer cancel()
		_ = server.Shutdown(shutdownCtx)
	}()
	log.Printf("LearnVault listening on %s", server.Addr)
	if err := server.Serve(listener); err != nil && !errors.Is(err, http.ErrServerClosed) {
		return err
	}
	return nil
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
		w.Header().Set("Content-Security-Policy", "default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' data:; font-src 'self'; media-src 'self' data: blob:; object-src 'none'; frame-ancestors 'none'")
		next.ServeHTTP(w, r)
	})
}
