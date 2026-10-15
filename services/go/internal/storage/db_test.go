package storage

import (
	"context"
	"database/sql"
	"errors"
	"path/filepath"
	"reflect"
	"testing"
	"time"

	"learnvault/internal/catalog"
)

func TestOpenMigratesDatabaseAndBootstrapsAdmin(t *testing.T) {
	db, err := Open(context.Background(), filepath.Join(t.TempDir(), "learnvault.db"))
	if err != nil {
		t.Fatal(err)
	}
	defer db.Close()

	if err := db.BootstrapAdmin(context.Background(), "owner", "correct horse battery staple"); err != nil {
		t.Fatal(err)
	}
	ok, err := db.VerifyPassword(context.Background(), "owner", "correct horse battery staple")
	if err != nil || !ok {
		t.Fatalf("admin credentials not verified: ok=%v err=%v", ok, err)
	}
	ok, err = db.VerifyPassword(context.Background(), "owner", "wrong password")
	if err != nil || ok {
		t.Fatalf("wrong password accepted: ok=%v err=%v", ok, err)
	}
}

func TestBootstrapAdminAcceptsTemporaryAdminDefaultsAndStoresOnlyHash(t *testing.T) {
	db := openTestDB(t)
	if err := db.BootstrapAdmin(context.Background(), "admin", "admin"); err != nil {
		t.Fatal(err)
	}
	u, err := db.UserByName(context.Background(), "admin")
	if err != nil {
		t.Fatal(err)
	}
	if !u.MustChangeCredentials || !u.IsAdmin {
		t.Fatalf("temporary bootstrap state = %+v", u)
	}
	var stored string
	if err := db.sql.QueryRow(`SELECT password_hash FROM users WHERE username='admin'`).Scan(&stored); err != nil {
		t.Fatal(err)
	}
	if stored == "admin" || stored == "" {
		t.Fatalf("plaintext temporary password stored: %q", stored)
	}
}

func TestBootstrapAdminDoesNotReplaceExistingAccount(t *testing.T) {
	db, err := Open(context.Background(), filepath.Join(t.TempDir(), "learnvault.db"))
	if err != nil {
		t.Fatal(err)
	}
	defer db.Close()
	if err := db.BootstrapAdmin(context.Background(), "owner", "first password"); err != nil {
		t.Fatal(err)
	}
	if err := db.BootstrapAdmin(context.Background(), "owner", "second password"); err != nil {
		t.Fatal(err)
	}
	ok, err := db.VerifyPassword(context.Background(), "owner", "first password")
	if err != nil || !ok {
		t.Fatalf("bootstrap overwrote initial credentials: ok=%v err=%v", ok, err)
	}
}

func TestSessionLifecycle(t *testing.T) {
	db := openTestDB(t)
	ctx := context.Background()
	if err := db.BootstrapAdmin(ctx, "owner", "correct horse battery staple"); err != nil {
		t.Fatal(err)
	}

	hasAdmin, err := db.HasAdmin(ctx)
	if err != nil || !hasAdmin {
		t.Fatalf("HasAdmin() = %v, %v; want true, nil", hasAdmin, err)
	}
	user, err := db.UserByName(ctx, "OWNER")
	if err != nil || user.Username != "owner" || user.Role != "admin" {
		t.Fatalf("UserByName() = %+v, %v", user, err)
	}

	token, err := db.CreateSession(ctx, user.ID, time.Hour)
	if err != nil {
		t.Fatal(err)
	}
	if got, err := db.UserForSession(ctx, token); err != nil || got.ID != user.ID {
		t.Fatalf("UserForSession() = %+v, %v; want user %d", got, err, user.ID)
	}
	if _, err := db.UserForSession(ctx, "unknown-token"); !errors.Is(err, sql.ErrNoRows) {
		t.Fatalf("unknown token error = %v; want sql.ErrNoRows", err)
	}

	expired, err := db.CreateSession(ctx, user.ID, -time.Second)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := db.UserForSession(ctx, expired); !errors.Is(err, sql.ErrNoRows) {
		t.Fatalf("expired token error = %v; want sql.ErrNoRows", err)
	}
	if err := db.DeleteSession(ctx, token); err != nil {
		t.Fatal(err)
	}
	if _, err := db.UserForSession(ctx, token); !errors.Is(err, sql.ErrNoRows) {
		t.Fatalf("deleted token error = %v; want sql.ErrNoRows", err)
	}
}

