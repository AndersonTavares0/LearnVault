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
		if recorder.Code != http.StatusOK || !strings.Contains(recorder.Body.String(), "data-sveltekit-preload-data") {
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
