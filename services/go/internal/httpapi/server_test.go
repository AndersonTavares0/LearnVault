package httpapi

import (
	"context"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"learnvault/internal/library"
	"learnvault/internal/storage"
)

func TestHealthEndpoint(t *testing.T) {
	db, err := storage.Open(context.Background(), filepath.Join(t.TempDir(), "db.sqlite"))
	if err != nil {
		t.Fatal(err)
	}
	defer db.Close()
	srv := New(db)
	rr := httptest.NewRecorder()
	srv.ServeHTTP(rr, httptest.NewRequest(http.MethodGet, "/api/healthz", nil))
	if rr.Code != http.StatusOK || !strings.Contains(rr.Body.String(), `"status":"ok"`) {
		t.Fatalf("unexpected health response: %d %s", rr.Code, rr.Body.String())
	}
}

func TestHealthEndpointIncludesVersion(t *testing.T) {
	db, err := storage.Open(context.Background(), filepath.Join(t.TempDir(), "db.sqlite"))
	if err != nil {
		t.Fatal(err)
	}
	defer db.Close()
	srv := NewWithVersion(db, false, "v0.1.0")
	rr := httptest.NewRecorder()
	srv.ServeHTTP(rr, httptest.NewRequest(http.MethodGet, "/api/healthz", nil))
	var response map[string]string
	if err := json.Unmarshal(rr.Body.Bytes(), &response); err != nil {
		t.Fatal(err)
	}
	if response["status"] != "ok" || response["version"] != "v0.1.0" {
		t.Fatalf("health response = %#v; want status ok and version v0.1.0", response)
	}
}

func TestAdminCanRescanAndReadCatalog(t *testing.T) {
	root := t.TempDir()
	lesson := filepath.Join(root, "Cat", "Curso", "Modulo", "Aula")
	if err := os.MkdirAll(lesson, 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(lesson, "lesson.mp4"), []byte("video"), 0o644); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(lesson, "aula.mp4"), []byte("video"), 0o644); err != nil {
		t.Fatal(err)
	}
	db, err := storage.Open(context.Background(), filepath.Join(t.TempDir(), "db.sqlite"))
	if err != nil {
		t.Fatal(err)
	}
	defer db.Close()
	if err := db.BootstrapAdmin(context.Background(), "owner", "a sufficiently long password"); err != nil {
		t.Fatal(err)
	}
	lib, err := library.Open(root)
	if err != nil {
		t.Fatal(err)
	}
	srv := NewWithLibrary(db, false, lib)
	cookie := loginForTest(t, srv)
	rescan := httptest.NewRequest(http.MethodPost, "/api/admin/catalog/rescan", nil)
	rescan.AddCookie(cookie)
	rescanResult := httptest.NewRecorder()
	srv.ServeHTTP(rescanResult, rescan)
	if rescanResult.Code != http.StatusOK {
		t.Fatalf("rescan returned %d: %s", rescanResult.Code, rescanResult.Body.String())
	}
	request := httptest.NewRequest(http.MethodGet, "/api/catalog", nil)
	request.AddCookie(cookie)
	result := httptest.NewRecorder()
	srv.ServeHTTP(result, request)
	if result.Code != http.StatusOK || !strings.Contains(result.Body.String(), `"title":"Curso"`) {
		t.Fatalf("unexpected catalog: %d %s", result.Code, result.Body.String())
	}
}

