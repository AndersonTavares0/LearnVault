package httpapi

import (
	"context"
	"crypto/rand"
	"encoding/base64"
	"encoding/json"
	"mime"
	"net/http"
	"path/filepath"
	"net/url"
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
	version      string
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
	UpdateCredentialsAndSession(context.Context, int64, string, string, string, time.Duration) error
	Catalog(context.Context) (catalog.Catalog, error)
	ReplaceCatalog(context.Context, catalog.Catalog) error
}

func New(db *storage.DB) *Server {
	return NewWithOptions(db, true)
}

func NewWithOptions(db *storage.DB, cookieSecure bool) *Server {
	return NewWithVersion(db, cookieSecure, "dev")
}

func newWithStore(db store, cookieSecure bool) *Server {
	return newWithStoreVersion(db, cookieSecure, "dev")
}

func newWithStoreVersion(db store, cookieSecure bool, version string) *Server {
	return &Server{db: db, cookieSecure: cookieSecure, version: version}
}

func NewWithVersion(db *storage.DB, cookieSecure bool, version string) *Server {
	return newWithStoreVersion(db, cookieSecure, version)
}

func NewWithLibrary(db *storage.DB, cookieSecure bool, root *library.Root) *Server {
	return NewWithLibraryVersion(db, cookieSecure, root, "dev")
}

func NewWithLibraryVersion(db *storage.DB, cookieSecure bool, root *library.Root, version string) *Server {
	server := NewWithVersion(db, cookieSecure, version)
	server.libraryPath = root.Path()
	server.libraryRoot = root
	return server
}

func (s *Server) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	w.Header().Set("X-Content-Type-Options", "nosniff")
	w.Header().Set("Referrer-Policy", "no-referrer")
	if isAPIPath(r.URL.Path) {
		// Every API response is uncacheable. Session state, catalogs, and media must
		// never be stored by an intermediary or remain available from a browser cache
		// after logout, so those paths are additionally private. Health, unknown
		// routes, and API errors still need a cache policy of their own: a stale 200
		// or 503 must never mask a recovery.
		if noStoreAPIPath(r.URL.Path) {
			w.Header().Set("Cache-Control", "private, no-store")
		} else {
			w.Header().Set("Cache-Control", "no-store")
		}
		s.api(w, r)
		return
	}
	http.NotFound(w, r)
}

// isAPIPath reports whether path belongs to the JSON API namespace, including
// the bare /api root, so unknown routes and errors still receive a cache policy.
func isAPIPath(path string) bool {
	return path == "/api" || strings.HasPrefix(path, "/api/")
}

