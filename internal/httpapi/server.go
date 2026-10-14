package httpapi

import (
	"context"
	"encoding/json"
	"mime"
	"net/http"
	"path/filepath"
	"strings"
	"sync"
	"time"

	"learnvault/internal/catalog"
	"learnvault/internal/library"
	"learnvault/internal/storage"
)

const sessionCookie = "learnvault_session"
const sessionLifetime = 12 * time.Hour

type Server struct {
	db           store
	cookieSecure bool
	libraryPath  string
	libraryRoot  *library.Root
	rescanMu     sync.Mutex
}

// store is the API boundary required by Server. Production uses storage.DB;
// keeping the boundary narrow lets HTTP behavior be tested without repeating
// password derivation or database setup for every request-path test.
type store interface {
	Ping(context.Context) error
	VerifyPassword(context.Context, string, string) (bool, error)
	UserByName(context.Context, string) (storage.User, error)
	CreateSession(context.Context, int64, time.Duration) (string, error)
	UserForSession(context.Context, string) (storage.User, error)
	DeleteSession(context.Context, string) error
	Catalog(context.Context) (catalog.Catalog, error)
	ReplaceCatalog(context.Context, catalog.Catalog) error
}

func New(db *storage.DB) *Server {
	return NewWithOptions(db, true)
}

func NewWithOptions(db *storage.DB, cookieSecure bool) *Server {
	return newWithStore(db, cookieSecure)
}

func newWithStore(db store, cookieSecure bool) *Server {
	return &Server{db: db, cookieSecure: cookieSecure}
}

func NewWithLibrary(db *storage.DB, cookieSecure bool, root *library.Root) *Server {
	server := NewWithOptions(db, cookieSecure)
	server.libraryPath = root.Path()
	server.libraryRoot = root
	return server
}

func (s *Server) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	w.Header().Set("X-Content-Type-Options", "nosniff")
	w.Header().Set("Referrer-Policy", "no-referrer")
	if strings.HasPrefix(r.URL.Path, "/api/") {
		s.api(w, r)
		return
	}
	http.NotFound(w, r)
}

func (s *Server) api(w http.ResponseWriter, r *http.Request) {
	switch r.URL.Path {
	case "/api/healthz":
		if r.Method != http.MethodGet {
			methodNotAllowed(w)
			return
		}
		if err := s.db.Ping(r.Context()); err != nil {
			writeJSON(w, http.StatusServiceUnavailable, map[string]string{"status": "unavailable"})
			return
		}
		writeJSON(w, http.StatusOK, map[string]string{"status": "ok"})
	case "/api/login":
		if r.Method != http.MethodPost {
			methodNotAllowed(w)
			return
		}
		s.login(w, r)
	case "/api/logout":
		if r.Method != http.MethodPost {
			methodNotAllowed(w)
			return
		}
		token := sessionFromRequest(r)
		if token != "" {
			_ = s.db.DeleteSession(r.Context(), token)
		}
		http.SetCookie(w, &http.Cookie{Name: sessionCookie, Value: "", Path: "/", MaxAge: -1, HttpOnly: true, Secure: s.cookieSecure, SameSite: http.SameSiteStrictMode})
		w.WriteHeader(http.StatusNoContent)
	case "/api/me":
		if r.Method != http.MethodGet {
			methodNotAllowed(w)
			return
		}
		user, ok := s.currentUser(r)
		if !ok {
			writeJSON(w, http.StatusUnauthorized, map[string]string{"error": "unauthorized"})
			return
		}
		writeJSON(w, http.StatusOK, user)
	case "/api/catalog":
		if r.Method != http.MethodGet {
			methodNotAllowed(w)
			return
		}
		if _, ok := s.currentUser(r); !ok {
			writeJSON(w, http.StatusUnauthorized, map[string]string{"error": "unauthorized"})
			return
		}
		value, err := s.db.Catalog(r.Context())
		if err != nil {
			writeJSON(w, http.StatusInternalServerError, map[string]string{"error": "internal_error"})
			return
		}
		writeJSON(w, http.StatusOK, value)
	case "/api/media":
		if r.Method != http.MethodGet && r.Method != http.MethodHead {
			methodNotAllowed(w)
			return
		}
		if _, ok := s.currentUser(r); !ok {
			writeJSON(w, http.StatusUnauthorized, map[string]string{"error": "unauthorized"})
			return
		}
		if s.libraryRoot == nil {
			writeJSON(w, http.StatusServiceUnavailable, map[string]string{"error": "library_unavailable"})
			return
		}
		relative := r.URL.Query().Get("path")
		if relative == "" || filepath.Ext(relative) == "" {
			writeJSON(w, http.StatusBadRequest, map[string]string{"error": "invalid_media_path"})
			return
		}
		file, err := s.libraryRoot.OpenFile(relative)
		if err != nil {
			http.NotFound(w, r)
			return
		}
		defer file.Close()
		info, err := file.Stat()
		if err != nil || info.IsDir() {
			http.NotFound(w, r)
			return
		}
		contentType := mime.TypeByExtension(filepath.Ext(relative))
		if contentType == "" {
			contentType = "application/octet-stream"
		}
		w.Header().Set("Content-Type", contentType)
		http.ServeContent(w, r, filepath.Base(relative), info.ModTime(), file)
	case "/api/admin/catalog/rescan":
		if r.Method != http.MethodPost {
			methodNotAllowed(w)
			return
		}
		user, ok := s.currentUser(r)
		if !ok || user.Role != "admin" {
			writeJSON(w, http.StatusForbidden, map[string]string{"error": "forbidden"})
			return
		}
		if s.libraryPath == "" {
			writeJSON(w, http.StatusServiceUnavailable, map[string]string{"error": "library_unavailable"})
			return
		}
		if !s.rescanMu.TryLock() {
			writeJSON(w, http.StatusConflict, map[string]string{"error": "rescan_in_progress"})
			return
		}
		defer s.rescanMu.Unlock()
		value, err := catalog.Scan(s.libraryPath)
		if err != nil {
			writeJSON(w, http.StatusInternalServerError, map[string]string{"error": "scan_failed"})
			return
		}
		if err := s.db.ReplaceCatalog(r.Context(), value); err != nil {
			writeJSON(w, http.StatusInternalServerError, map[string]string{"error": "persist_failed"})
			return
		}
		writeJSON(w, http.StatusOK, map[string]int{"categories": len(value.Categories)})
	default:
		writeJSON(w, http.StatusNotFound, map[string]string{"error": "not_found"})
	}
}

