package httpapi

import (
	"context"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"learnvault/internal/catalog"
	"learnvault/internal/storage"
)

// studyTestMedia is the canonical library-relative path used as the Go media identity.
const studyTestMedia = "cat/curso/modulo/aula/video.mp4"

// studyTestCatalog puts the media at lesson depth so the study existence check exercises
// the same canonical node walk used by /api/media, not just the categorical projection.
func studyTestCatalog() catalog.Catalog {
	return catalog.Catalog{
		Nodes: []catalog.Node{{
			Title: "cat", Path: "cat", Role: "category",
			Children: []catalog.Node{{
				Title: "aula", Path: "cat/curso/modulo/aula", Role: "lesson",
				Files: []catalog.File{{Name: "video.mp4", Path: studyTestMedia, Size: 10}},
			}},
		}},
	}
}

func seedStudyCatalog(t *testing.T, db *storage.DB) {
	t.Helper()
	if err := db.ReplaceCatalog(context.Background(), studyTestCatalog()); err != nil {
		t.Fatal(err)
	}
}

func openStudyServer(t *testing.T) (*storage.DB, *Server) {
	t.Helper()
	db := openTestDB(t)
	seedStudyCatalog(t, db)
	if err := db.BootstrapAdmin(context.Background(), "owner", "a sufficiently long password"); err != nil {
		t.Fatal(err)
	}
	return db, NewWithOptions(db, false)
}

// studySessionFor creates a session without the HTTP credential-change flow, so the test
// focuses on study behavior and can reuse the token after reopening the same database.
func studySessionFor(t *testing.T, db *storage.DB) *http.Cookie {
	t.Helper()
	user, err := db.UserByName(context.Background(), "owner")
	if err != nil {
		t.Fatal(err)
	}
	token, err := db.CreateSession(context.Background(), user.ID, time.Hour)
	if err != nil {
		t.Fatal(err)
	}
	return &http.Cookie{Name: sessionCookie, Value: token}
}

func studyRequest(method, target string, cookie *http.Cookie, body string, sameOriginHeader bool) *http.Request {
	var request *http.Request
	if body == "" {
		request = httptest.NewRequest(method, target, nil)
	} else {
		request = httptest.NewRequest(method, target, strings.NewReader(body))
	}
	if cookie != nil {
		request.AddCookie(cookie)
	}
	if sameOriginHeader {
		request.Header.Set("Origin", "http://example.com")
	}
	return request
}

func studyJSON(t *testing.T, recorder *httptest.ResponseRecorder) map[string]any {
	t.Helper()
	var decoded map[string]any
	if err := json.Unmarshal(recorder.Body.Bytes(), &decoded); err != nil {
		t.Fatalf("decode %q: %v", recorder.Body.String(), err)
	}
	return decoded
}

func TestStudyRequiresAuthentication(t *testing.T) {
	db, srv := openStudyServer(t)
	_ = db

	cases := []struct {
		method string
		target string
		body   string
	}{
		{http.MethodGet, "/api/study/recent", ""},
		{http.MethodGet, "/api/study/progress?media_id=" + studyTestMedia, ""},
		{http.MethodPost, "/api/study/position", `{"media_id":"` + studyTestMedia + `","position_seconds":5}`},
		{http.MethodPost, "/api/study/completed", `{"media_id":"` + studyTestMedia + `","completed":true}`},
		{http.MethodGet, "/api/study/capabilities", ""},
	}
	for _, tc := range cases {
		t.Run(tc.method+" "+tc.target, func(t *testing.T) {
			recorder := httptest.NewRecorder()
			srv.ServeHTTP(recorder, studyRequest(tc.method, tc.target, nil, tc.body, true))
			if recorder.Code != http.StatusUnauthorized {
				t.Fatalf("status = %d; want 401 (body %s)", recorder.Code, recorder.Body.String())
			}
			if got := recorder.Header().Get("Cache-Control"); got != "private, no-store" {
				t.Fatalf("Cache-Control = %q; want private, no-store", got)
			}
		})
	}
}

