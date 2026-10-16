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
// de remoção nem as migrações de estudo, e confirma que abrir de novo aplica a cadeia
// incremental (006, 007, 008) sem perder dados.
func TestOpenMigratesLegacyGoDatabaseAdditively(t *testing.T) {
	path := filepath.Join(t.TempDir(), "legacy.db")
	db, err := Open(context.Background(), path)
	if err != nil {
		t.Fatal(err)
	}
	if err := db.BootstrapAdmin(context.Background(), "owner", "correct horse battery staple"); err != nil {
		t.Fatal(err)
	}
	if _, err := db.sql.Exec(`DELETE FROM schema_migrations WHERE version IN (6,7,8); DROP TABLE media_removal; DROP TABLE study_state`); err != nil {
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

func insertTestUser(t *testing.T, db *DB, username string) int64 {
	t.Helper()
	res, err := db.sql.Exec(`INSERT INTO users(username,password_hash,role) VALUES(?,?,?)`, username, "hash", "user")
	if err != nil {
		t.Fatal(err)
	}
	id, err := res.LastInsertId()
	if err != nil {
		t.Fatal(err)
	}
	return id
}

// TestStudyStatePersistsIsolatesAndOrders prova a persistência real: o progresso é por
// dono, a ordem é do mais recente ao mais antigo com desempate pelo caminho e o teto é
// aplicado. Nenhuma linha cruza donos.
func TestStudyStatePersistsIsolatesAndOrders(t *testing.T) {
	db := openTestDB(t)
	ctx := context.Background()
	userA := insertTestUser(t, db, "a")
	userB := insertTestUser(t, db, "b")

	for _, row := range []struct {
		user     int64
		path     string
		position int64
		done     int
		updated  int64
	}{
		{userA, "m1", 30, 0, 300},
		{userA, "m2", 20, 1, 200},
		{userA, "m0", 0, 0, 200},
		{userB, "m9", 5, 0, 999},
	} {
		if _, err := db.sql.Exec(`INSERT INTO study_state(user_id,media_path,position_seconds,completed,updated_at) VALUES(?,?,?,?,?)`,
			row.user, row.path, row.position, row.done, row.updated); err != nil {
			t.Fatal(err)
		}
	}

	states, err := db.RecentStudy(ctx, userA, 20)
	if err != nil {
		t.Fatal(err)
	}
	order := make([]string, 0, len(states))
	for _, state := range states {
		order = append(order, state.MediaPath)
	}
	if len(order) != 3 || order[0] != "m1" || order[1] != "m0" || order[2] != "m2" {
		t.Fatalf("RecentStudy order = %v; want [m1 m0 m2]", order)
	}
	if states[1].PositionSeconds != 0 || states[1].Completed {
		t.Fatalf("recorded zero-position row lost: %#v", states[1])
	}

	other, err := db.RecentStudy(ctx, userB, 20)
	if err != nil {
		t.Fatal(err)
	}
	if len(other) != 1 || other[0].MediaPath != "m9" {
		t.Fatalf("owner isolation broken: %#v", other)
	}

	if clamped, err := db.RecentStudy(ctx, userA, 1000); err != nil || len(clamped) != 3 {
		t.Fatalf("RecentStudy ceiling = %v, %v; want 3 rows", clamped, err)
	}
	if none, err := db.RecentStudy(ctx, userA, 0); err != nil || len(none) != 0 {
		t.Fatalf("RecentStudy limit 0 = %v, %v; want empty", none, err)
	}
}

// TestStudyMutationsAreIdempotentAndIndependent prova que reenviar um valor idêntico não
// move updated_at, que posição e conclusão são independentes e que uma posição negativa é
// recusada sem gravar nada.
func TestStudyMutationsAreIdempotentAndIndependent(t *testing.T) {
	db := openTestDB(t)
	ctx := context.Background()
	user := insertTestUser(t, db, "a")
	if _, err := db.sql.Exec(`INSERT INTO study_state(user_id,media_path,position_seconds,completed,updated_at) VALUES(?,?,?,?,?)`,
		user, "m1", 5, 0, 1000); err != nil {
		t.Fatal(err)
	}

	same, err := db.RecordStudyPosition(ctx, user, "m1", 5)
	if err != nil {
		t.Fatal(err)
	}
	if same.UpdatedAt != 1000 || same.Completed {
		t.Fatalf("same position changed state: %#v", same)
	}

	moved, err := db.RecordStudyPosition(ctx, user, "m1", 6)
	if err != nil {
		t.Fatal(err)
	}
	if moved.UpdatedAt <= 1000 || moved.PositionSeconds != 6 {
		t.Fatalf("real change not recorded: %#v", moved)
	}

	completed, err := db.SetStudyCompleted(ctx, user, "m1", true)
	if err != nil {
		t.Fatal(err)
	}
	if !completed.Completed || completed.PositionSeconds != 6 {
		t.Fatalf("completion moved position: %#v", completed)
	}
	repeated, err := db.SetStudyCompleted(ctx, user, "m1", true)
	if err != nil {
		t.Fatal(err)
	}
	if repeated.UpdatedAt != completed.UpdatedAt {
		t.Fatalf("same completion moved updated_at: %d -> %d", completed.UpdatedAt, repeated.UpdatedAt)
	}

	if _, err := db.RecordStudyPosition(ctx, user, "m-new", -1); !errors.Is(err, ErrInvalidStudyPosition) {
		t.Fatalf("negative position error = %v; want ErrInvalidStudyPosition", err)
	}
	if _, found, err := db.StudyProgress(ctx, user, "m-new"); err != nil || found {
		t.Fatalf("negative position persisted: found=%v err=%v", found, err)
	}
}

// TestStudyStateConstraints prova as restrições do schema persistido: conclusão booleana,
// posição não negativa, chave primária por dono/mídia e chave estrangeira para o usuário.
func TestStudyStateConstraints(t *testing.T) {
	db := openTestDB(t)
	user := insertTestUser(t, db, "a")

	if _, err := db.sql.Exec(`INSERT INTO study_state(user_id,media_path,position_seconds,completed,updated_at) VALUES(?,?,0,2,1)`, user); err == nil {
		t.Fatal("completed accepted a value other than 0/1")
	}
	if _, err := db.sql.Exec(`INSERT INTO study_state(user_id,media_path,position_seconds,completed,updated_at) VALUES(?,?,-1,0,1)`, user, "neg"); err == nil {
		t.Fatal("position accepted a negative value")
	}
	if _, err := db.sql.Exec(`INSERT INTO study_state(user_id,media_path,position_seconds,completed,updated_at) VALUES(?,?,0,0,1)`, user, "m1"); err != nil {
		t.Fatal(err)
	}
	if _, err := db.sql.Exec(`INSERT INTO study_state(user_id,media_path,position_seconds,completed,updated_at) VALUES(?,?,0,0,2)`, user, "m1"); err == nil {
		t.Fatal("duplicate owner/media pair accepted")
	}
	if _, err := db.sql.Exec(`INSERT INTO study_state(user_id,media_path,position_seconds,completed,updated_at) VALUES(?,?,0,0,1)`, int64(99999), "m2"); err == nil {
		t.Fatal("unknown user accepted by foreign key")
	}
}

// TestOpenMigratesLegacyGoDatabaseWithStudyProgressAdditively simula um banco Go na
// versão 6, sem a tabela de progresso, e confirma que reabrir aplica a migração aditiva
// sem perder dados já existentes.
func TestOpenMigratesLegacyGoDatabaseWithStudyProgressAdditively(t *testing.T) {
	path := filepath.Join(t.TempDir(), "legacy-study.db")
	ctx := context.Background()
	db, err := Open(ctx, path)
	if err != nil {
		t.Fatal(err)
	}
	if err := db.BootstrapAdmin(ctx, "owner", "correct horse battery staple"); err != nil {
		t.Fatal(err)
	}
	if _, err := db.sql.Exec(`DELETE FROM schema_migrations WHERE version=7; DROP TABLE study_state`); err != nil {
		t.Fatal(err)
	}
	if err := db.Close(); err != nil {
		t.Fatal(err)
	}

	reopened, err := Open(ctx, path)
	if err != nil {
		t.Fatalf("reopening legacy Go database failed: %v", err)
	}
	defer reopened.Close()
	if ok, err := reopened.VerifyPassword(ctx, "owner", "correct horse battery staple"); err != nil || !ok {
		t.Fatalf("legacy data lost after incremental study migration: ok=%v err=%v", ok, err)
	}
	var exists int
	if err := reopened.sql.QueryRow(`SELECT count(*) FROM sqlite_master WHERE type='table' AND name='study_state'`).Scan(&exists); err != nil {
		t.Fatal(err)
	}
	if exists != 1 {
		t.Fatal("incremental migration did not create study_state")
	}
	if action := foreignKeyOnDelete(t, reopened, "study_state", "users"); action != "NO ACTION" {
		t.Fatalf("study_state user FK on_delete = %q; want NO ACTION", action)
	}
	user, err := reopened.UserByName(ctx, "owner")
	if err != nil {
		t.Fatal(err)
	}
	if _, err := reopened.RecordStudyPosition(ctx, user.ID, "cat/a.mp4", 12); err != nil {
		t.Fatal(err)
	}
}

// foreignKeyOnDelete reads the declared ON DELETE action of the foreign key that `table`
// holds toward `ref`, so a schema assertion does not depend on parsing DDL text.
func foreignKeyOnDelete(t *testing.T, db *DB, table, ref string) string {
	t.Helper()
	rows, err := db.sql.Query(`PRAGMA foreign_key_list(` + table + `)`)
	if err != nil {
		t.Fatal(err)
	}
	defer rows.Close()
	for rows.Next() {
		var id, seq int
		var refTable, from, to, onUpdate, onDelete, match string
		if err := rows.Scan(&id, &seq, &refTable, &from, &to, &onUpdate, &onDelete, &match); err != nil {
			t.Fatal(err)
		}
		if refTable == ref {
			return onDelete
		}
	}
	if err := rows.Err(); err != nil {
		t.Fatal(err)
	}
	t.Fatalf("no foreign key from %s to %s", table, ref)
	return ""
}

// TestStudyStateForeignKeyPreservesProgressOnUserDelete prova a semântica de preservação:
// a chave estrangeira para o usuário é `NO ACTION`, então excluir o usuário falha alto em
// vez de apagar o progresso silenciosamente (não há rota de exclusão de usuário).
func TestStudyStateForeignKeyPreservesProgressOnUserDelete(t *testing.T) {
	db := openTestDB(t)
	ctx := context.Background()
	user := insertTestUser(t, db, "a")
	if _, err := db.RecordStudyPosition(ctx, user, "cat/a.mp4", 12); err != nil {
		t.Fatal(err)
	}
	if action := foreignKeyOnDelete(t, db, "study_state", "users"); action != "NO ACTION" {
		t.Fatalf("study_state user FK on_delete = %q; want NO ACTION", action)
	}
	if _, err := db.sql.Exec(`DELETE FROM users WHERE id=?`, user); err == nil {
		t.Fatal("deleting a user cascaded study progress; want NO ACTION preservation")
	}
	if _, found, err := db.StudyProgress(ctx, user, "cat/a.mp4"); err != nil || !found {
		t.Fatalf("study progress not preserved after blocked user delete: found=%v err=%v", found, err)
	}
}

// TestRemoveMediaRecordsMediaClassification prova que a classificação do scanner é
// capturada no tombstone no momento da remoção: mídia vira is_media 1, material/legenda
// vira 0, e os dois `.ts` (grande/mídia e pequeno/material) são distinguidos pelo tamanho.
func TestRemoveMediaRecordsMediaClassification(t *testing.T) {
	db := openTestDB(t)
	ctx := context.Background()
	value := catalog.Catalog{Nodes: []catalog.Node{{
		Title: "cat", Path: "cat", Role: "category",
		Children: []catalog.Node{{
			Title: "aula", Path: "cat/lesson", Role: "lesson",
			Files: []catalog.File{
				{Name: "video.mp4", Path: "cat/lesson/video.mp4", Size: 10},
				{Name: "movie.ts", Path: "cat/lesson/movie.ts", Size: 2 * 1024 * 1024},
				{Name: "segment.ts", Path: "cat/lesson/segment.ts", Size: 512},
				{Name: "notes.pdf", Path: "cat/lesson/notes.pdf", Size: 10},
			},
		}},
	}}}
	if err := db.ReplaceCatalog(ctx, value); err != nil {
		t.Fatal(err)
	}
	for _, path := range []string{"cat/lesson/video.mp4", "cat/lesson/movie.ts", "cat/lesson/segment.ts", "cat/lesson/notes.pdf"} {
		if err := db.RemoveMedia(ctx, path); err != nil {
			t.Fatal(err)
		}
	}
	want := map[string]int64{
		"cat/lesson/video.mp4":  1,
		"cat/lesson/movie.ts":   1,
		"cat/lesson/segment.ts": 0,
		"cat/lesson/notes.pdf":  0,
	}
	for path, expected := range want {
		var isMedia sql.NullInt64
		if err := db.sql.QueryRow(`SELECT is_media FROM media_removal WHERE relative_path=?`, path).Scan(&isMedia); err != nil {
			t.Fatal(err)
		}
		if !isMedia.Valid || isMedia.Int64 != expected {
			t.Fatalf("is_media for %s = %+v; want %d", path, isMedia, expected)
		}
	}
}

// TestStudyRemovedMediaLegacyRowsAreConservative prova a compatibilidade com linhas de
// remoção antigas, sem classificação (`NULL`): extensões inequivocamente de mídia são
// aceitas e `.ts` (ambíguo sem tamanho) e legendas são recusados; um caminho sem tombstone
// nunca é elegível.
func TestStudyRemovedMediaLegacyRowsAreConservative(t *testing.T) {
	db := openTestDB(t)
	ctx := context.Background()
	for _, path := range []string{"cat/legacy.mp4", "cat/legacy.ts", "cat/subtitle.srt"} {
		if _, err := db.sql.Exec(`INSERT INTO media_removal(relative_path,removed_at) VALUES(?,?)`, path, 1); err != nil {
			t.Fatal(err)
		}
	}
	for path, want := range map[string]bool{
		"cat/legacy.mp4":   true,
		"cat/legacy.ts":    false,
		"cat/subtitle.srt": false,
		"cat/absent.mp4":   false,
	} {
		got, err := db.StudyRemovedMedia(ctx, path)
		if err != nil || got != want {
			t.Fatalf("StudyRemovedMedia(%s) = %v, %v; want %v", path, got, err, want)
		}
	}
}

// TestOpenAddsRemovalMediaMetadataAdditively simula um banco lançado na versão 6, sem
// `media_removal.is_media` nem `study_state`, e confirma que reabrir aplica as migrações
// 007 e 008 aditivamente, sem perder a tabela de remoção.
func TestOpenAddsRemovalMediaMetadataAdditively(t *testing.T) {
	path := filepath.Join(t.TempDir(), "legacy-removal.db")
	ctx := context.Background()
	db, err := Open(ctx, path)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := db.sql.Exec(`DELETE FROM schema_migrations WHERE version IN (6,7,8); DROP TABLE study_state; DROP TABLE media_removal`); err != nil {
		t.Fatal(err)
	}
	if err := db.Close(); err != nil {
		t.Fatal(err)
	}

	reopened, err := Open(ctx, path)
	if err != nil {
		t.Fatalf("reopening legacy v6 database failed: %v", err)
	}
	defer reopened.Close()

	var hasColumn bool
	rows, err := reopened.sql.Query(`PRAGMA table_info(media_removal)`)
	if err != nil {
		t.Fatal(err)
	}
	for rows.Next() {
		var cid, notNull, pk int
		var name, columnType string
		var defaultValue sql.NullString
		if err := rows.Scan(&cid, &name, &columnType, &notNull, &defaultValue, &pk); err != nil {
			rows.Close()
			t.Fatal(err)
		}
		if name == "is_media" {
			hasColumn = true
		}
	}
	rows.Close()
	if !hasColumn {
		t.Fatal("incremental migration did not add media_removal.is_media")
	}
	if eligible, err := reopened.StudyRemovedMedia(ctx, "cat/absent.mp4"); err != nil || eligible {
		t.Fatalf("path without tombstone = %v, %v; want not eligible", eligible, err)
	}
	if err := reopened.RemoveMedia(ctx, "cat/a.mp4"); err != nil {
		t.Fatal(err)
	}
}
