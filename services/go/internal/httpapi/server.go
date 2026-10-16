package httpapi

import (
	"context"
	"crypto/rand"
	"encoding/base64"
	"encoding/json"
	"errors"
	"io"
	"mime"
	"net/http"
	"net/url"
	"path/filepath"
	"strconv"
	"strings"
	"sync"
	"time"

	"learnvault/internal/catalog"
	"learnvault/internal/library"
	"learnvault/internal/storage"
)

const sessionCookie = "learnvault_session"
const sessionLifetime = 12 * time.Hour

// defaultRecentStudyLimit mirrors the Rust server default for `/api/study/recent` when
// the query omits `limit`. The real ceiling stays storage.MaxRecentStudy.
const defaultRecentStudyLimit = 20

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
	RemovedMedia(context.Context) (map[string]struct{}, error)
	StudyProgress(context.Context, int64, string) (storage.StudyProgress, bool, error)
	RecordStudyPosition(context.Context, int64, string, int64) (storage.StudyProgress, error)
	SetStudyCompleted(context.Context, int64, string, bool) (storage.StudyProgress, error)
	RecentStudy(context.Context, int64, int) ([]storage.StudyProgress, error)
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
	// Unknown routes outside the API namespace are served by the SPA handler in
	// production; when the server is mounted directly this branch still forbids
	// storing the 404, so a stale error is never cached.
	w.Header().Set("Cache-Control", "no-store")
	http.NotFound(w, r)
}

// isAPIPath reports whether path belongs to the JSON API namespace, including
// the bare /api root, so unknown routes and errors still receive a cache policy.
func isAPIPath(path string) bool {
	return path == "/api" || strings.HasPrefix(path, "/api/")
}

