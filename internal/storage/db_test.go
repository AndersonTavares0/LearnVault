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