func TestStudyRoundTripCompletionIndependenceAndPersistence(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "study.db")
	ctx := context.Background()

	db, err := storage.Open(ctx, path)
	if err != nil {
		t.Fatal(err)
	}
	seedStudyCatalog(t, db)
	if err := db.BootstrapAdmin(ctx, "owner", "a sufficiently long password"); err != nil {
		t.Fatal(err)
	}
	srv := NewWithOptions(db, false)
	cookie := studySessionFor(t, db)

	post := func(target, body string) *httptest.ResponseRecorder {
		recorder := httptest.NewRecorder()
		srv.ServeHTTP(recorder, studyRequest(http.MethodPost, target, cookie, body, true))
		return recorder
	}

	recorded := post("/api/study/position", `{"media_id":"`+studyTestMedia+`","position_seconds":42}`)
	if recorded.Code != http.StatusOK {
		t.Fatalf("record position = %d: %s", recorded.Code, recorded.Body.String())
	}
	body := studyJSON(t, recorded)
	if body["media_id"] != studyTestMedia || body["position_seconds"] != float64(42) || body["completed"] != false || body["recorded"] != true {
		t.Fatalf("recorded body = %#v", body)
	}
	updatedAt, ok := body["updated_at"].(float64)
	if !ok || updatedAt <= 1000 {
		t.Fatalf("updated_at = %#v; want a real clock instant", body["updated_at"])
	}

	read := httptest.NewRecorder()
	srv.ServeHTTP(read, studyRequest(http.MethodGet, "/api/study/progress?media_id="+studyTestMedia, cookie, "", false))
	if read.Code != http.StatusOK {
		t.Fatalf("read = %d: %s", read.Code, read.Body.String())
	}
	if got := studyJSON(t, read); got["recorded"] != true || got["position_seconds"] != float64(42) || got["updated_at"] != updatedAt {
		t.Fatalf("read body = %#v", got)
	}

	// Resending the same position is idempotent and does not move updated_at.
	again := studyJSON(t, post("/api/study/position", `{"media_id":"`+studyTestMedia+`","position_seconds":42}`))
	if again["updated_at"] != updatedAt {
		t.Fatalf("idempotent rewrite moved updated_at: %#v -> %#v", updatedAt, again["updated_at"])
	}

	// Completing preserves the position; advancing preserves completion.
	completed := studyJSON(t, post("/api/study/completed", `{"media_id":"`+studyTestMedia+`","completed":true}`))
	if completed["completed"] != true || completed["position_seconds"] != float64(42) {
		t.Fatalf("completion changed position: %#v", completed)
	}
	moved := studyJSON(t, post("/api/study/position", `{"media_id":"`+studyTestMedia+`","position_seconds":90}`))
	if moved["position_seconds"] != float64(90) || moved["completed"] != true {
		t.Fatalf("advancing cleared completion: %#v", moved)
	}
	cleared := studyJSON(t, post("/api/study/completed", `{"media_id":"`+studyTestMedia+`","completed":false}`))
	if cleared["completed"] != false || cleared["position_seconds"] != float64(90) {
		t.Fatalf("clearing completion moved position: %#v", cleared)
	}

	recent := httptest.NewRecorder()
	srv.ServeHTTP(recent, studyRequest(http.MethodGet, "/api/study/recent", cookie, "", false))
	items, ok := studyJSON(t, recent)["items"].([]any)
	if !ok || len(items) != 1 {
		t.Fatalf("recent items = %#v; want one", studyJSON(t, recent)["items"])
	}

	// Restart equivalence: a new process opens the same SQLite file and still reads state.
	if err := db.Close(); err != nil {
		t.Fatal(err)
	}
	reopened, err := storage.Open(ctx, path)
	if err != nil {
		t.Fatalf("reopen: %v", err)
	}
	defer reopened.Close()
	restarted := NewWithOptions(reopened, false)
	afterRestart := httptest.NewRecorder()
	restarted.ServeHTTP(afterRestart, studyRequest(http.MethodGet, "/api/study/progress?media_id="+studyTestMedia, cookie, "", false))
	if afterRestart.Code != http.StatusOK {
		t.Fatalf("restart read = %d: %s", afterRestart.Code, afterRestart.Body.String())
	}
	if got := studyJSON(t, afterRestart); got["recorded"] != true || got["position_seconds"] != float64(90) {
		t.Fatalf("progress did not survive restart: %#v", got)
	}
}

