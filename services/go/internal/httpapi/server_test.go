package httpapi

import (
	"context"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"net/url"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"learnvault/internal/library"
	"learnvault/internal/storage"
)

func openTestDB(t *testing.T) *storage.DB {
	t.Helper()
	db, err := storage.Open(context.Background(), filepath.Join(t.TempDir(), "learnvault.db"))
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = db.Close() })
	return db
}

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

func TestMutatingAuthEndpointsRequireSameOrigin(t *testing.T) {
	db := openTestDB(t)
	defer db.Close()
	if err := db.BootstrapAdmin(context.Background(), "owner", "a sufficiently long password"); err != nil { t.Fatal(err) }
	srv := NewWithOptions(db, false)
	for _, path := range []string{"/api/login", "/api/logout", "/api/account/credentials", "/api/admin/catalog/rescan"} {
		for _, origin := range []string{"", "http://evil.example"} {
			req := httptest.NewRequest(http.MethodPost, path, strings.NewReader(`{"username":"owner","password":"a sufficiently long password"}`))
			if origin != "" { req.Header.Set("Origin", origin) }
			result := httptest.NewRecorder(); srv.ServeHTTP(result, req)
			if result.Code != http.StatusForbidden || !strings.Contains(result.Body.String(), `"error":"csrf_failed"`) { t.Fatalf("%s origin %q: %d %s", path, origin, result.Code, result.Body.String()) }
		}
	}
	req := httptest.NewRequest(http.MethodPost, "/api/login", strings.NewReader(`{"username":"owner","password":"a sufficiently long password"}`)); req.Header.Set("Origin", "http://example.com")
	result := httptest.NewRecorder(); srv.ServeHTTP(result, req)
	if result.Code != http.StatusOK { t.Fatalf("allowed origin returned %d: %s", result.Code, result.Body.String()) }
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
	rescan.Header.Set("Origin", "http://example.com")
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
	rescan.Header.Set("Origin", "http://example.com")
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
	if got := result.Header().Get("Cache-Control"); got != "private, no-store" {
		t.Fatalf("media Cache-Control = %q; want private, no-store", got)
	}
}

func TestRescanTombstonesRemovedMedia(t *testing.T) {
	root := t.TempDir()
	lesson := filepath.Join(root, "Cat", "Curso", "Modulo", "Aula")
	if err := os.MkdirAll(lesson, 0o755); err != nil { t.Fatal(err) }
	mediaPath := filepath.Join(lesson, "removed.mp4")
	if err := os.WriteFile(mediaPath, []byte("video"), 0o644); err != nil { t.Fatal(err) }
	db := openTestDB(t)
	if err := db.BootstrapAdmin(context.Background(), "owner", "a sufficiently long password"); err != nil { t.Fatal(err) }
	lib, err := library.Open(root); if err != nil { t.Fatal(err) }
	srv := NewWithLibrary(db, false, lib)
	cookie := loginForTest(t, srv)
	rescan := httptest.NewRequest(http.MethodPost, "/api/admin/catalog/rescan", nil)
	rescan.Header.Set("Origin", "http://example.com"); rescan.AddCookie(cookie)
	first := httptest.NewRecorder(); srv.ServeHTTP(first, rescan)
	if first.Code != http.StatusOK { t.Fatalf("initial rescan returned %d", first.Code) }
	if err := os.Remove(mediaPath); err != nil { t.Fatal(err) }
	second := httptest.NewRequest(http.MethodPost, "/api/admin/catalog/rescan", nil)
	second.Header.Set("Origin", "http://example.com"); second.AddCookie(cookie)
	result := httptest.NewRecorder(); srv.ServeHTTP(result, second)
	if result.Code != http.StatusOK { t.Fatalf("removal rescan returned %d", result.Code) }
	request := httptest.NewRequest(http.MethodGet, "/api/media?path=Cat/Curso/Modulo/Aula/removed.mp4", nil)
	request.AddCookie(cookie)
	media := httptest.NewRecorder(); srv.ServeHTTP(media, request)
	if media.Code != http.StatusNotFound { t.Fatalf("removed media returned %d; want 404", media.Code) }
	if got := media.Header().Get("Cache-Control"); got != "private, no-store" { t.Fatalf("removed media Cache-Control = %q", got) }
}

