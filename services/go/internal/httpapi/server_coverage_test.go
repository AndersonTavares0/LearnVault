package httpapi

import (
	"context"
	"errors"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"
	"time"

	"learnvault/internal/catalog"
	"learnvault/internal/library"
	"learnvault/internal/storage"
)

var errTestStore = errors.New("test store unavailable")

type fakeStore struct {
	healthErr error
	user      storage.User
	session   *string
}

func (s fakeStore) Ping(context.Context) error { return s.healthErr }
func (s fakeStore) VerifyPassword(_ context.Context, username, password string) (bool, error) {
	if s.healthErr != nil {
		return false, s.healthErr
	}
	return username == "owner" && password == "a sufficiently long password", nil
}
func (s fakeStore) UserByName(_ context.Context, username string) (storage.User, error) {
	if s.healthErr != nil || username != s.user.Username {
		return storage.User{}, errTestStore
	}
	return s.user, nil
}
func (s fakeStore) CreateSession(context.Context, int64, time.Duration) (string, error) {
	if s.healthErr != nil {
		return "", s.healthErr
	}
	if s.session != nil {
		*s.session = "fixture-session"
	}
	return "fixture-session", nil
}
func (s fakeStore) UserForSession(_ context.Context, token string) (storage.User, error) {
	if s.healthErr != nil || s.session == nil || token != *s.session {
		return storage.User{}, errTestStore
	}
	return s.user, nil
}
func (s fakeStore) DeleteSession(context.Context, string) error { return s.healthErr }
func (s fakeStore) UpdateCredentials(context.Context, int64, string, string) error {
	return s.healthErr
}
func (s fakeStore) UpdateCredentialsAndSession(_ context.Context, _ int64, username, _ string, token string, _ time.Duration) error {
	if s.healthErr != nil {
		return s.healthErr
	}
	s.user.Username = username
	s.user.MustChangeCredentials = false
	if s.session != nil {
		*s.session = token
	}
	return nil
}
func (s fakeStore) Catalog(context.Context) (catalog.Catalog, error) {
	return catalog.Catalog{}, s.healthErr
}
func (s fakeStore) ReplaceCatalog(context.Context, catalog.Catalog) error { return s.healthErr }
func (s fakeStore) RemoveMedia(context.Context, string) error             { return s.healthErr }
func (s fakeStore) StudyRemovedMedia(context.Context, string) (bool, error) {
	return false, s.healthErr
}
func (s fakeStore) StudyProgress(context.Context, int64, string) (storage.StudyProgress, bool, error) {
	return storage.StudyProgress{}, false, s.healthErr
}
func (s fakeStore) RecordStudyPosition(context.Context, int64, string, int64) (storage.StudyProgress, error) {
	return storage.StudyProgress{}, s.healthErr
}
func (s fakeStore) SetStudyCompleted(context.Context, int64, string, bool) (storage.StudyProgress, error) {
	return storage.StudyProgress{}, s.healthErr
}
func (s fakeStore) RecentStudy(context.Context, int64, int) ([]storage.StudyProgress, error) {
	return nil, s.healthErr
}

func newFakeServer(t *testing.T) (*Server, *http.Cookie) {
	t.Helper()
	session := ""
	srv := newWithStore(fakeStore{user: storage.User{ID: 1, Username: "owner", Role: "admin"}, session: &session}, false)
	return srv, loginForTest(t, srv)
}

