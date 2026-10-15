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
	return "fixture-session", s.healthErr
}
func (s fakeStore) UserForSession(_ context.Context, token string) (storage.User, error) {
	if s.healthErr != nil || token != "fixture-session" {
		return storage.User{}, errTestStore
	}
	return s.user, nil
}
func (s fakeStore) DeleteSession(context.Context, string) error { return s.healthErr }
func (s fakeStore) UpdateCredentials(context.Context, int64, string, string) error { return s.healthErr }
func (s fakeStore) Catalog(context.Context) (catalog.Catalog, error) {
	return catalog.Catalog{}, s.healthErr
}
func (s fakeStore) ReplaceCatalog(context.Context, catalog.Catalog) error { return s.healthErr }

func newFakeServer(t *testing.T) (*Server, *http.Cookie) {
	t.Helper()
	srv := newWithStore(fakeStore{user: storage.User{ID: 1, Username: "owner", Role: "admin"}}, false)
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
			srv.ServeHTTP(recorder, httptest.NewRequest(http.MethodPost, "/api/login", strings.NewReader(tc.body)))
			if recorder.Code != tc.want {
				t.Fatalf("status = %d; want %d (body %s)", recorder.Code, tc.want, recorder.Body.String())
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
			if tc.cookie {
				request.AddCookie(cookie)
			}
			srv.ServeHTTP(recorder, request)
			if recorder.Code != tc.want {
				t.Fatalf("status = %d; want %d (body %s)", recorder.Code, tc.want, recorder.Body.String())
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
		})
	}
}