func TestAuthenticatedMediaSupportsRangeAndRejectsUnauthenticatedAccess(t *testing.T) {
	root := t.TempDir()
	lesson := filepath.Join(root, "Cat", "Curso", "Modulo", "Aula")
	if err := os.MkdirAll(lesson, 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(lesson, "aula.mp4"), []byte("0123456789"), 0o644); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(root, "not-cataloged.mp4"), []byte("secret"), 0o644); err != nil {
		t.Fatal(err)
	}
	db, err := storage.Open(context.Background(), filepath.Join(t.TempDir(), "db.sqlite"))
	if err != nil {
		t.Fatal(err)
	}
	defer db.Close()
	if err := db.BootstrapAdmin(context.Background(), "owner", "a sufficiently long password"); err != nil {
		t.Fatal(err)
	}
	lib, err := library.Open(root)
	if err != nil {
		t.Fatal(err)
	}
	srv := NewWithLibrary(db, false, lib)
	unauthenticated := httptest.NewRecorder()
	srv.ServeHTTP(unauthenticated, httptest.NewRequest(http.MethodGet, "/api/media?path=Cat/Curso/Modulo/Aula/aula.mp4", nil))
	if unauthenticated.Code != http.StatusUnauthorized {
		t.Fatalf("expected 401, got %d", unauthenticated.Code)
	}
	cookie := loginForTest(t, srv)
	rescan := httptest.NewRequest(http.MethodPost, "/api/admin/catalog/rescan", nil)
	rescan.AddCookie(cookie)
	rescanResult := httptest.NewRecorder()
	srv.ServeHTTP(rescanResult, rescan)
	if rescanResult.Code != http.StatusOK {
		t.Fatalf("rescan returned %d: %s", rescanResult.Code, rescanResult.Body.String())
	}
	request := httptest.NewRequest(http.MethodGet, "/api/media?path=Cat/Curso/Modulo/Aula/aula.mp4", nil)
	request.AddCookie(cookie)
	request.Header.Set("Range", "bytes=2-5")
	result := httptest.NewRecorder()
	srv.ServeHTTP(result, request)
	if result.Code != http.StatusPartialContent || result.Body.String() != "2345" {
		t.Fatalf("unexpected range response: %d %q", result.Code, result.Body.String())
	}
	if result.Header().Get("Accept-Ranges") != "bytes" {
		t.Fatal("range support header missing")
	}
	if result.Header().Get("Content-Type") != "video/mp4" {
		t.Fatalf("video content type = %q; want video/mp4", result.Header().Get("Content-Type"))
	}
	rootMedia := httptest.NewRequest(http.MethodGet, "/api/media?path=not-cataloged.mp4", nil)
	rootMedia.AddCookie(cookie)
	served := httptest.NewRecorder()
	srv.ServeHTTP(served, rootMedia)
	if served.Code != http.StatusOK || served.Body.String() != "secret" {
		t.Fatalf("root media returned %d %q; want catalogued file", served.Code, served.Body.String())
	}
}

func TestMediaMaterialsAreDownloadsButVTTRemainsPlayable(t *testing.T) {
	root := t.TempDir()
	lesson := filepath.Join(root, "Cat", "Curso", "Modulo", "Aula")
	if err := os.MkdirAll(lesson, 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(lesson, "lesson.mp4"), []byte("video"), 0o644); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(lesson, "unsafe.html"), []byte("<script>alert(1)</script>"), 0o644); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(lesson, "unsafe.svg"), []byte("<svg onload=alert(1) />"), 0o644); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(lesson, "captions.vtt"), []byte("WEBVTT\n\n00:00.000 --> 00:01.000\nOlá"), 0o644); err != nil {
		t.Fatal(err)
	}
	db, err := storage.Open(context.Background(), filepath.Join(t.TempDir(), "db.sqlite"))
	if err != nil {
		t.Fatal(err)
	}
	defer db.Close()
	if err := db.BootstrapAdmin(context.Background(), "owner", "a sufficiently long password"); err != nil {
		t.Fatal(err)
	}
	lib, err := library.Open(root)
	if err != nil {
		t.Fatal(err)
	}
	srv := NewWithLibrary(db, false, lib)
	cookie := loginForTest(t, srv)
	rescan := httptest.NewRequest(http.MethodPost, "/api/admin/catalog/rescan", nil)
	rescan.AddCookie(cookie)
	rescanResult := httptest.NewRecorder()
	srv.ServeHTTP(rescanResult, rescan)
	if rescanResult.Code != http.StatusOK {
		t.Fatalf("rescan returned %d: %s", rescanResult.Code, rescanResult.Body.String())
	}

	for _, name := range []string{"unsafe.html", "unsafe.svg"} {
		req := httptest.NewRequest(http.MethodGet, "/api/media?path=Cat/Curso/Modulo/Aula/"+name, nil)
		req.AddCookie(cookie)
		result := httptest.NewRecorder()
		srv.ServeHTTP(result, req)
		if result.Code != http.StatusOK {
			t.Fatalf("%s returned %d", name, result.Code)
		}
		if got := result.Header().Get("Content-Disposition"); !strings.HasPrefix(got, "attachment") {
			t.Errorf("%s Content-Disposition = %q; want attachment", name, got)
		}
		if got := result.Header().Get("Content-Type"); got != "application/octet-stream" {
			t.Errorf("%s Content-Type = %q; want application/octet-stream", name, got)
		}
		if got := result.Header().Get("X-Content-Type-Options"); got != "nosniff" {
			t.Errorf("%s nosniff = %q; want nosniff", name, got)
		}
	}

	req := httptest.NewRequest(http.MethodGet, "/api/media?path=Cat/Curso/Modulo/Aula/captions.vtt", nil)
	req.AddCookie(cookie)
	result := httptest.NewRecorder()
	srv.ServeHTTP(result, req)
	if result.Code != http.StatusOK || result.Body.String() != "WEBVTT\n\n00:00.000 --> 00:01.000\nOlá" {
		t.Fatalf("unexpected VTT response: %d %q", result.Code, result.Body.String())
	}
	if got := result.Header().Get("Content-Type"); got != "text/vtt" {
		t.Errorf("VTT Content-Type = %q; want text/vtt", got)
	}
	if result.Header().Get("Content-Disposition") != "" {
		t.Errorf("VTT Content-Disposition = %q; want empty", result.Header().Get("Content-Disposition"))
	}
}