func TestStudyRejectsInvalidInputAndUnknownMedia(t *testing.T) {
	db, srv := openStudyServer(t)
	cookie := studySessionFor(t, db)

	get := func(target string, sameOriginHeader bool) *httptest.ResponseRecorder {
		recorder := httptest.NewRecorder()
		srv.ServeHTTP(recorder, studyRequest(http.MethodGet, target, cookie, "", sameOriginHeader))
		return recorder
	}
	post := func(body string) *httptest.ResponseRecorder {
		recorder := httptest.NewRecorder()
		srv.ServeHTTP(recorder, studyRequest(http.MethodPost, "/api/study/position", cookie, body, true))
		return recorder
	}

	cases := []struct {
		name string
		res  *httptest.ResponseRecorder
		want int
		code string
	}{
		{"negative position", post(`{"media_id":"` + studyTestMedia + `","position_seconds":-1}`), http.StatusUnprocessableEntity, "invalid_position"},
		{"unknown media read", get("/api/study/progress?media_id=missing.mp4", false), http.StatusNotFound, "media_not_found"},
		{"unknown media write", post(`{"media_id":"missing.mp4","position_seconds":5}`), http.StatusNotFound, "media_not_found"},
		{"traversal media", post(`{"media_id":"cat/../escape.mp4","position_seconds":5}`), http.StatusNotFound, "media_not_found"},
		{"missing media param", get("/api/study/progress", false), http.StatusBadRequest, "invalid_query"},
		{"malformed limit", get("/api/study/recent?limit=abc", false), http.StatusBadRequest, "invalid_query"},
		{"negative limit", get("/api/study/recent?limit=-1", false), http.StatusBadRequest, "invalid_query"},
		{"missing position field", post(`{"media_id":"` + studyTestMedia + `"}`), http.StatusBadRequest, "invalid_request"},
		{"unknown body field", post(`{"media_id":"` + studyTestMedia + `","position_seconds":5,"extra":1}`), http.StatusBadRequest, "invalid_request"},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			if tc.res.Code != tc.want {
				t.Fatalf("status = %d; want %d (body %s)", tc.res.Code, tc.want, tc.res.Body.String())
			}
			if tc.res.Header().Get("Cache-Control") != "private, no-store" {
				t.Fatalf("Cache-Control = %q; want private, no-store", tc.res.Header().Get("Cache-Control"))
			}
			if err := studyJSON(t, tc.res)["error"]; err != tc.code {
				t.Fatalf("error = %#v; want %q", err, tc.code)
			}
		})
	}

	// A malformed completion body is rejected, not silently treated as false.
	completed := httptest.NewRecorder()
	srv.ServeHTTP(completed, studyRequest(http.MethodPost, "/api/study/completed", cookie, `{"media_id":"`+studyTestMedia+`"}`, true))
	if completed.Code != http.StatusBadRequest {
		t.Fatalf("missing completed = %d; want 400 (body %s)", completed.Code, completed.Body.String())
	}

	// None of the rejected requests may have persisted state.
	recent := get("/api/study/recent", false)
	items, ok := studyJSON(t, recent)["items"].([]any)
	if !ok || len(items) != 0 {
		t.Fatalf("rejected requests persisted state: %#v", studyJSON(t, recent)["items"])
	}
}

