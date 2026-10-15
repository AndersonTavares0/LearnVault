package main

import (
	"context"
	"io/fs"
	"net/http"
	"net/http/httptest"
	"path/filepath"
	"strings"
	"testing"
	"testing/fstest"

	"learnvault/internal/httpapi"
	"learnvault/internal/storage"
)

func TestSPAFallbackServesEmbeddedShellAtRootAndClientRoutes(t *testing.T) {
	files, err := fs.Sub(webBuild, "web/build")
	if err != nil {
		t.Fatal(err)
	}
	handler := spaHandler(files)
	for _, path := range []string{"/", "/courses/example-course"} {
		recorder := httptest.NewRecorder()
		handler.ServeHTTP(recorder, httptest.NewRequest(http.MethodGet, path, nil))
		body := recorder.Body.String()
		if recorder.Code != http.StatusOK ||
			!strings.Contains(body, `<div id="app"></div>`) ||
			!strings.Contains(body, `<script type="module"`) ||
			!strings.Contains(body, `/_app/immutable/`) {
			t.Fatalf("SPA fallback for %s: status=%d, body=%q", path, recorder.Code, recorder.Body.String())
		}
		if got := recorder.Header().Get("Cache-Control"); got != "no-cache" {
			t.Fatalf("SPA fallback for %s: Cache-Control = %q; want no-cache", path, got)
		}
	}
}

func TestSPAFallbackDoesNotMaskMissingAssetsOrAPI(t *testing.T) {
	files, err := fs.Sub(webBuild, "web/build")
	if err != nil {
		t.Fatal(err)
	}
	handler := spaHandler(files)
	for _, path := range []string{"/missing.js", "/api/healthz", "/api"} {
		recorder := httptest.NewRecorder()
		handler.ServeHTTP(recorder, httptest.NewRequest(http.MethodGet, path, nil))
		if recorder.Code != http.StatusNotFound {
			t.Fatalf("expected missing route %s to return 404, got %d", path, recorder.Code)
		}
	}
}

// TestUnknownNonAPIRoutesAreUncacheable garante que uma rota desconhecida fora de `/api`
// responde 404 com `no-store`, sem cair no shell da SPA nem em um redirect.
func TestUnknownNonAPIRoutesAreUncacheable(t *testing.T) {
	files, err := fs.Sub(webBuild, "web/build")
	if err != nil {
		t.Fatal(err)
	}
	db, err := storage.Open(context.Background(), filepath.Join(t.TempDir(), "db.sqlite"))
	if err != nil {
		t.Fatal(err)
	}
	defer db.Close()
	handler := newMux(httpapi.NewWithOptions(db, false), files)
	for _, path := range []string{"/missing.js", "/nested/missing.css"} {
		recorder := httptest.NewRecorder()
		handler.ServeHTTP(recorder, httptest.NewRequest(http.MethodGet, path, nil))
		if recorder.Code != http.StatusNotFound {
			t.Fatalf("%s status = %d; want 404 (body %s)", path, recorder.Code, recorder.Body.String())
		}
		if got := recorder.Header().Get("Cache-Control"); got != "no-store" {
			t.Fatalf("%s Cache-Control = %q; want no-store", path, got)
		}
		if contentType := recorder.Header().Get("Content-Type"); strings.HasPrefix(contentType, "text/html") {
			t.Fatalf("%s Content-Type = %q; want the 404 body, not the SPA shell", path, contentType)
		}
	}
}

// TestMissingSPAShellIsUncacheable garante que a ausência do shell não seja armazenada,
// para que um 503 obsoleto não prolongue a indisponibilidade.
func TestMissingSPAShellIsUncacheable(t *testing.T) {
	handler := spaHandler(fstest.MapFS{})
	recorder := httptest.NewRecorder()
	handler.ServeHTTP(recorder, httptest.NewRequest(http.MethodGet, "/", nil))
	if recorder.Code != http.StatusServiceUnavailable {
		t.Fatalf("status = %d; want 503 (body %s)", recorder.Code, recorder.Body.String())
	}
	if got := recorder.Header().Get("Cache-Control"); got != "no-store" {
		t.Fatalf("Cache-Control = %q; want no-store", got)
	}
}

// TestBareAPIRouteReachesAPIAndIsUncacheable garante que `/api` sem barra não cai no
// fallback da SPA: a rota é servida pelo handler JSON e recebe `no-store`.
func TestBareAPIRouteReachesAPIAndIsUncacheable(t *testing.T) {
	files, err := fs.Sub(webBuild, "web/build")
	if err != nil {
		t.Fatal(err)
	}
	db, err := storage.Open(context.Background(), filepath.Join(t.TempDir(), "db.sqlite"))
	if err != nil {
		t.Fatal(err)
	}
	defer db.Close()
	handler := newMux(httpapi.NewWithOptions(db, false), files)
	for _, path := range []string{"/api", "/api/unknown"} {
		recorder := httptest.NewRecorder()
		handler.ServeHTTP(recorder, httptest.NewRequest(http.MethodGet, path, nil))
		if recorder.Code != http.StatusNotFound {
			t.Fatalf("%s status = %d; want 404 (body %s)", path, recorder.Code, recorder.Body.String())
		}
		if contentType := recorder.Header().Get("Content-Type"); !strings.HasPrefix(contentType, "application/json") {
			t.Fatalf("%s Content-Type = %q; want JSON, not the SPA shell", path, contentType)
		}
		if got := recorder.Header().Get("Cache-Control"); got != "no-store" {
			t.Fatalf("%s Cache-Control = %q; want no-store", path, got)
		}
	}
}

func TestSecurityHeadersWrapResponse(t *testing.T) {
	called := false
	next := http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		called = true
		w.WriteHeader(http.StatusNoContent)
	})
	recorder := httptest.NewRecorder()
	securityHeaders(next).ServeHTTP(recorder, httptest.NewRequest(http.MethodGet, "/", nil))

	if !called || recorder.Code != http.StatusNoContent {
		t.Fatalf("wrapped handler called=%v status=%d", called, recorder.Code)
	}
	for header, want := range map[string]string{
		"X-Content-Type-Options":  "nosniff",
		"X-Frame-Options":         "DENY",
		"Referrer-Policy":         "no-referrer",
		"Content-Security-Policy": "default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' data:; font-src 'self'; media-src 'self' data: blob:; object-src 'none'; frame-ancestors 'none'",
	} {
		if got := recorder.Header().Get(header); got != want {
			t.Errorf("%s = %q; want %q", header, got, want)
		}
	}
}
