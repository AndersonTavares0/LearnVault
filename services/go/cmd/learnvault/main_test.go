package main

import (
	"io/fs"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"
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
	}
}

func TestSPAFallbackDoesNotMaskMissingAssetsOrAPI(t *testing.T) {
	files, err := fs.Sub(webBuild, "web/build")
	if err != nil {
		t.Fatal(err)
	}
	handler := spaHandler(files)
	for _, path := range []string{"/missing.js", "/api/healthz"} {
		recorder := httptest.NewRecorder()
		handler.ServeHTTP(recorder, httptest.NewRequest(http.MethodGet, path, nil))
		if recorder.Code != http.StatusNotFound {
			t.Fatalf("expected missing route %s to return 404, got %d", path, recorder.Code)
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
		"Content-Security-Policy": "default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' data:; font-src 'self'; media-src 'self' data:; object-src 'none'; frame-ancestors 'none'",
	} {
		if got := recorder.Header().Get(header); got != want {
			t.Errorf("%s = %q; want %q", header, got, want)
		}
	}
}