func TestAPIRejectsUnsupportedMethodsAndUnknownRoutes(t *testing.T) {
	srv, _ := newFakeServer(t)

	cases := []struct {
		path   string
		method string
	}{
		{"/api/healthz", http.MethodPost},
		{"/api/login", http.MethodGet},
		{"/api/logout", http.MethodGet},
		{"/api/account/credentials", http.MethodGet},
		{"/api/me", http.MethodPost},
		{"/api/catalog", http.MethodPost},
		{"/api/media", http.MethodPost},
		{"/api/admin/catalog/rescan", http.MethodGet},
		{"/api/unknown", http.MethodGet},
		{"/outside", http.MethodGet},
	}
	for _, tc := range cases {
		t.Run(tc.method+" "+tc.path, func(t *testing.T) {
			recorder := httptest.NewRecorder()
			srv.ServeHTTP(recorder, httptest.NewRequest(tc.method, tc.path, nil))
			want := http.StatusMethodNotAllowed
			if tc.path == "/api/unknown" {
				want = http.StatusNotFound
			}
			if !strings.HasPrefix(tc.path, "/api/") {
				want = http.StatusNotFound
			}
			if recorder.Code != want {
				t.Fatalf("status = %d; want %d (body %s)", recorder.Code, want, recorder.Body.String())
			}
			if recorder.Header().Get("X-Content-Type-Options") != "nosniff" || recorder.Header().Get("Referrer-Policy") != "no-referrer" {
				t.Fatal("API security headers are missing")
			}
			if strings.HasPrefix(tc.path, "/api/") {
				wantCache := "private, no-store"
				if tc.path == "/api/healthz" || tc.path == "/api/unknown" {
					wantCache = "no-store"
				}
				if recorder.Header().Get("Cache-Control") != wantCache {
					t.Fatalf("%s Cache-Control = %q; want %q", tc.path, recorder.Header().Get("Cache-Control"), wantCache)
				}
			} else if recorder.Header().Get("Cache-Control") != "no-store" {
				t.Fatalf("%s Cache-Control = %q; want no-store", tc.path, recorder.Header().Get("Cache-Control"))
			}
		})
	}
}

func TestLoginRejectsMalformedUnknownAndInvalidCredentials(t *testing.T) {
	srv, _ := newFakeServer(t)
	cases := []struct {
		name string
		body string
		want int
	}{
		{"malformed json", "{", http.StatusBadRequest},
		{"unknown field", `{"username":"owner","password":"a sufficiently long password","extra":true}`, http.StatusBadRequest},
		{"wrong password", `{"username":"owner","password":"wrong password"}`, http.StatusUnauthorized},
		{"missing user", `{"username":"missing","password":"wrong password"}`, http.StatusUnauthorized},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			recorder := httptest.NewRecorder()
			request := httptest.NewRequest(http.MethodPost, "/api/login", strings.NewReader(tc.body))
			request.Header.Set("Origin", "http://example.com")
			srv.ServeHTTP(recorder, request)
			if recorder.Code != tc.want {
				t.Fatalf("status = %d; want %d (body %s)", recorder.Code, tc.want, recorder.Body.String())
			}
			if recorder.Header().Get("Cache-Control") != "private, no-store" {
				t.Fatalf("login %s Cache-Control = %q; want private, no-store", tc.name, recorder.Header().Get("Cache-Control"))
			}
		})
	}
}

func TestUnavailableStorageAndLibraryReturnSafeErrors(t *testing.T) {
	srv, cookie := newFakeServer(t)
	srv.db = fakeStore{healthErr: errTestStore}

	for _, tc := range []struct {
		path   string
		method string
		cookie bool
		want   int
	}{
		{"/api/healthz", http.MethodGet, false, http.StatusServiceUnavailable},
		{"/api/me", http.MethodGet, true, http.StatusUnauthorized},
		{"/api/login", http.MethodPost, false, http.StatusInternalServerError},
	} {
		t.Run(tc.path, func(t *testing.T) {
			recorder := httptest.NewRecorder()
			request := httptest.NewRequest(tc.method, tc.path, strings.NewReader(`{"username":"owner","password":"a sufficiently long password"}`))
			if tc.path == "/api/login" {
				request.Header.Set("Origin", "http://example.com")
			}
			if tc.cookie {
				request.AddCookie(cookie)
			}
			srv.ServeHTTP(recorder, request)
			if recorder.Code != tc.want {
				t.Fatalf("status = %d; want %d (body %s)", recorder.Code, tc.want, recorder.Body.String())
			}
			wantCache := "private, no-store"
			if tc.path == "/api/healthz" {
				wantCache = "no-store"
			}
			if recorder.Header().Get("Cache-Control") != wantCache {
				t.Fatalf("%s Cache-Control = %q; want %q", tc.path, recorder.Header().Get("Cache-Control"), wantCache)
			}
		})
	}
}

