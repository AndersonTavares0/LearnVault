package storage

import (
	"context"
	"path/filepath"
	"testing"
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
