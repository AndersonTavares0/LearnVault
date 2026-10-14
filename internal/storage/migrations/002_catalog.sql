CREATE TABLE IF NOT EXISTS catalog_categories (
    id INTEGER PRIMARY KEY,
    path TEXT NOT NULL UNIQUE,
    title TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS catalog_courses (
    id INTEGER PRIMARY KEY,
    category_id INTEGER NOT NULL REFERENCES catalog_categories(id) ON DELETE CASCADE,
    path TEXT NOT NULL UNIQUE,
    title TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS catalog_modules (
    id INTEGER PRIMARY KEY,
    course_id INTEGER NOT NULL REFERENCES catalog_courses(id) ON DELETE CASCADE,
    path TEXT NOT NULL UNIQUE,
    title TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS catalog_lessons (
    id INTEGER PRIMARY KEY,
    module_id INTEGER NOT NULL REFERENCES catalog_modules(id) ON DELETE CASCADE,
    path TEXT NOT NULL UNIQUE,
    title TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS catalog_files (
    id INTEGER PRIMARY KEY,
    lesson_id INTEGER NOT NULL REFERENCES catalog_lessons(id) ON DELETE CASCADE,
    path TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN ('media','subtitle','material')),
    size INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS catalog_courses_category_idx ON catalog_courses(category_id);
CREATE INDEX IF NOT EXISTS catalog_modules_course_idx ON catalog_modules(course_id);
CREATE INDEX IF NOT EXISTS catalog_lessons_module_idx ON catalog_lessons(module_id);
INSERT OR IGNORE INTO schema_migrations(version) VALUES (2);
