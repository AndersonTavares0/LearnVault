package main

import (
	"bytes"
	"context"
	"errors"
	"net"
	"net/http"
	"os"
	"path/filepath"
	"sync"
	"testing"
	"time"

	"learnvault/internal/config"
)

func TestRunRejectsInvalidInitialAdminCredentials(t *testing.T) {
	cfg := testRunConfig(t)
	cfg.AdminUser = "owner"
	cfg.AdminPass = "short"
	listenCalled := false
	err := run(context.Background(), cfg, func(string, string) (net.Listener, error) {
		listenCalled = true
		return nil, errors.New("listener should not be created")
	})
	if err == nil || listenCalled {
		t.Fatalf("run() error = %v, listener called = %v; want bootstrap error before listen", err, listenCalled)
	}
}

func TestRunStartsServerAndShutsDownOnContextCancellation(t *testing.T) {
	cfg := testRunConfig(t)
	cfg.AdminUser = "owner"
	cfg.AdminPass = "a sufficiently long password"
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()
	finished := make(chan error, 1)
	listenerReady := make(chan net.Listener, 1)
	serveReady := make(chan struct{})
	go func() {
		finished <- run(ctx, cfg, func(network, address string) (net.Listener, error) {
			listener, err := net.Listen(network, address)
			if err != nil {
				return nil, err
			}
			ready := &serveReadyListener{Listener: listener, ready: serveReady}
			listenerReady <- ready
			return ready, nil
		})
	}()

	var listener net.Listener
	select {
	case listener = <-listenerReady:
	case err := <-finished:
		t.Fatalf("run() before server became ready = %v", err)
	}
	select {
	case <-serveReady:
	case err := <-finished:
		t.Fatalf("server stopped before accepting connections: %v", err)
	}
	client := &http.Client{Timeout: 3 * time.Second}
	response, err := client.Get("http://" + listener.Addr().String() + "/api/healthz")
	if err != nil {
		t.Fatalf("server did not start: %v", err)
	}
	response.Body.Close()
	if response.StatusCode != http.StatusOK {
		t.Fatalf("health status = %d; want %d", response.StatusCode, http.StatusOK)
	}

	cancel()
	select {
	case err := <-finished:
		if err != nil {
			t.Fatalf("run() after cancellation = %v; want nil", err)
		}
	case <-time.After(3 * time.Second):
		t.Fatal("server did not stop after context cancellation")
	}
}

type serveReadyListener struct {
	net.Listener
	ready chan<- struct{}
	once  sync.Once
}

func (l *serveReadyListener) Accept() (net.Conn, error) {
	l.once.Do(func() { close(l.ready) })
	return l.Listener.Accept()
}

func TestRunReturnsListenErrors(t *testing.T) {
	cfg := testRunConfig(t)
	cfg.AdminUser = "owner"
	cfg.AdminPass = "a sufficiently long password"
	want := errors.New("listen failed")
	got := run(context.Background(), cfg, func(string, string) (net.Listener, error) { return nil, want })
	if !errors.Is(got, want) {
		t.Fatalf("run() error = %v; want %v", got, want)
	}
}

func TestVersionDefaultsToDevelopment(t *testing.T) {
	if version != "dev" {
		t.Fatalf("version = %q; want dev", version)
	}
}

func TestPrintVersion(t *testing.T) {
	var output bytes.Buffer
	if !printVersion([]string{"learnvault", "--version"}, &output) {
		t.Fatal("printVersion() did not handle --version")
	}
	if output.String() != "dev\n" {
		t.Fatalf("version output = %q; want dev", output.String())
	}
	if printVersion([]string{"learnvault"}, &output) {
		t.Fatal("printVersion() handled missing flag")
	}
}

func testRunConfig(t *testing.T) config.Config {
	t.Helper()
	root := t.TempDir()
	library := filepath.Join(root, "library")
	if err := os.MkdirAll(library, 0o755); err != nil {
		t.Fatal(err)
	}
	return config.Config{
		ListenAddr: "127.0.0.1", Port: "0",
		DataDir: filepath.Join(root, "data"), CacheDir: filepath.Join(root, "cache"),
		LibraryDir: library, DatabasePath: filepath.Join(root, "data", "learnvault.sqlite"),
		CookieSecure: false,
	}
}