func TestMediaAndRescanRejectInvalidOrUnavailableRequests(t *testing.T) {
	root, err := library.Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	srv, cookie := newFakeServer(t)
	srv.libraryPath = root.Path()
	srv.libraryRoot = root
	withoutLibrary := newWithStore(srv.db, false)

	for _, tc := range []struct {
		name string
		path string
		want int
	}{
		{"missing media file", "/api/media?path=video.mp4", http.StatusNotFound},
		{"missing extension", "/api/media?path=lesson", http.StatusBadRequest},
		{"empty path", "/api/media", http.StatusBadRequest},
		{"rescan unavailable", "/api/admin/catalog/rescan", http.StatusServiceUnavailable},
	} {
		t.Run(tc.name, func(t *testing.T) {
			recorder := httptest.NewRecorder()
			request := httptest.NewRequest(http.MethodGet, tc.path, nil)
			if strings.Contains(tc.path, "rescan") {
				request.Method = http.MethodPost
				request.Header.Set("Origin", "http://example.com")
			}
			request.AddCookie(cookie)
			handler := srv
			if strings.Contains(tc.path, "rescan") {
				handler = withoutLibrary
			}
			handler.ServeHTTP(recorder, request)
			if recorder.Code != tc.want {
				t.Fatalf("status = %d; want %d (body %s)", recorder.Code, tc.want, recorder.Body.String())
			}
			if recorder.Header().Get("Cache-Control") != "private, no-store" {
				t.Fatalf("%s Cache-Control = %q; want private, no-store", tc.path, recorder.Header().Get("Cache-Control"))
			}
		})
	}
}

// TestAPIHealthAndUnknownRoutesDisableCaching garante que o healthz público e
// as rotas API desconhecidas nunca sejam armazenados, sem introduzir dados
// sensíveis na resposta de saúde.
func TestAPIHealthAndUnknownRoutesDisableCaching(t *testing.T) {
	srv, _ := newFakeServer(t)

	t.Run("health is public and uncacheable", func(t *testing.T) {
		recorder := httptest.NewRecorder()
		srv.ServeHTTP(recorder, httptest.NewRequest(http.MethodGet, "/api/healthz", nil))
		if recorder.Code != http.StatusOK {
			t.Fatalf("status = %d; want 200 (body %s)", recorder.Code, recorder.Body.String())
		}
		body := recorder.Body.String()
		if !strings.Contains(body, `"status":"ok"`) {
			t.Fatalf("health body missing status: %s", body)
		}
		// O healthz é público e não pode carregar dados sensíveis.
		for _, forbidden := range []string{"password", "hash", "token", "secret", "owner"} {
			if strings.Contains(strings.ToLower(body), forbidden) {
				t.Fatalf("health body leaked %q: %s", forbidden, body)
			}
		}
		if got := recorder.Header().Get("Cache-Control"); got != "no-store" {
			t.Fatalf("health Cache-Control = %q; want no-store", got)
		}
		if recorder.Header().Get("X-Content-Type-Options") != "nosniff" || recorder.Header().Get("Referrer-Policy") != "no-referrer" {
			t.Fatal("health security headers are missing")
		}
	})

	t.Run("unknown api routes are uncacheable", func(t *testing.T) {
		for _, path := range []string{"/api/unknown", "/api"} {
			recorder := httptest.NewRecorder()
			srv.ServeHTTP(recorder, httptest.NewRequest(http.MethodGet, path, nil))
			if recorder.Code != http.StatusNotFound {
				t.Fatalf("%s status = %d; want 404 (body %s)", path, recorder.Code, recorder.Body.String())
			}
			if got := recorder.Header().Get("Cache-Control"); got != "no-store" {
				t.Fatalf("%s Cache-Control = %q; want no-store", path, got)
			}
		}
	})
}