func TestStudyMutationsRequireSameOrigin(t *testing.T) {
	db, srv := openStudyServer(t)
	cookie := studySessionFor(t, db)

	for _, tc := range []struct {
		method string
		target string
		body   string
	}{
		{http.MethodPost, "/api/study/position", `{"media_id":"` + studyTestMedia + `","position_seconds":5}`},
		{http.MethodPost, "/api/study/completed", `{"media_id":"` + studyTestMedia + `","completed":true}`},
	} {
		recorder := httptest.NewRecorder()
		srv.ServeHTTP(recorder, studyRequest(tc.method, tc.target, cookie, tc.body, false))
		if recorder.Code != http.StatusForbidden {
			t.Fatalf("%s = %d; want 403 (body %s)", tc.target, recorder.Code, recorder.Body.String())
		}
		if err := studyJSON(t, recorder)["error"]; err != "csrf_failed" {
			t.Fatalf("%s error = %#v; want csrf_failed", tc.target, err)
		}
	}

	recent := httptest.NewRecorder()
	srv.ServeHTTP(recent, studyRequest(http.MethodGet, "/api/study/recent", cookie, "", false))
	items, ok := studyJSON(t, recent)["items"].([]any)
	if !ok || len(items) != 0 {
		t.Fatalf("cross-origin mutation persisted state: %#v", studyJSON(t, recent)["items"])
	}
}

func TestStudyTemporarySessionCannotReachStudy(t *testing.T) {
	db := openTestDB(t)
	if err := db.BootstrapAdmin(context.Background(), "admin", "admin"); err != nil {
		t.Fatal(err)
	}
	user, err := db.UserByName(context.Background(), "admin")
	if err != nil {
		t.Fatal(err)
	}
	if !user.MustChangeCredentials {
		t.Fatal("temporary bootstrap did not require a credential change")
	}
	token, err := db.CreateSession(context.Background(), user.ID, time.Hour)
	if err != nil {
		t.Fatal(err)
	}
	srv := NewWithOptions(db, false)
	cookie := &http.Cookie{Name: sessionCookie, Value: token}

	recorder := httptest.NewRecorder()
	srv.ServeHTTP(recorder, studyRequest(http.MethodGet, "/api/study/recent", cookie, "", false))
	if recorder.Code != http.StatusForbidden {
		t.Fatalf("status = %d; want 403 (body %s)", recorder.Code, recorder.Body.String())
	}
	if err := studyJSON(t, recorder)["error"]; err != "credentials_change_required" {
		t.Fatalf("error = %#v; want credentials_change_required", err)
	}
}

func TestStudyRemovedMediaKeepsProgressButLeavesRecent(t *testing.T) {
	db, srv := openStudyServer(t)
	cookie := studySessionFor(t, db)

	if err := db.RemoveMedia(context.Background(), studyTestMedia); err != nil {
		t.Fatal(err)
	}

	absent := httptest.NewRecorder()
	srv.ServeHTTP(absent, studyRequest(http.MethodGet, "/api/study/progress?media_id="+studyTestMedia, cookie, "", false))
	if absent.Code != http.StatusOK {
		t.Fatalf("read removed media = %d: %s", absent.Code, absent.Body.String())
	}
	if got := studyJSON(t, absent); got["recorded"] != false {
		t.Fatalf("removed media read = %#v; want absent recorded=false", got)
	}

	written := httptest.NewRecorder()
	srv.ServeHTTP(written, studyRequest(http.MethodPost, "/api/study/position", cookie, `{"media_id":"`+studyTestMedia+`","position_seconds":42}`, true))
	if written.Code != http.StatusOK {
		t.Fatalf("write removed media = %d: %s", written.Code, written.Body.String())
	}
	done := httptest.NewRecorder()
	srv.ServeHTTP(done, studyRequest(http.MethodPost, "/api/study/completed", cookie, `{"media_id":"`+studyTestMedia+`","completed":true}`, true))
	if done.Code != http.StatusOK {
		t.Fatalf("complete removed media = %d: %s", done.Code, done.Body.String())
	}

	read := httptest.NewRecorder()
	srv.ServeHTTP(read, studyRequest(http.MethodGet, "/api/study/progress?media_id="+studyTestMedia, cookie, "", false))
	if got := studyJSON(t, read); got["recorded"] != true || got["position_seconds"] != float64(42) || got["completed"] != true {
		t.Fatalf("removed media progress = %#v", got)
	}

	recent := httptest.NewRecorder()
	srv.ServeHTTP(recent, studyRequest(http.MethodGet, "/api/study/recent", cookie, "", false))
	items, ok := studyJSON(t, recent)["items"].([]any)
	if !ok || len(items) != 0 {
		t.Fatalf("removed media should leave recent without erasing progress: %#v", studyJSON(t, recent)["items"])
	}
}