func (s *Server) login(w http.ResponseWriter, r *http.Request) {
	r.Body = http.MaxBytesReader(w, r.Body, 8*1024)
	var input struct {
		Username string `json:"username"`
		Password string `json:"password"`
	}
	decoder := json.NewDecoder(r.Body)
	decoder.DisallowUnknownFields()
	if err := decoder.Decode(&input); err != nil {
		writeJSON(w, http.StatusBadRequest, map[string]string{"error": "invalid_request"})
		return
	}
	valid, err := s.db.VerifyPassword(r.Context(), input.Username, input.Password)
	if err != nil {
		writeJSON(w, http.StatusInternalServerError, map[string]string{"error": "internal_error"})
		return
	}
	if !valid {
		writeJSON(w, http.StatusUnauthorized, map[string]string{"error": "invalid_credentials"})
		return
	}
	user, err := s.db.UserByName(r.Context(), input.Username)
	if err != nil {
		writeJSON(w, http.StatusInternalServerError, map[string]string{"error": "internal_error"})
		return
	}
	token, err := s.db.CreateSession(r.Context(), user.ID, sessionLifetime)
	if err != nil {
		writeJSON(w, http.StatusInternalServerError, map[string]string{"error": "internal_error"})
		return
	}
	http.SetCookie(w, &http.Cookie{Name: sessionCookie, Value: token, Path: "/", Expires: time.Now().Add(sessionLifetime), MaxAge: int(sessionLifetime.Seconds()), HttpOnly: true, Secure: s.cookieSecure, SameSite: http.SameSiteStrictMode})
	writeJSON(w, http.StatusOK, user)
}

func (s *Server) currentUser(r *http.Request) (storage.User, bool) {
	token := sessionFromRequest(r)
	if token == "" {
		return storage.User{}, false
	}
	u, err := s.db.UserForSession(r.Context(), token)
	return u, err == nil
}

func sessionFromRequest(r *http.Request) string {
	cookie, err := r.Cookie(sessionCookie)
	if err != nil {
		return ""
	}
	return cookie.Value
}

func methodNotAllowed(w http.ResponseWriter) {
	writeJSON(w, http.StatusMethodNotAllowed, map[string]string{"error": "method_not_allowed"})
}
func writeJSON(w http.ResponseWriter, status int, value any) {
	w.Header().Set("Content-Type", "application/json; charset=utf-8")
	w.WriteHeader(status)
	_ = json.NewEncoder(w).Encode(value)
}
