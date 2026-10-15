package storage

import (
	"context"
	"crypto/rand"
	"crypto/sha256"
	"crypto/subtle"
	"database/sql"
	"embed"
	"encoding/base64"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"time"

	"golang.org/x/crypto/argon2"
	"learnvault/internal/catalog"
	_ "modernc.org/sqlite"
)

//go:embed migrations/*.sql
var migrationFiles embed.FS

type DB struct{ sql *sql.DB }

type User struct {
	ID       int64  `json:"-"`
	UserID   int64  `json:"user_id"`
	Username string `json:"username"`
	Role     string `json:"-"`
	IsAdmin  bool `json:"is_admin"`
	MustChangeCredentials bool `json:"must_change_credentials"`
}

func Open(ctx context.Context, path string) (*DB, error) {
	if err := ensureParent(path); err != nil {
		return nil, err
	}
	db, err := sql.Open("sqlite", "file:"+path+"?_pragma=foreign_keys(1)&_pragma=journal_mode(WAL)&_pragma=busy_timeout(5000)")
	if err != nil {
		return nil, err
	}
	db.SetMaxOpenConns(1)
	if err := db.PingContext(ctx); err != nil {
		db.Close()
		return nil, err
	}
	if _, err := db.ExecContext(ctx, `CREATE TABLE IF NOT EXISTS schema_migrations (version INTEGER PRIMARY KEY, applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP)`); err != nil {
		db.Close()
		return nil, err
	}
	entries, err := migrationFiles.ReadDir("migrations")
	if err != nil {
		db.Close()
		return nil, err
	}
	for _, entry := range entries {
		var applied int
		if _, err := fmt.Sscanf(entry.Name(), "%d_", &applied); err != nil { db.Close(); return nil, fmt.Errorf("invalid migration %s", entry.Name()) }
		var exists int
		if err := db.QueryRowContext(ctx, `SELECT count(*) FROM schema_migrations WHERE version=?`, applied).Scan(&exists); err != nil { db.Close(); return nil, err }
		if exists > 0 { continue }
		contents, err := migrationFiles.ReadFile("migrations/" + entry.Name())
		if err != nil {
			db.Close()
			return nil, err
		}
		if _, err := db.ExecContext(ctx, string(contents)); err != nil {
			db.Close()
			return nil, fmt.Errorf("apply migration %s: %w", entry.Name(), err)
		}
	}
	return &DB{sql: db}, nil
}

func ensureParent(path string) error {
	return os.MkdirAll(filepath.Dir(path), 0o700)
}

func (d *DB) Close() error                   { return d.sql.Close() }
func (d *DB) Ping(ctx context.Context) error { return d.sql.PingContext(ctx) }

func (d *DB) BootstrapAdmin(ctx context.Context, username, password string) error {
	username = strings.TrimSpace(username)
	if !(username == "admin" && password == "admin") && (len(username) < 2 || len(username) > 64 || len(password) < 12 || len(password) > 1024) {
		return errors.New("admin username must be 2-64 characters and password 12-1024 characters")
	}
	hash, err := hashPassword(password)
	if err != nil {
		return err
	}
	mustChange := 0
	if username == "admin" && password == "admin" { mustChange = 1 }
	_, err = d.sql.ExecContext(ctx, `INSERT OR IGNORE INTO users(username,password_hash,role,must_change_credentials) SELECT ?,?,'admin',? WHERE NOT EXISTS (SELECT 1 FROM users WHERE role='admin')`, username, hash, mustChange)
	return err
}

func (d *DB) UpdateCredentialsAndSession(ctx context.Context, userID int64, username, password, token string, lifetime time.Duration) error {
	if len([]rune(username)) < 2 || len([]rune(username)) > 64 || strings.TrimSpace(username) != username || len([]rune(password)) < 12 || len([]rune(password)) > 1024 { return errors.New("invalid credentials") }
	hash, err := hashPassword(password); if err != nil { return err }
	tx, err := d.sql.BeginTx(ctx, nil); if err != nil { return err }; defer tx.Rollback()
	if _, err = tx.ExecContext(ctx, `UPDATE users SET username=?, password_hash=?, must_change_credentials=0 WHERE id=?`, username, hash, userID); err != nil { return err }
	if _, err = tx.ExecContext(ctx, `DELETE FROM sessions WHERE user_id=?`, userID); err != nil { return err }
	expires := time.Now().UTC().Add(lifetime).Format(time.RFC3339Nano)
	if _, err = tx.ExecContext(ctx, `INSERT INTO sessions(token_hash,user_id,expires_at) VALUES(?,?,?)`, tokenHash(token), userID, expires); err != nil { return err }
	return tx.Commit()
}