func TestStudyRecentLimitAndCapabilities(t *testing.T) {
	db, srv := openStudyServer(t)
	cookie := studySessionFor(t, db)

	seed := studyTestCatalog()
	for _, media := range []string{"cat/a.mp4", "cat/b.mp4", "cat/c.mp4"} {
		seed.Nodes[0].Children[0].Files = append(seed.Nodes[0].Children[0].Files, catalog.File{Name: media, Path: media, Size: 1})
	}
	if err := db.ReplaceCatalog(context.Background(), seed); err != nil {
		t.Fatal(err)
	}
	for _, media := range []string{"cat/a.mp4", "cat/b.mp4", "cat/c.mp4"} {
		body := `{"media_id":"` + media + `","position_seconds":1}`
		recorder := httptest.NewRecorder()
		srv.ServeHTTP(recorder, studyRequest(http.MethodPost, "/api/study/position", cookie, body, true))
		if recorder.Code != http.StatusOK {
			t.Fatalf("write %s = %d: %s", media, recorder.Code, recorder.Body.String())
		}
	}

	full := httptest.NewRecorder()
	srv.ServeHTTP(full, studyRequest(http.MethodGet, "/api/study/recent", cookie, "", false))
	if items, _ := studyJSON(t, full)["items"].([]any); len(items) != 3 {
		t.Fatalf("recent default = %d items; want 3", len(items))
	}
	limited := httptest.NewRecorder()
	srv.ServeHTTP(limited, studyRequest(http.MethodGet, "/api/study/recent?limit=2", cookie, "", false))
	if items, _ := studyJSON(t, limited)["items"].([]any); len(items) != 2 {
		t.Fatalf("recent limit=2 = %d items; want 2", len(items))
	}
	empty := httptest.NewRecorder()
	srv.ServeHTTP(empty, studyRequest(http.MethodGet, "/api/study/recent?limit=0", cookie, "", false))
	items, ok := studyJSON(t, empty)["items"].([]any)
	if !ok || len(items) != 0 {
		t.Fatalf("recent limit=0 = %#v; want empty array", studyJSON(t, empty)["items"])
	}

	capabilities := httptest.NewRecorder()
	srv.ServeHTTP(capabilities, studyRequest(http.MethodGet, "/api/study/capabilities", cookie, "", false))
	body := studyJSON(t, capabilities)
	if body["study"] != true || body["media_identity"] != "relative_path" || body["stable_media_identity"] != false {
		t.Fatalf("capabilities = %#v", body)
	}
	if body["recent_max_limit"] != float64(storage.MaxRecentStudy) || body["removal_keeps_progress"] != true {
		t.Fatalf("capabilities = %#v", body)
	}
}

func TestStudyEndpointsRejectUnsupportedMethods(t *testing.T) {
	_, srv := openStudyServer(t)
	for _, tc := range []struct {
		method string
		target string
	}{
		{http.MethodPost, "/api/study/recent"},
		{http.MethodPost, "/api/study/progress"},
		{http.MethodGet, "/api/study/position"},
		{http.MethodGet, "/api/study/completed"},
		{http.MethodPost, "/api/study/capabilities"},
	} {
		recorder := httptest.NewRecorder()
		srv.ServeHTTP(recorder, httptest.NewRequest(tc.method, tc.target, nil))
		if recorder.Code != http.StatusMethodNotAllowed {
			t.Fatalf("%s %s = %d; want 405", tc.method, tc.target, recorder.Code)
		}
	}
}

// twoUserStudyStore is a session-authenticated adapter fake that keys progress by owner,
// so the HTTP layer can be shown to isolate owners without a second real account.
type twoUserStudyStore struct {
	fakeStore
	users    map[string]storage.User
	progress map[int64]map[string]storage.StudyProgress
	removed  map[string]struct{}
}

func (s *twoUserStudyStore) UserForSession(_ context.Context, token string) (storage.User, error) {
	user, ok := s.users[token]
	if !ok {
		return storage.User{}, errTestStore
	}
	return user, nil
}

