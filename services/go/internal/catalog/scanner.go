package catalog

import (
	"errors"
	"io/fs"
	"os"
	"path/filepath"
	"sort"
	"strings"
)

type Catalog struct {
	Categories []Category `json:"categories"`
}
type Category struct {
	Title   string   `json:"title"`
	Path    string   `json:"path"`
	Courses []Course `json:"courses"`
}
type Course struct {
	Title   string   `json:"title"`
	Path    string   `json:"path"`
	Modules []Module `json:"modules"`
}
type Module struct {
	Title   string   `json:"title"`
	Path    string   `json:"path"`
	Lessons []Lesson `json:"lessons"`
}
type Lesson struct {
	Title     string `json:"title"`
	Path      string `json:"path"`
	Media     []File `json:"media"`
	Subtitles []File `json:"subtitles"`
	Materials []File `json:"materials"`
}
type File struct {
	Name string `json:"name"`
	Path string `json:"path"`
	Size int64  `json:"size"`
}

var mediaExt = map[string]bool{".mp4": true, ".webm": true, ".mkv": true, ".m3u8": true, ".ts": true}
var subtitleExt = map[string]bool{".srt": true, ".vtt": true, ".ass": true, ".ssa": true}

// Scan only reads directory entries and file metadata. It never writes to root.
// The hierarchy deliberately follows the stable physical pattern category/course/module/lesson;
// directories containing temporary downloader artifacts are excluded.
func Scan(root string) (Catalog, error) {
	info, err := os.Stat(root)
	if err != nil {
		return Catalog{}, err
	}
	if !info.IsDir() {
		return Catalog{}, errors.New("catalog root is not a directory")
	}
	entries, err := os.ReadDir(root)
	if err != nil {
		return Catalog{}, err
	}
	var result Catalog
	for _, categoryEntry := range entries {
		if !categoryEntry.IsDir() || ignoredDir(categoryEntry.Name()) {
			continue
		}
		categoryPath := filepath.Join(root, categoryEntry.Name())
		category := Category{Title: cleanTitle(categoryEntry.Name()), Path: relPath(root, categoryPath)}
		courseRoots := findCourseRoots(categoryPath)
		for _, coursePath := range courseRoots {
			course := buildCourse(root, coursePath)
			if len(course.Modules) > 0 {
				category.Courses = append(category.Courses, course)
			}
		}
		if len(category.Courses) > 0 {
			result.Categories = append(result.Categories, category)
		}
	}
	sort.Slice(result.Categories, func(i, j int) bool { return result.Categories[i].Path < result.Categories[j].Path })
	return result, nil
}

func findCourseRoots(category string) []string {
	var modules []string
	_ = filepath.WalkDir(category, func(path string, entry fs.DirEntry, err error) error {
		if err != nil {
			return err
		}
		if path != category && entry.IsDir() && ignoredDir(entry.Name()) {
			return filepath.SkipDir
		}
		if path == category || !entry.IsDir() {
			return nil
		}
		if hasLessonChildren(path) {
			modules = append(modules, path)
		}
		return nil
	})
	seen := map[string]bool{}
	var courses []string
	for _, module := range modules {
		course := filepath.Dir(module)
		if !seen[course] {
			seen[course] = true
			courses = append(courses, course)
		}
	}
	sort.Strings(courses)
	return courses
}

func buildCourse(root, path string) Course {
	course := Course{Title: cleanTitle(filepath.Base(path)), Path: relPath(root, path)}
	entries, _ := os.ReadDir(path)
	for _, entry := range entries {
		if !entry.IsDir() || ignoredDir(entry.Name()) {
			continue
		}
		module := buildModule(root, filepath.Join(path, entry.Name()))
		if len(module.Lessons) > 0 {
			course.Modules = append(course.Modules, module)
		}
	}
	return course
}

func buildModule(root, path string) Module {
	module := Module{Title: cleanTitle(filepath.Base(path)), Path: relPath(root, path)}
	entries, _ := os.ReadDir(path)
	for _, entry := range entries {
		if !entry.IsDir() || ignoredDir(entry.Name()) || !hasMediaFiles(filepath.Join(path, entry.Name())) {
			continue
		}
		lesson := buildLesson(root, filepath.Join(path, entry.Name()))
		module.Lessons = append(module.Lessons, lesson)
	}
	sort.Slice(module.Lessons, func(i, j int) bool { return module.Lessons[i].Path < module.Lessons[j].Path })
	return module
}

func buildLesson(root, path string) Lesson {
	lesson := Lesson{Title: cleanTitle(filepath.Base(path)), Path: relPath(root, path)}
	entries, _ := os.ReadDir(path)
	for _, entry := range entries {
		if entry.IsDir() || ignoredFile(entry.Name()) {
			continue
		}
		info, err := entry.Info()
		if err != nil {
			continue
		}
		file := File{Name: entry.Name(), Path: relPath(root, filepath.Join(path, entry.Name())), Size: info.Size()}
		ext := strings.ToLower(filepath.Ext(entry.Name()))
		switch {
		case mediaExt[ext] && (ext != ".ts" || info.Size() >= 1024*1024):
			lesson.Media = append(lesson.Media, file)
		case subtitleExt[ext]:
			lesson.Subtitles = append(lesson.Subtitles, file)
		default:
			lesson.Materials = append(lesson.Materials, file)
		}
	}
	return lesson
}

func hasLessonChildren(path string) bool {
	entries, err := os.ReadDir(path)
	if err != nil {
		return false
	}
	for _, entry := range entries {
		if entry.IsDir() && !ignoredDir(entry.Name()) && hasMediaFiles(filepath.Join(path, entry.Name())) {
			return true
		}
	}
	return false
}
func hasMediaFiles(path string) bool {
	entries, err := os.ReadDir(path)
	if err != nil {
		return false
	}
	for _, entry := range entries {
		if entry.IsDir() || ignoredFile(entry.Name()) {
			continue
		}
		info, err := entry.Info()
		if err != nil {
			continue
		}
		ext := strings.ToLower(filepath.Ext(entry.Name()))
		if mediaExt[ext] && (ext != ".ts" || info.Size() >= 1024*1024) {
			return true
		}
	}
	return false
}
func ignoredDir(name string) bool {
	n := strings.ToLower(name)
	return strings.HasPrefix(n, "temp_") || n == "temp" || n == "0____"
}
func ignoredFile(name string) bool { return strings.HasPrefix(strings.ToLower(name), ".") }
func relPath(root, path string) string {
	value, _ := filepath.Rel(root, path)
	return filepath.ToSlash(value)
}
func cleanTitle(name string) string {
	return strings.TrimSpace(strings.TrimLeft(name, "0123456789._- •\t"))
}