func (d *DB) HasAdmin(ctx context.Context) (bool, error) {
	var count int
	err := d.sql.QueryRowContext(ctx, `SELECT count(*) FROM users WHERE role='admin'`).Scan(&count)
	return count > 0, err
}

func (d *DB) VerifyPassword(ctx context.Context, username, password string) (bool, error) {
	var stored string
	if err := d.sql.QueryRowContext(ctx, `SELECT password_hash FROM users WHERE username=? COLLATE NOCASE`, strings.TrimSpace(username)).Scan(&stored); err != nil {
		if errors.Is(err, sql.ErrNoRows) {
			return false, nil
		}
		return false, err
	}
	return verifyPassword(stored, password), nil
}

func (d *DB) UserByName(ctx context.Context, username string) (User, error) {
	var u User
	err := d.sql.QueryRowContext(ctx, `SELECT id,username,role,must_change_credentials FROM users WHERE username=? COLLATE NOCASE`, username).Scan(&u.ID, &u.Username, &u.Role, &u.MustChangeCredentials); u.UserID = u.ID; u.IsAdmin = u.Role == "admin"
	return u, err
}

func (d *DB) CreateSession(ctx context.Context, userID int64, lifetime time.Duration) (string, error) {
	var raw [32]byte
	if _, err := rand.Read(raw[:]); err != nil {
		return "", err
	}
	token := base64.RawURLEncoding.EncodeToString(raw[:])
	expires := time.Now().UTC().Add(lifetime).Format(time.RFC3339Nano)
	_, err := d.sql.ExecContext(ctx, `INSERT INTO sessions(token_hash,user_id,expires_at) VALUES(?,?,?)`, tokenHash(token), userID, expires)
	return token, err
}

func (d *DB) UserForSession(ctx context.Context, token string) (User, error) {
	var u User
	err := d.sql.QueryRowContext(ctx, `SELECT u.id,u.username,u.role,u.must_change_credentials FROM sessions s JOIN users u ON u.id=s.user_id WHERE s.token_hash=? AND s.expires_at>?`, tokenHash(token), time.Now().UTC().Format(time.RFC3339Nano)).Scan(&u.ID, &u.Username, &u.Role, &u.MustChangeCredentials); u.UserID = u.ID; u.IsAdmin = u.Role == "admin"
	return u, err
}

func (d *DB) UpdateCredentials(ctx context.Context, userID int64, username, password string) error {
	if len([]rune(username)) < 2 || len([]rune(username)) > 64 || strings.TrimSpace(username) != username || len([]rune(password)) < 12 || len([]rune(password)) > 1024 { return errors.New("invalid credentials") }
	hash, err := hashPassword(password); if err != nil { return err }
	tx, err := d.sql.BeginTx(ctx, nil); if err != nil { return err }; defer tx.Rollback()
	if _, err = tx.ExecContext(ctx, `UPDATE users SET username=?, password_hash=?, must_change_credentials=0 WHERE id=?`, username, hash, userID); err != nil { return err }
	if _, err = tx.ExecContext(ctx, `DELETE FROM sessions WHERE user_id=?`, userID); err != nil { return err }
	return tx.Commit()
}

func (d *DB) DeleteSession(ctx context.Context, token string) error {
	_, err := d.sql.ExecContext(ctx, `DELETE FROM sessions WHERE token_hash=?`, tokenHash(token))
	return err
}