func (s *twoUserStudyStore) Catalog(context.Context) (catalog.Catalog, error) {
	return studyTestCatalog(), nil
}

func (s *twoUserStudyStore) RemovedMedia(context.Context) (map[string]struct{}, error) {
	return s.removed, nil
}

func (s *twoUserStudyStore) StudyProgress(_ context.Context, userID int64, mediaPath string) (storage.StudyProgress, bool, error) {
	state, ok := s.progress[userID][mediaPath]
	return state, ok, nil
}

func (s *twoUserStudyStore) RecordStudyPosition(_ context.Context, userID int64, mediaPath string, position int64) (storage.StudyProgress, error) {
	if position < 0 {
		return storage.StudyProgress{}, storage.ErrInvalidStudyPosition
	}
	if s.progress[userID] == nil {
		s.progress[userID] = map[string]storage.StudyProgress{}
	}
	state := s.progress[userID][mediaPath]
	state.MediaPath = mediaPath
	if state.PositionSeconds != position {
		state.UpdatedAt = time.Now().Unix()
	}
	state.PositionSeconds = position
	s.progress[userID][mediaPath] = state
	return state, nil
}

func (s *twoUserStudyStore) SetStudyCompleted(_ context.Context, userID int64, mediaPath string, completed bool) (storage.StudyProgress, error) {
	if s.progress[userID] == nil {
		s.progress[userID] = map[string]storage.StudyProgress{}
	}
	state := s.progress[userID][mediaPath]
	state.MediaPath = mediaPath
	state.Completed = completed
	s.progress[userID][mediaPath] = state
	return state, nil
}

func (s *twoUserStudyStore) RecentStudy(_ context.Context, userID int64, _ int) ([]storage.StudyProgress, error) {
	states := []storage.StudyProgress{}
	for _, state := range s.progress[userID] {
		states = append(states, state)
	}
	return states, nil
}

func TestStudyProgressIsIsolatedBetweenSessions(t *testing.T) {
	store := &twoUserStudyStore{
		users: map[string]storage.User{
			"token-a": {ID: 1, Username: "a", Role: "admin"},
			"token-b": {ID: 2, Username: "b", Role: "user"},
		},
		progress: map[int64]map[string]storage.StudyProgress{},
		removed:  map[string]struct{}{},
	}
	srv := newWithStore(store, false)
	cookieA := &http.Cookie{Name: sessionCookie, Value: "token-a"}
	cookieB := &http.Cookie{Name: sessionCookie, Value: "token-b"}

	write := func(cookie *http.Cookie, body string) *httptest.ResponseRecorder {
		recorder := httptest.NewRecorder()
		srv.ServeHTTP(recorder, studyRequest(http.MethodPost, "/api/study/position", cookie, body, true))
		return recorder
	}
	if code := write(cookieA, `{"media_id":"`+studyTestMedia+`","position_seconds":42}`).Code; code != http.StatusOK {
		t.Fatalf("owner A write = %d", code)
	}

	readBody := func(cookie *http.Cookie) map[string]any {
		recorder := httptest.NewRecorder()
		srv.ServeHTTP(recorder, studyRequest(http.MethodGet, "/api/study/progress?media_id="+studyTestMedia, cookie, "", false))
		return studyJSON(t, recorder)
	}
	if got := readBody(cookieA); got["recorded"] != true || got["position_seconds"] != float64(42) {
		t.Fatalf("owner A read = %#v", got)
	}
	if got := readBody(cookieB); got["recorded"] != false {
		t.Fatalf("owner B sees owner A progress: %#v", got)
	}

	if code := write(cookieB, `{"media_id":"`+studyTestMedia+`","position_seconds":7}`).Code; code != http.StatusOK {
		t.Fatalf("owner B write = %d", code)
	}
	if got := readBody(cookieA); got["position_seconds"] != float64(42) {
		t.Fatalf("owner B write changed owner A progress: %#v", got)
	}
	if got := readBody(cookieB); got["position_seconds"] != float64(7) {
		t.Fatalf("owner B read = %#v", got)
	}
}