func TestCatalogRoundTripReplacementAndRollback(t *testing.T) {
	db := openTestDB(t)
	ctx := context.Background()
	want := catalog.Catalog{Categories: []catalog.Category{{
		Title: "Programming", Path: "programming",
		Courses: []catalog.Course{{Title: "Go", Path: "programming/go", Modules: []catalog.Module{{
			Title: "Basics", Path: "programming/go/basics", Lessons: []catalog.Lesson{{
				Title: "First lesson", Path: "programming/go/basics/first",
				Media:     []catalog.File{{Name: "video.mp4", Path: "programming/go/basics/first/video.mp4", Size: 42}},
				Subtitles: []catalog.File{{Name: "video.srt", Path: "programming/go/basics/first/video.srt", Size: 7}},
				Materials: []catalog.File{{Name: "notes.pdf", Path: "programming/go/basics/first/notes.pdf", Size: 99}},
			}},
		}}}},
	}}}
	if err := db.ReplaceCatalog(ctx, want); err != nil {
		t.Fatal(err)
	}
	got, err := db.Catalog(ctx)
	if err != nil {
		t.Fatal(err)
	}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("Catalog() = %#v; want %#v", got, want)
	}

	bad := catalog.Catalog{Categories: []catalog.Category{
		{Title: "Duplicate one", Path: "duplicate"},
		{Title: "Duplicate two", Path: "duplicate"},
	}}
	if err := db.ReplaceCatalog(ctx, bad); err == nil {
		t.Fatal("ReplaceCatalog() accepted duplicate category paths")
	}
	got, err = db.Catalog(ctx)
	if err != nil || !reflect.DeepEqual(got, want) {
		t.Fatalf("failed replacement changed existing catalog: got %#v, err %v", got, err)
	}

	if err := db.ReplaceCatalog(ctx, catalog.Catalog{}); err != nil {
		t.Fatal(err)
	}
	got, err = db.Catalog(ctx)
	if err != nil || len(got.Categories) != 0 {
		t.Fatalf("empty replacement = %#v, %v; want no categories", got, err)
	}
}

func openTestDB(t *testing.T) *DB {
	t.Helper()
	db, err := Open(context.Background(), filepath.Join(t.TempDir(), "learnvault.db"))
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = db.Close() })
	return db
}

func catalogFiles(value catalog.Catalog) map[string]bool {
	files := map[string]bool{}
	for _, category := range value.Categories {
		for _, course := range category.Courses {
			for _, module := range course.Modules {
				for _, lesson := range module.Lessons {
					for _, file := range append(append(lesson.Media, lesson.Subtitles...), lesson.Materials...) {
						files[file.Path] = true
					}
				}
			}
		}
	}
	for _, file := range value.LooseFiles {
		files[file.Path] = true
	}
	return files
}