func (d *DB) ReplaceCatalog(ctx context.Context, value catalog.Catalog) error {
	tx, err := d.sql.BeginTx(ctx, nil)
	if err != nil {
		return err
	}
	defer tx.Rollback()
	if _, err = tx.ExecContext(ctx, `DELETE FROM catalog_categories`); err != nil {
		return err
	}
	encoded, err := json.Marshal(value.Nodes)
	if err != nil {
		return err
	}
	if _, err = tx.ExecContext(ctx, `DELETE FROM catalog_nodes_json`); err != nil {
		return err
	}
	if _, err = tx.ExecContext(ctx, `INSERT INTO catalog_nodes_json(id,value) VALUES(1,?)`, string(encoded)); err != nil {
		return err
	}
	if _, err = tx.ExecContext(ctx, `DELETE FROM catalog_root_files`); err != nil {
		return err
	}
	for _, file := range value.LooseFiles {
		if _, err = tx.ExecContext(ctx, `INSERT INTO catalog_root_files(path,name,size) VALUES(?,?,?)`, file.Path, file.Name, file.Size); err != nil {
			return err
		}
	}
	for _, category := range value.Categories {
		res, err := tx.ExecContext(ctx, `INSERT INTO catalog_categories(path,title) VALUES(?,?)`, category.Path, category.Title)
		if err != nil {
			return err
		}
		categoryID, err := res.LastInsertId()
		if err != nil {
			return err
		}
		for _, course := range category.Courses {
			res, err := tx.ExecContext(ctx, `INSERT INTO catalog_courses(category_id,path,title) VALUES(?,?,?)`, categoryID, course.Path, course.Title)
			if err != nil {
				return err
			}
			courseID, err := res.LastInsertId()
			if err != nil {
				return err
			}
			for _, module := range course.Modules {
				res, err := tx.ExecContext(ctx, `INSERT INTO catalog_modules(course_id,path,title) VALUES(?,?,?)`, courseID, module.Path, module.Title)
				if err != nil {
					return err
				}
				moduleID, err := res.LastInsertId()
				if err != nil {
					return err
				}
				for _, lesson := range module.Lessons {
					res, err := tx.ExecContext(ctx, `INSERT INTO catalog_lessons(module_id,path,title) VALUES(?,?,?)`, moduleID, lesson.Path, lesson.Title)
					if err != nil {
						return err
					}
					lessonID, err := res.LastInsertId()
					if err != nil {
						return err
					}
					for _, file := range lesson.Media {
						if err := insertCatalogFile(ctx, tx, lessonID, file, "media"); err != nil {
							return err
						}
					}
					for _, file := range lesson.Subtitles {
						if err := insertCatalogFile(ctx, tx, lessonID, file, "subtitle"); err != nil {
							return err
						}
					}
					for _, file := range lesson.Materials {
						if err := insertCatalogFile(ctx, tx, lessonID, file, "material"); err != nil {
							return err
						}
					}
				}
			}
		}
	}
	return tx.Commit()
}

func insertCatalogFile(ctx context.Context, tx *sql.Tx, lessonID int64, file catalog.File, kind string) error {
	_, err := tx.ExecContext(ctx, `INSERT INTO catalog_files(lesson_id,path,name,kind,size) VALUES(?,?,?,?,?)`, lessonID, file.Path, file.Name, kind, file.Size)
	return err
}