func noStoreAPIPath(path string) bool {
	switch path {
	case "/api/login", "/api/me", "/api/catalog", "/api/media", "/api/account/credentials", "/api/logout", "/api/admin/catalog/rescan", "/api/study/recent", "/api/study/progress", "/api/study/position", "/api/study/completed", "/api/study/capabilities":
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
		if !sameOrigin(r) {
			writeJSON(w, http.StatusForbidden, map[string]string{"error": "csrf_failed"})
			return
		}
		s.login(w, r)
	case "/api/logout":
		if r.Method != http.MethodPost {
			methodNotAllowed(w)
			return
		}
		if !sameOrigin(r) {
			writeJSON(w, http.StatusForbidden, map[string]string{"error": "csrf_failed"})
			return
		}
		token := sessionFromRequest(r)
		if token != "" {
			_ = s.db.DeleteSession(r.Context(), token)
		}
		http.SetCookie(w, &http.Cookie{Name: sessionCookie, Value: "", Path: "/", MaxAge: -1, HttpOnly: true, Secure: s.cookieSecure, SameSite: http.SameSiteStrictMode})
		w.WriteHeader(http.StatusNoContent)
	case "/api/account/credentials":
		if r.Method != http.MethodPost {
			methodNotAllowed(w)
			return
		}
		if !sameOrigin(r) {
			writeJSON(w, http.StatusForbidden, map[string]string{"error": "csrf_failed"})
			return
		}
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
		} else if user.MustChangeCredentials {
			writeJSON(w, http.StatusForbidden, map[string]string{"error": "credentials_change_required"})
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
		if user, ok := s.currentUser(r); !ok {
			writeJSON(w, http.StatusUnauthorized, map[string]string{"error": "unauthorized"})
			return
		} else if user.MustChangeCredentials {
			writeJSON(w, http.StatusForbidden, map[string]string{"error": "credentials_change_required"})
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
		if !sameOrigin(r) {
			writeJSON(w, http.StatusForbidden, map[string]string{"error": "csrf_failed"})
			return
		}
		user, ok := s.currentUser(r)
		if !ok || user.Role != "admin" || user.MustChangeCredentials {
			if ok && user.MustChangeCredentials {
				writeJSON(w, http.StatusForbidden, map[string]string{"error": "credentials_change_required"})
				return
			}
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
	case "/api/study/recent":
		if r.Method != http.MethodGet {
			methodNotAllowed(w)
			return
		}
		s.studyRecent(w, r)
	case "/api/study/progress":
		if r.Method != http.MethodGet {
			methodNotAllowed(w)
			return
		}
		s.studyProgress(w, r)
	case "/api/study/position":
		if r.Method != http.MethodPost {
			methodNotAllowed(w)
			return
		}
		s.studyPosition(w, r)
	case "/api/study/completed":
		if r.Method != http.MethodPost {
			methodNotAllowed(w)
			return
		}
		s.studyCompleted(w, r)
	case "/api/study/capabilities":
		if r.Method != http.MethodGet {
			methodNotAllowed(w)
			return
		}
		s.studyCapabilities(w, r)
	default:
		writeJSON(w, http.StatusNotFound, map[string]string{"error": "not_found"})
	}
}

// catalogAllowsPath is the single authorization boundary for media. It walks the
// persisted canonical node tree (Catalog.Nodes) recursively, so a catalogued file is
// authorized at any depth — including files loose in a category — exactly as the SPA
// renders it. The categorical projection is derived from the same scan, so walking the
// nodes never hides a file the UI shows. The extension is only used for response
// handling; a file must first be present in the persisted scanner catalog (media,
// subtitles, or materials) to be served, so an uncatalogued or removed path stays 404.
func catalogAllowsPath(value catalog.Catalog, relative string) bool {
	clean, ok := normalizeCatalogPath(relative)
	if !ok {
		return false
	}
	if catalogContainsFile(value.LooseFiles, clean) {
		return true
	}
	return catalogNodesContain(value.Nodes, clean)
}

// catalogNodesContain is the one recursive walk over the persisted nodes. It descends
// children at any depth and checks both a node's files and its loose files, so media,
// subtitles and materials share a single identity rule.
func catalogNodesContain(nodes []catalog.Node, clean string) bool {
	for _, node := range nodes {
		if catalogContainsFile(node.Files, clean) || catalogContainsFile(node.LooseFiles, clean) {
			return true
		}
		if catalogNodesContain(node.Children, clean) {
			return true
		}
	}
	return false
}

// catalogContainsFile reports whether any persisted file normalizes to the requested
// canonical path, so traversal, absolute paths and alternate spellings never authorize.
func catalogContainsFile(files []catalog.File, clean string) bool {
	for _, file := range files {
		if path, valid := normalizeCatalogPath(file.Path); valid && path == clean {
			return true
		}
	}
	return false
}

// catalogStudyMedia reports whether a canonical path is a study-eligible playable media
// file in the persisted catalog. It walks the same canonical node tree as
// catalogAllowsPath, but applies the scanner's media rule so subtitles and materials such
// as PDFs are not accepted as study subjects.
func catalogStudyMedia(value catalog.Catalog, clean string) bool {
	if studyMediaFile(value.LooseFiles, clean) {
		return true
	}
	return studyMediaNodes(value.Nodes, clean)
}

func studyMediaNodes(nodes []catalog.Node, clean string) bool {
	for _, node := range nodes {
		if studyMediaFile(node.Files, clean) || studyMediaFile(node.LooseFiles, clean) {
			return true
		}
		if studyMediaNodes(node.Children, clean) {
			return true
		}
	}
	return false
}

func studyMediaFile(files []catalog.File, clean string) bool {
	for _, file := range files {
		if path, valid := normalizeCatalogPath(file.Path); valid && path == clean {
			return catalog.IsMediaName(file.Name, file.Size)
		}
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
	user, ok := s.currentUser(r)
	if !ok {
		writeJSON(w, http.StatusUnauthorized, map[string]string{"error": "unauthorized"})
		return
	}
	r.Body = http.MaxBytesReader(w, r.Body, 8*1024)
	var input struct {
		CurrentPassword string `json:"current_password"`
		Username        string `json:"username"`
		Password        string `json:"password"`
	}
	decoder := json.NewDecoder(r.Body)
	decoder.DisallowUnknownFields()
	if err := decoder.Decode(&input); err != nil {
		writeJSON(w, http.StatusBadRequest, map[string]string{"error": "invalid_request"})
		return
	}
	if len([]rune(input.CurrentPassword)) < 1 || len([]rune(input.CurrentPassword)) > 1024 || len([]rune(input.Username)) < 2 || len([]rune(input.Username)) > 64 || strings.TrimSpace(input.Username) != input.Username || len([]rune(input.Password)) < 12 || len([]rune(input.Password)) > 1024 {
		writeJSON(w, http.StatusUnprocessableEntity, map[string]string{"error": "invalid_credentials_format"})
		return
	}
	// A senha atual é exigida para que uma sessão roubada não troque as credenciais. Ela é
	// verificada antes de qualquer escrita e a troca continua transacional mais abaixo.
	valid, err := s.db.VerifyPassword(r.Context(), user.Username, input.CurrentPassword)
	if err != nil {
		writeJSON(w, http.StatusInternalServerError, map[string]string{"error": "internal_error"})
		return
	}
	if !valid {
		writeJSON(w, http.StatusUnauthorized, map[string]string{"error": "invalid_credentials"})
		return
	}
	var raw [32]byte
	if _, err := rand.Read(raw[:]); err != nil {
		writeJSON(w, http.StatusInternalServerError, map[string]string{"error": "internal_error"})
		return
	}
	token := base64.RawURLEncoding.EncodeToString(raw[:])
	if err := s.db.UpdateCredentialsAndSession(r.Context(), user.ID, input.Username, input.Password, token, sessionLifetime); err != nil {
		writeJSON(w, http.StatusInternalServerError, map[string]string{"error": "internal_error"})
		return
	}
	http.SetCookie(w, &http.Cookie{Name: sessionCookie, Value: token, Path: "/", MaxAge: int(sessionLifetime.Seconds()), HttpOnly: true, Secure: s.cookieSecure, SameSite: http.SameSiteStrictMode})
	user.Username = input.Username
	user.MustChangeCredentials = false
	writeJSON(w, http.StatusOK, user)
}

// studyUser resolves the authenticated owner of study progress. The owner is always the
// session user: no route accepts an owner from the client, and a temporary session that
// still has to change credentials is refused before any read or write.
func (s *Server) studyUser(w http.ResponseWriter, r *http.Request) (storage.User, bool) {
	user, ok := s.currentUser(r)
	if !ok {
		writeJSON(w, http.StatusUnauthorized, map[string]string{"error": "unauthorized"})
		return storage.User{}, false
	}
	if user.MustChangeCredentials {
		writeJSON(w, http.StatusForbidden, map[string]string{"error": "credentials_change_required"})
		return storage.User{}, false
	}
	return user, true
}

// knownStudyMedia reports whether a canonical library-relative path is a study-eligible
// media identity. It reuses the canonical catalog node walk, but only accepts playable
// media (the same scanner rule as `/api/media`'s classification), so subtitles and
// materials such as PDFs are not valid study subjects. A path kept by an explicit removal
// tombstone is also accepted, mirroring the Rust core: removal hides media from the
// catalog but the identity survives, so progress stays readable and writable.
func (s *Server) knownStudyMedia(ctx context.Context, mediaPath string) (bool, error) {
	value, err := s.db.Catalog(ctx)
	if err != nil {
		return false, err
	}
	if catalogStudyMedia(value, mediaPath) {
		return true, nil
	}
	removed, err := s.db.RemovedMedia(ctx)
	if err != nil {
		return false, err
	}
	if _, ok := removed[mediaPath]; !ok {
		return false, nil
	}
	// A tombstoned path no longer carries catalog metadata, so `.ts` is ambiguous (a
	// segment-sized `.ts` is a material). Only unambiguously playable extensions are
	// accepted; subtitles, materials and tombstoned `.ts` stay unknown.
	return catalog.IsUnambiguousMediaExtension(mediaPath), nil
}

// studyStateResponse is the client-facing progress of one media. updated_at is null when
// there is no recorded state, so "never studied" is distinct from "studied at zero".
type studyStateResponse struct {
	MediaID         string `json:"media_id"`
	PositionSeconds int64  `json:"position_seconds"`
	Completed       bool   `json:"completed"`
	UpdatedAt       *int64 `json:"updated_at"`
	Recorded        bool   `json:"recorded"`
}

func recordedStudyResponse(state storage.StudyProgress) studyStateResponse {
	updatedAt := state.UpdatedAt
	return studyStateResponse{
		MediaID:         state.MediaPath,
		PositionSeconds: state.PositionSeconds,
		Completed:       state.Completed,
		UpdatedAt:       &updatedAt,
		Recorded:        true,
	}
}

// absentStudyResponse mirrors the Rust absent-progress contract exactly: a known media
// that was never studied answers 200 with position 0, completed false, a null updated_at
// and recorded false, so "never studied" is distinguishable from "studied at zero".
func absentStudyResponse(mediaPath string) studyStateResponse {
	return studyStateResponse{
		MediaID:         mediaPath,
		PositionSeconds: 0,
		Completed:       false,
		UpdatedAt:       nil,
		Recorded:        false,
	}
}

func (s *Server) studyRecent(w http.ResponseWriter, r *http.Request) {
	user, ok := s.studyUser(w, r)
	if !ok {
		return
	}
	limit, valid := parseRecentStudyLimit(r.URL.Query())
	if !valid {
		writeJSON(w, http.StatusBadRequest, map[string]string{"error": "invalid_query"})
		return
	}
	states, err := s.db.RecentStudy(r.Context(), user.ID, limit)
	if err != nil {
		writeJSON(w, http.StatusInternalServerError, map[string]string{"error": "internal_error"})
		return
	}
	items := make([]studyStateResponse, 0, len(states))
	for _, state := range states {
		items = append(items, recordedStudyResponse(state))
	}
	writeJSON(w, http.StatusOK, struct {
		Items []studyStateResponse `json:"items"`
	}{Items: items})
}

func (s *Server) studyProgress(w http.ResponseWriter, r *http.Request) {
	user, ok := s.studyUser(w, r)
	if !ok {
		return
	}
	query := r.URL.Query()
	if !query.Has("media_id") {
		writeJSON(w, http.StatusBadRequest, map[string]string{"error": "invalid_query"})
		return
	}
	mediaPath, ok := s.requireStudyMedia(w, r, query.Get("media_id"))
	if !ok {
		return
	}
	state, recorded, err := s.db.StudyProgress(r.Context(), user.ID, mediaPath)
	if err != nil {
		writeJSON(w, http.StatusInternalServerError, map[string]string{"error": "internal_error"})
		return
	}
	if !recorded {
		writeJSON(w, http.StatusOK, absentStudyResponse(mediaPath))
		return
	}
	writeJSON(w, http.StatusOK, recordedStudyResponse(state))
}

func (s *Server) studyPosition(w http.ResponseWriter, r *http.Request) {
	user, ok := s.studyMutationUser(w, r)
	if !ok {
		return
	}
	r.Body = http.MaxBytesReader(w, r.Body, 8*1024)
	var input struct {
		MediaID         string `json:"media_id"`
		PositionSeconds *int64 `json:"position_seconds"`
	}
	if !decodeStrictJSON(w, r, &input) || input.PositionSeconds == nil {
		writeJSON(w, http.StatusBadRequest, map[string]string{"error": "invalid_request"})
		return
	}
	mediaPath, ok := s.requireStudyMedia(w, r, input.MediaID)
	if !ok {
		return
	}
	state, err := s.db.RecordStudyPosition(r.Context(), user.ID, mediaPath, *input.PositionSeconds)
	if errors.Is(err, storage.ErrInvalidStudyPosition) {
		writeJSON(w, http.StatusUnprocessableEntity, map[string]string{"error": "invalid_position"})
		return
	}
	if err != nil {
		writeJSON(w, http.StatusInternalServerError, map[string]string{"error": "internal_error"})
		return
	}
	writeJSON(w, http.StatusOK, recordedStudyResponse(state))
}

func (s *Server) studyCompleted(w http.ResponseWriter, r *http.Request) {
	user, ok := s.studyMutationUser(w, r)
	if !ok {
		return
	}
	r.Body = http.MaxBytesReader(w, r.Body, 8*1024)
	var input struct {
		MediaID   string `json:"media_id"`
		Completed *bool  `json:"completed"`
	}
	if !decodeStrictJSON(w, r, &input) || input.Completed == nil {
		writeJSON(w, http.StatusBadRequest, map[string]string{"error": "invalid_request"})
		return
	}
	mediaPath, ok := s.requireStudyMedia(w, r, input.MediaID)
	if !ok {
		return
	}
	state, err := s.db.SetStudyCompleted(r.Context(), user.ID, mediaPath, *input.Completed)
	if err != nil {
		writeJSON(w, http.StatusInternalServerError, map[string]string{"error": "internal_error"})
		return
	}
	writeJSON(w, http.StatusOK, recordedStudyResponse(state))
}

// studyCapabilities is the explicit capability/feature-detection contract for the Study
// UI. The Go service persists progress but its media identity is the canonical relative
// path, which does not survive a rename the way the Rust `media_id` does; this endpoint
// states that instead of letting a client assume full parity.
func (s *Server) studyCapabilities(w http.ResponseWriter, r *http.Request) {
	if _, ok := s.studyUser(w, r); !ok {
		return
	}
	writeJSON(w, http.StatusOK, struct {
		Study                bool   `json:"study"`
		MediaIdentity        string `json:"media_identity"`
		StableMediaIdentity  bool   `json:"stable_media_identity"`
		RecentDefaultLimit   int    `json:"recent_default_limit"`
		RecentMaxLimit       int    `json:"recent_max_limit"`
		RemovalKeepsProgress bool   `json:"removal_keeps_progress"`
	}{
		Study:                true,
		MediaIdentity:        "relative_path",
		StableMediaIdentity:  false,
		RecentDefaultLimit:   defaultRecentStudyLimit,
		RecentMaxLimit:       storage.MaxRecentStudy,
		RemovalKeepsProgress: true,
	})
}

// studyMutationUser applies the shared mutation preconditions: same-origin (CSRF) first,
// then the authenticated owner. It writes the error response and reports false on failure.
func (s *Server) studyMutationUser(w http.ResponseWriter, r *http.Request) (storage.User, bool) {
	if !sameOrigin(r) {
		writeJSON(w, http.StatusForbidden, map[string]string{"error": "csrf_failed"})
		return storage.User{}, false
	}
	return s.studyUser(w, r)
}

// requireStudyMedia normalizes the media identity and rejects an unknown path, returning
// the canonical path on success. It writes the 404 response and reports false when the
// path is not a media the server knows (invalid, uncatalogued or never recorded).
func (s *Server) requireStudyMedia(w http.ResponseWriter, r *http.Request, mediaID string) (string, bool) {
	mediaPath, valid := normalizeCatalogPath(mediaID)
	if !valid {
		writeJSON(w, http.StatusNotFound, map[string]string{"error": "media_not_found"})
		return "", false
	}
	known, err := s.knownStudyMedia(r.Context(), mediaPath)
	if err != nil {
		writeJSON(w, http.StatusInternalServerError, map[string]string{"error": "internal_error"})
		return "", false
	}
	if !known {
		writeJSON(w, http.StatusNotFound, map[string]string{"error": "media_not_found"})
		return "", false
	}
	return mediaPath, true
}

// parseRecentStudyLimit mirrors the Rust query contract: a missing limit uses the default,
// and any present value that is not a non-negative integer is a malformed query. Values
// above the ceiling are clamped by the store, not here.
func parseRecentStudyLimit(query url.Values) (int, bool) {
	if !query.Has("limit") {
		return defaultRecentStudyLimit, true
	}
	parsed, err := strconv.Atoi(query.Get("limit"))
	if err != nil || parsed < 0 {
		return 0, false
	}
	return parsed, true
}

// decodeStrictJSON rejects unknown fields and malformed bodies, matching the other API
// handlers, and requires the body to contain exactly one JSON value: trailing data is
// rejected as serde_json does for the Rust API. A missing required field is rejected by the
// caller through a pointer field, so a malformed mutation is never silently treated as a
// neutral value.
func decodeStrictJSON(w http.ResponseWriter, r *http.Request, target any) bool {
	decoder := json.NewDecoder(r.Body)
	decoder.DisallowUnknownFields()
	if err := decoder.Decode(target); err != nil {
		return false
	}
	return errors.Is(decoder.Decode(&struct{}{}), io.EOF)
}

func sameOrigin(r *http.Request) bool {
	host := r.Host
	if host == "" {
		host = "localhost"
	}
	expectedHTTP, expectedHTTPS := "http://"+host, "https://"+host
	origin := r.Header.Get("Origin")
	if origin == "" {
		ref := r.Header.Get("Referer")
		if ref == "" {
			return false
		}
		u, err := url.Parse(ref)
		if err != nil || u.Host != host {
			return false
		}
		origin = u.Scheme + "://" + u.Host
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