// TestRescanKeepsExplicitRemovalAndCatalogIdentity cobre o tombstone explícito: depois
// de uma remoção, um rescan com o arquivo ainda no disco não pode reaparecer a mídia nem
// reautorizá-la, e a identidade da remoção (o caminho canônico) permanece.
func TestRescanKeepsExplicitRemovalAndCatalogIdentity(t *testing.T) {
	root := t.TempDir()
	lesson := filepath.Join(root, "Cat", "Curso", "Modulo", "Aula")
	if err := os.MkdirAll(lesson, 0o755); err != nil { t.Fatal(err) }
	mediaPath := "Cat/Curso/Modulo/Aula/removed.mp4"
	if err := os.WriteFile(filepath.Join(root, filepath.FromSlash(mediaPath)), []byte("video"), 0o644); err != nil { t.Fatal(err) }
	db := openTestDB(t)
	if err := db.BootstrapAdmin(context.Background(), "owner", "a sufficiently long password"); err != nil { t.Fatal(err) }
	lib, err := library.Open(root); if err != nil { t.Fatal(err) }
	srv := NewWithLibrary(db, false, lib)
	cookie := loginForTest(t, srv)

	rescan := func() {
		req := httptest.NewRequest(http.MethodPost, "/api/admin/catalog/rescan", nil)
		req.Header.Set("Origin", "http://example.com"); req.AddCookie(cookie)
		res := httptest.NewRecorder(); srv.ServeHTTP(res, req)
		if res.Code != http.StatusOK { t.Fatalf("rescan returned %d: %s", res.Code, res.Body.String()) }
	}
	mediaStatus := func() int {
		req := httptest.NewRequest(http.MethodGet, "/api/media?path="+url.QueryEscape(mediaPath), nil)
		req.AddCookie(cookie)
		res := httptest.NewRecorder(); srv.ServeHTTP(res, req)
		return res.Code
	}

	rescan()
	if code := mediaStatus(); code != http.StatusOK { t.Fatalf("media before removal = %d; want 200", code) }

	if err := db.RemoveMedia(context.Background(), mediaPath); err != nil { t.Fatal(err) }
	// O arquivo continua no disco; o rescan não pode desfazer a remoção.
	rescan()
	removed, err := db.RemovedMedia(context.Background()); if err != nil { t.Fatal(err) }
	if len(removed) != 1 { t.Fatalf("removal rows after rescan = %d; want 1", len(removed)) }
	if _, ok := removed[mediaPath]; !ok { t.Fatalf("removal identity lost after rescan: %v", removed) }

	if code := mediaStatus(); code != http.StatusNotFound { t.Fatalf("removed media after rescan = %d; want 404", code) }
	catalogReq := httptest.NewRequest(http.MethodGet, "/api/catalog", nil); catalogReq.AddCookie(cookie)
	catalogRes := httptest.NewRecorder(); srv.ServeHTTP(catalogRes, catalogReq)
	if catalogRes.Code != http.StatusOK { t.Fatalf("catalog returned %d", catalogRes.Code) }
	if strings.Contains(catalogRes.Body.String(), "removed.mp4") { t.Fatalf("removed media still present in catalog: %s", catalogRes.Body.String()) }
}

// TestCredentialsChangeRequiresCurrentPassword garante que uma sessão válida não troca
// credenciais sem a senha atual: senha errada é recusada com 401 e nada muda; ausência da
// senha atual é um pedido malformado (422). A troca correta sucede e invalida a sessão.
func TestCredentialsChangeRequiresCurrentPassword(t *testing.T) {
	db := openTestDB(t)
	if err := db.BootstrapAdmin(context.Background(), "owner", "a sufficiently long password"); err != nil { t.Fatal(err) }
	srv := NewWithOptions(db, false)
	login := httptest.NewRequest(http.MethodPost, "/api/login", strings.NewReader(`{"username":"owner","password":"a sufficiently long password"}`))
	login.Header.Set("Origin", "http://example.com")
	loginResult := httptest.NewRecorder(); srv.ServeHTTP(loginResult, login)
	if loginResult.Code != http.StatusOK { t.Fatalf("login returned %d", loginResult.Code) }
	cookie := loginResult.Result().Cookies()[0]

	change := func(body string) *httptest.ResponseRecorder {
		req := httptest.NewRequest(http.MethodPost, "/api/account/credentials", strings.NewReader(body))
		req.Header.Set("Origin", "http://example.com"); req.AddCookie(cookie)
		res := httptest.NewRecorder(); srv.ServeHTTP(res, req)
		return res
	}
	if res := change(`{"username":"owner2","password":"a sufficiently long replacement password"}`); res.Code != http.StatusUnprocessableEntity {
		t.Fatalf("missing current_password = %d; want 422", res.Code)
	}
	if res := change(`{"current_password":"wrong current password","username":"owner2","password":"a sufficiently long replacement password"}`); res.Code != http.StatusUnauthorized {
		t.Fatalf("wrong current_password = %d; want 401: %s", res.Code, res.Body.String())
	}
	if ok, _ := db.VerifyPassword(context.Background(), "owner", "a sufficiently long password"); !ok {
		t.Fatal("rejected change altered the stored credentials")
	}

	valid := change(`{"current_password":"a sufficiently long password","username":"owner2","password":"a sufficiently long replacement password"}`)
	if valid.Code != http.StatusOK { t.Fatalf("valid change = %d: %s", valid.Code, valid.Body.String()) }
	if ok, _ := db.VerifyPassword(context.Background(), "owner2", "a sufficiently long replacement password"); !ok {
		t.Fatal("valid change did not update credentials")
	}
	// A troca invalida as sessões anteriores e emite uma sessão nova.
	stillOld := httptest.NewRequest(http.MethodGet, "/api/me", nil); stillOld.AddCookie(cookie)
	oldResult := httptest.NewRecorder(); srv.ServeHTTP(oldResult, stillOld)
	if oldResult.Code != http.StatusUnauthorized { t.Fatalf("old session still valid after change: %d", oldResult.Code) }
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
	rescan.Header.Set("Origin", "http://example.com")
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
	req.Header.Set("Origin", "http://example.com")
	rr := httptest.NewRecorder()
	srv.ServeHTTP(rr, req)
	if rr.Code != http.StatusOK {
		t.Fatalf("login returned %d", rr.Code)
	}
	cookie := rr.Result().Cookies()[0]
	change := httptest.NewRequest(http.MethodPost, "/api/account/credentials", strings.NewReader(`{"current_password":"a sufficiently long password","username":"owner2","password":"a sufficiently long replacement password"}`))
	change.Header.Set("Origin", "http://example.com")
	change.AddCookie(cookie)
	changed := httptest.NewRecorder(); srv.ServeHTTP(changed, change)
	if changed.Code != http.StatusOK { t.Fatalf("credential change returned %d: %s", changed.Code, changed.Body.String()) }
	return changed.Result().Cookies()[0]
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
	login.Header.Set("Origin", "http://example.com")
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
	if meResult.Code != http.StatusOK || !strings.Contains(meResult.Body.String(), `"is_admin":true`) {
		t.Fatalf("unexpected /api/me response: %d %s", meResult.Code, meResult.Body.String())
	}
}