func (d *DB) Catalog(ctx context.Context) (catalog.Catalog, error) {
	var result catalog.Catalog
	var nodesJSON string
	if err := d.sql.QueryRowContext(ctx, `SELECT value FROM catalog_nodes_json WHERE id=1`).Scan(&nodesJSON); err == nil {
		if err := json.Unmarshal([]byte(nodesJSON), &result.Nodes); err != nil {
			return result, err
		}
	} else if !errors.Is(err, sql.ErrNoRows) {
		return result, err
	}
	rootRows, err := d.sql.QueryContext(ctx, `SELECT name,path,size FROM catalog_root_files ORDER BY path`)
	if err != nil {
		return result, err
	}
	for rootRows.Next() {
		var file catalog.File
		if err := rootRows.Scan(&file.Name, &file.Path, &file.Size); err != nil {
			rootRows.Close()
			return result, err
		}
		result.LooseFiles = append(result.LooseFiles, file)
	}
	if err := rootRows.Err(); err != nil {
		rootRows.Close()
		return result, err
	}
	rootRows.Close()
	rows, err := d.sql.QueryContext(ctx, `SELECT c.id,c.path,c.title, co.id,co.path,co.title, m.id,m.path,m.title, l.id,l.path,l.title, f.name,f.path,f.kind,f.size FROM catalog_categories c LEFT JOIN catalog_courses co ON co.category_id=c.id LEFT JOIN catalog_modules m ON m.course_id=co.id LEFT JOIN catalog_lessons l ON l.module_id=m.id LEFT JOIN catalog_files f ON f.lesson_id=l.id ORDER BY c.path,co.path,m.path,l.path,f.path`)
	if err != nil {
		return result, err
	}
	categoryIndexes := map[int64]int{}
	courseIndexes := map[int64]struct{ category, course int }{}
	moduleIndexes := map[int64]struct{ category, course, module int }{}
	lessonIndexes := map[int64]struct{ category, course, module, lesson int }{}
	for rows.Next() {
		var categoryID, courseID, moduleID, lessonID sql.NullInt64
		var categoryPath, categoryTitle, coursePath, courseTitle, modulePath, moduleTitle, lessonPath, lessonTitle sql.NullString
		var fileName, filePath, fileKind sql.NullString
		var fileSize sql.NullInt64
		if err := rows.Scan(&categoryID, &categoryPath, &categoryTitle, &courseID, &coursePath, &courseTitle, &moduleID, &modulePath, &moduleTitle, &lessonID, &lessonPath, &lessonTitle, &fileName, &filePath, &fileKind, &fileSize); err != nil {
			rows.Close()
			return result, err
		}
		ci, ok := categoryIndexes[categoryID.Int64]
		if !ok {
			result.Categories = append(result.Categories, catalog.Category{Path: categoryPath.String, Title: categoryTitle.String})
			ci = len(result.Categories) - 1
			categoryIndexes[categoryID.Int64] = ci
		}
		if courseID.Valid {
			course, ok := courseIndexes[courseID.Int64]
			if !ok {
				result.Categories[ci].Courses = append(result.Categories[ci].Courses, catalog.Course{Path: coursePath.String, Title: courseTitle.String})
				course = struct{ category, course int }{ci, len(result.Categories[ci].Courses) - 1}
				courseIndexes[courseID.Int64] = course
			}
			if moduleID.Valid {
				module, ok := moduleIndexes[moduleID.Int64]
				if !ok {
					result.Categories[course.category].Courses[course.course].Modules = append(result.Categories[course.category].Courses[course.course].Modules, catalog.Module{Path: modulePath.String, Title: moduleTitle.String})
					module = struct{ category, course, module int }{course.category, course.course, len(result.Categories[course.category].Courses[course.course].Modules) - 1}
					moduleIndexes[moduleID.Int64] = module
				}
				if lessonID.Valid {
					lesson, ok := lessonIndexes[lessonID.Int64]
					if !ok {
						result.Categories[module.category].Courses[module.course].Modules[module.module].Lessons = append(result.Categories[module.category].Courses[module.course].Modules[module.module].Lessons, catalog.Lesson{Path: lessonPath.String, Title: lessonTitle.String})
						lesson = struct{ category, course, module, lesson int }{module.category, module.course, module.module, len(result.Categories[module.category].Courses[module.course].Modules[module.module].Lessons) - 1}
						lessonIndexes[lessonID.Int64] = lesson
					}
					if fileName.Valid {
						file := catalog.File{Name: fileName.String, Path: filePath.String, Size: fileSize.Int64}
						target := &result.Categories[lesson.category].Courses[lesson.course].Modules[lesson.module].Lessons[lesson.lesson]
						switch fileKind.String {
						case "media":
							target.Media = append(target.Media, file)
						case "subtitle":
							target.Subtitles = append(target.Subtitles, file)
						default:
							target.Materials = append(target.Materials, file)
						}
					}
				}
			}
		}
	}
	if err := rows.Err(); err != nil {
		rows.Close()
		return result, err
	}
	rows.Close()
	return result, nil
}

func hashPassword(password string) (string, error) {
	salt := make([]byte, 16)
	if _, err := rand.Read(salt); err != nil {
		return "", err
	}
	hash := argon2.IDKey([]byte(password), salt, 2, 64*1024, 1, 32)
	return fmt.Sprintf("$argon2id$v=19$m=65536,t=2,p=1$%s$%s", base64.RawStdEncoding.EncodeToString(salt), base64.RawStdEncoding.EncodeToString(hash)), nil
}

func verifyPassword(encoded, password string) bool {
	parts := strings.Split(encoded, "$")
	if len(parts) != 6 || parts[1] != "argon2id" {
		return false
	}
	salt, err1 := base64.RawStdEncoding.DecodeString(parts[4])
	expected, err2 := base64.RawStdEncoding.DecodeString(parts[5])
	if err1 != nil || err2 != nil || len(expected) == 0 {
		return false
	}
	actual := argon2.IDKey([]byte(password), salt, 2, 64*1024, 1, uint32(len(expected)))
	return subtleCompare(actual, expected)
}

func subtleCompare(a, b []byte) bool {
	return len(a) == len(b) && subtle.ConstantTimeCompare(a, b) == 1
}

func tokenHash(token string) string {
	sum := sha256.Sum256([]byte(token))
	return hex.EncodeToString(sum[:])
}