func loginForTest(t *testing.T, srv http.Handler) *http.Cookie {
	t.Helper()
	body := `{"username":"owner","password":"a sufficiently long password"}`
	req := httptest.NewRequest(http.MethodPost, "/api/login", strings.NewReader(body))
	rr := httptest.NewRecorder()
	srv.ServeHTTP(rr, req)
	if rr.Code != http.StatusOK {
		t.Fatalf("login returned %d", rr.Code)
	}
	return rr.Result().Cookies()[0]
}

func TestUnauthenticatedMeIsRejected(t *testing.T) {
	db, err := storage.Open(context.Background(), filepath.Join(t.TempDir(), "db.sqlite"))
	if err != nil {
		t.Fatal(err)
	}
	defer db.Close()
	srv := New(db)
	rr := httptest.NewRecorder()
	srv.ServeHTTP(rr, httptest.NewRequest(http.MethodGet, "/api/me", nil))
	if rr.Code != http.StatusUnauthorized {
		t.Fatalf("expected 401, got %d", rr.Code)
	}
}

func TestLoginCreatesSessionAndMeReturnsCurrentUser(t *testing.T) {
	db, err := storage.Open(context.Background(), filepath.Join(t.TempDir(), "db.sqlite"))
	if err != nil {
		t.Fatal(err)
	}
	defer db.Close()
	if err := db.BootstrapAdmin(context.Background(), "owner", "a sufficiently long password"); err != nil {
		t.Fatal(err)
	}
	srv := NewWithOptions(db, true)
	body, _ := json.Marshal(map[string]string{"username": "owner", "password": "a sufficiently long password"})
	login := httptest.NewRequest(http.MethodPost, "/api/login", strings.NewReader(string(body)))
	loginResult := httptest.NewRecorder()
	srv.ServeHTTP(loginResult, login)
	if loginResult.Code != http.StatusOK {
		t.Fatalf("login returned %d: %s", loginResult.Code, loginResult.Body.String())
	}
	cookies := loginResult.Result().Cookies()
	if len(cookies) != 1 || !cookies[0].HttpOnly || !cookies[0].Secure {
		t.Fatalf("session cookie missing security attributes: %#v", cookies)
	}
	me := httptest.NewRequest(http.MethodGet, "/api/me", nil)
	me.AddCookie(cookies[0])
	meResult := httptest.NewRecorder()
	srv.ServeHTTP(meResult, me)
	if meResult.Code != http.StatusOK || !strings.Contains(meResult.Body.String(), `"role":"admin"`) {
		t.Fatalf("unexpected /api/me response: %d %s", meResult.Code, meResult.Body.String())
	}
}