func noStoreAPIPath(path string) bool {
	switch path {
	case "/api/login", "/api/me", "/api/catalog", "/api/media", "/api/account/credentials", "/api/logout", "/api/admin/catalog/rescan":
		return true
	default:
		return false
	}
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
		writeJSON(w, http.StatusOK, map[string]string{"status": "ok", "version": s.version})
	case "/api/login":
		if r.Method != http.MethodPost {
			methodNotAllowed(w)
			return
		}
		if !sameOrigin(r) { writeJSON(w, http.StatusForbidden, map[string]string{"error":"csrf_failed"}); return }
		s.login(w, r)
	case "/api/logout":
		if r.Method != http.MethodPost {
			methodNotAllowed(w)
			return
		}
		if !sameOrigin(r) { writeJSON(w, http.StatusForbidden, map[string]string{"error":"csrf_failed"}); return }
		token := sessionFromRequest(r)
		if token != "" {
			_ = s.db.DeleteSession(r.Context(), token)
		}
		http.SetCookie(w, &http.Cookie{Name: sessionCookie, Value: "", Path: "/", MaxAge: -1, HttpOnly: true, Secure: s.cookieSecure, SameSite: http.SameSiteStrictMode})
		w.WriteHeader(http.StatusNoContent)
	case "/api/account/credentials":
		if r.Method != http.MethodPost { methodNotAllowed(w); return }
		if !sameOrigin(r) { writeJSON(w, http.StatusForbidden, map[string]string{"error":"csrf_failed"}); return }
		s.credentials(w, r)
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
		if user, ok := s.currentUser(r); !ok {
			writeJSON(w, http.StatusUnauthorized, map[string]string{"error": "unauthorized"})
			return
		} else if user.MustChangeCredentials { writeJSON(w, http.StatusForbidden, map[string]string{"error":"credentials_change_required"}); return
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
		if user, ok := s.currentUser(r); !ok {
			writeJSON(w, http.StatusUnauthorized, map[string]string{"error": "unauthorized"})
			return
		} else if user.MustChangeCredentials { writeJSON(w, http.StatusForbidden, map[string]string{"error":"credentials_change_required"}); return
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
		catalogValue, err := s.db.Catalog(r.Context())
		if err != nil {
			writeJSON(w, http.StatusInternalServerError, map[string]string{"error": "internal_error"})
			return
		}
		if !catalogAllowsPath(catalogValue, relative) {
			http.NotFound(w, r)
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
		contentType, playable := safeMediaType(filepath.Ext(relative))
		w.Header().Set("Content-Type", contentType)
		if !playable {
			w.Header().Set("Content-Disposition", mime.FormatMediaType("attachment", map[string]string{"filename": filepath.Base(relative)}))
		}
		http.ServeContent(w, r, filepath.Base(relative), info.ModTime(), file)
	case "/api/admin/catalog/rescan":
		if r.Method != http.MethodPost {
			methodNotAllowed(w)
			return
		}
		if !sameOrigin(r) { writeJSON(w, http.StatusForbidden, map[string]string{"error":"csrf_failed"}); return }
		user, ok := s.currentUser(r)
		if !ok || user.Role != "admin" || user.MustChangeCredentials {
			if ok && user.MustChangeCredentials { writeJSON(w, http.StatusForbidden, map[string]string{"error":"credentials_change_required"}); return }
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

// catalogAllowsPath is the authorization boundary for media. The extension is
// only used for response handling; a file must first be present in the
// persisted scanner catalog (media, subtitles, or materials).
func catalogAllowsPath(value catalog.Catalog, relative string) bool {
	clean, ok := normalizeCatalogPath(relative)
	if !ok {
		return false
	}
	for _, category := range value.Categories {
		for _, course := range category.Courses {
			for _, module := range course.Modules {
				for _, lesson := range module.Lessons {
					for _, file := range append(append(lesson.Media, lesson.Subtitles...), lesson.Materials...) {
						if path, valid := normalizeCatalogPath(file.Path); valid && path == clean {
							return true
						}
					}
				}
			}
		}
	}
	for _, file := range value.LooseFiles {
		if path, valid := normalizeCatalogPath(file.Path); valid && path == clean { return true }
	}
	return false
}

// normalizeCatalogPath delegates to the shared catalog normalizer so the media
// authorization boundary and the removal tombstone agree on one path identity.
func normalizeCatalogPath(value string) (string, bool) {
	return catalog.NormalizePath(value)
}

// safeMediaType only permits formats consumed by the player. Everything else,
// including browser-active documents such as HTML and SVG, is downloaded.
func safeMediaType(extension string) (string, bool) {
	switch strings.ToLower(extension) {
	case ".mp4":
		return "video/mp4", true
	case ".webm":
		return "video/webm", true
	case ".ogv":
		return "video/ogg", true
	case ".mp3":
		return "audio/mpeg", true
	case ".m4a":
		return "audio/mp4", true
	case ".ogg", ".oga":
		return "audio/ogg", true
	case ".wav":
		return "audio/wav", true
	case ".flac":
		return "audio/flac", true
	case ".vtt":
		return "text/vtt", true
	default:
		return "application/octet-stream", false
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

func (s *Server) credentials(w http.ResponseWriter, r *http.Request) {
	user, ok := s.currentUser(r); if !ok { writeJSON(w, http.StatusUnauthorized, map[string]string{"error":"unauthorized"}); return }
	r.Body = http.MaxBytesReader(w, r.Body, 8*1024); var input struct { CurrentPassword string `json:"current_password"`; Username string `json:"username"`; Password string `json:"password"` }
	decoder := json.NewDecoder(r.Body); decoder.DisallowUnknownFields(); if err := decoder.Decode(&input); err != nil { writeJSON(w, http.StatusBadRequest, map[string]string{"error":"invalid_request"}); return }
	if len([]rune(input.CurrentPassword)) < 1 || len([]rune(input.CurrentPassword)) > 1024 || len([]rune(input.Username)) < 2 || len([]rune(input.Username)) > 64 || strings.TrimSpace(input.Username) != input.Username || len([]rune(input.Password)) < 12 || len([]rune(input.Password)) > 1024 { writeJSON(w, http.StatusUnprocessableEntity, map[string]string{"error":"invalid_credentials_format"}); return }
	// A senha atual é exigida para que uma sessão roubada não troque as credenciais. Ela é
	// verificada antes de qualquer escrita e a troca continua transacional mais abaixo.
	valid, err := s.db.VerifyPassword(r.Context(), user.Username, input.CurrentPassword)
	if err != nil { writeJSON(w, http.StatusInternalServerError, map[string]string{"error":"internal_error"}); return }
	if !valid { writeJSON(w, http.StatusUnauthorized, map[string]string{"error":"invalid_credentials"}); return }
	var raw [32]byte; if _, err := rand.Read(raw[:]); err != nil { writeJSON(w, http.StatusInternalServerError, map[string]string{"error":"internal_error"}); return }; token := base64.RawURLEncoding.EncodeToString(raw[:])
	if err := s.db.UpdateCredentialsAndSession(r.Context(), user.ID, input.Username, input.Password, token, sessionLifetime); err != nil { writeJSON(w, http.StatusInternalServerError, map[string]string{"error":"internal_error"}); return }
	http.SetCookie(w, &http.Cookie{Name: sessionCookie, Value: token, Path: "/", MaxAge: int(sessionLifetime.Seconds()), HttpOnly: true, Secure: s.cookieSecure, SameSite: http.SameSiteStrictMode}); user.Username = input.Username; user.MustChangeCredentials = false; writeJSON(w, http.StatusOK, user)
}

func sameOrigin(r *http.Request) bool {
	host := r.Host; if host == "" { host = "localhost" }
	expectedHTTP, expectedHTTPS := "http://"+host, "https://"+host
	origin := r.Header.Get("Origin")
	if origin == "" {
		ref := r.Header.Get("Referer"); if ref == "" { return false }
		u, err := url.Parse(ref); if err != nil || u.Host != host { return false }; origin = u.Scheme+"://"+u.Host
	}
	return origin == expectedHTTP || origin == expectedHTTPS
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