// TestRemoveMediaTombstoneSurvivesRescan cobre o tombstone persistente: a remoção não é
// apagada nem reescrita por um rescan, a identidade (caminho canônico) é preservada, o
// catálogo lido deixa de listar a mídia removida e os demais registros permanecem.
func TestRemoveMediaTombstoneSurvivesRescan(t *testing.T) {
	db := openTestDB(t)
	ctx := context.Background()
	removedPath := "cat/course/module/lesson/removed.mp4"
	keptPath := "cat/course/module/lesson/kept.mp4"
	value := catalog.Catalog{Categories: []catalog.Category{{
		Title: "Cat", Path: "cat",
		Courses: []catalog.Course{{Title: "Course", Path: "cat/course", Modules: []catalog.Module{{
			Title: "Module", Path: "cat/course/module", Lessons: []catalog.Lesson{{
				Title: "Lesson", Path: "cat/course/module/lesson",
				Media: []catalog.File{{Name: "removed.mp4", Path: removedPath, Size: 1}, {Name: "kept.mp4", Path: keptPath, Size: 1}},
			}},
		}}}},
	}}}
	if err := db.ReplaceCatalog(ctx, value); err != nil {
		t.Fatal(err)
	}
	if err := db.RemoveMedia(ctx, removedPath); err != nil {
		t.Fatal(err)
	}
	var firstRemovedAt int64
	if err := db.sql.QueryRowContext(ctx, `SELECT removed_at FROM media_removal WHERE relative_path=?`, removedPath).Scan(&firstRemovedAt); err != nil {
		t.Fatal(err)
	}

	// O rescan repete a substituição com o arquivo ainda presente no disco.
	if err := db.ReplaceCatalog(ctx, value); err != nil {
		t.Fatal(err)
	}
	var secondRemovedAt int64
	if err := db.sql.QueryRowContext(ctx, `SELECT removed_at FROM media_removal WHERE relative_path=?`, removedPath).Scan(&secondRemovedAt); err != nil {
		t.Fatal(err)
	}
	if firstRemovedAt != secondRemovedAt {
		t.Fatalf("rescan rewrote removed_at: %d -> %d", firstRemovedAt, secondRemovedAt)
	}

	removed, err := db.RemovedMedia(ctx)
	if err != nil || len(removed) != 1 {
		t.Fatalf("RemovedMedia() = %v, %v; want exactly the removed path", removed, err)
	}
	if _, ok := removed[removedPath]; !ok {
		t.Fatalf("removal identity not preserved: %v", removed)
	}

	got, err := db.Catalog(ctx)
	if err != nil {
		t.Fatal(err)
	}
	files := catalogFiles(got)
	if files[removedPath] {
		t.Fatal("removed media still listed by Catalog()")
	}
	if !files[keptPath] {
		t.Fatal("rescan dropped an unrelated file")
	}

	// Repetir a remoção é idempotente e não reescreve o carimbo original.
	if err := db.RemoveMedia(ctx, removedPath); err != nil {
		t.Fatal(err)
	}
	var thirdRemovedAt int64
	if err := db.sql.QueryRowContext(ctx, `SELECT removed_at FROM media_removal WHERE relative_path=?`, removedPath).Scan(&thirdRemovedAt); err != nil {
		t.Fatal(err)
	}
	if thirdRemovedAt != firstRemovedAt {
		t.Fatalf("repeat removal rewrote removed_at: %d -> %d", firstRemovedAt, thirdRemovedAt)
	}
}

// TestOpenMigratesLegacyGoDatabaseAdditively simula um banco Go na versão 5, sem a tabela
// de remoção, e confirma que abrir de novo aplica a migração incremental sem perder dados.
func TestOpenMigratesLegacyGoDatabaseAdditively(t *testing.T) {
	path := filepath.Join(t.TempDir(), "legacy.db")
	db, err := Open(context.Background(), path)
	if err != nil {
		t.Fatal(err)
	}
	if err := db.BootstrapAdmin(context.Background(), "owner", "correct horse battery staple"); err != nil {
		t.Fatal(err)
	}
	if _, err := db.sql.Exec(`DELETE FROM schema_migrations WHERE version=6; DROP TABLE media_removal`); err != nil {
		t.Fatal(err)
	}
	if err := db.Close(); err != nil {
		t.Fatal(err)
	}

	reopened, err := Open(context.Background(), path)
	if err != nil {
		t.Fatalf("reopening legacy Go database failed: %v", err)
	}
	defer reopened.Close()
	if ok, err := reopened.VerifyPassword(context.Background(), "owner", "correct horse battery staple"); err != nil || !ok {
		t.Fatalf("legacy data lost after incremental migration: ok=%v err=%v", ok, err)
	}
	var exists int
	if err := reopened.sql.QueryRow(`SELECT count(*) FROM sqlite_master WHERE type='table' AND name='media_removal'`).Scan(&exists); err != nil {
		t.Fatal(err)
	}
	if exists != 1 {
		t.Fatal("incremental migration did not create media_removal")
	}
	if err := reopened.RemoveMedia(context.Background(), "cat/a.mp4"); err != nil {
		t.Fatal(err)
	}
}
