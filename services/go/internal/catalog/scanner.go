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
	LooseFiles []File     `json:"loose_files,omitempty"`
	Nodes      []Node     `json:"nodes,omitempty"`
}
type Node struct {
	Title      string `json:"title"`
	Path       string `json:"path"`
	Role       string `json:"role"`
	Files      []File `json:"files,omitempty"`
	LooseFiles []File `json:"loose_files,omitempty"`
	Children   []Node `json:"children,omitempty"`
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
	Kind string `json:"kind,omitempty"`
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
	if err := validateReadableTree(root); err != nil {
		return Catalog{}, err
	}
	entries, err := os.ReadDir(root)
	if err != nil {
		return Catalog{}, err
	}
	var result Catalog
	for _, categoryEntry := range entries {
		if !categoryEntry.IsDir() && !ignoredFile(categoryEntry.Name()) {
			if file, ok := scanFile(root, root, categoryEntry); ok {
				result.LooseFiles = append(result.LooseFiles, file)
			}
			continue
		}
		if !categoryEntry.IsDir() || ignoredDir(categoryEntry.Name()) {
			continue
		}
		categoryPath := filepath.Join(root, categoryEntry.Name())
		node, err := scanNode(root, categoryPath, "category")
		if err != nil {
			return Catalog{}, err
		}
		result.Nodes = append(result.Nodes, node)
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

// scanNode retains the physical tree, including shallow lessons and files. Roles are
// descriptive only; identity is always the complete relative path.
func scanNode(root, path, role string) (Node, error) {
	node := Node{Title: cleanTitle(filepath.Base(path)), Path: relPath(root, path), Role: role}
	entries, err := os.ReadDir(path)
	if err != nil {
		return Node{}, err
	}
	for _, entry := range entries {
		if ignoredFile(entry.Name()) {
			continue
		}
		childPath := filepath.Join(path, entry.Name())
		if entry.IsDir() {
			if ignoredDir(entry.Name()) {
				continue
			}
			child, err := scanNode(root, childPath, "group")
			if err != nil {
				return Node{}, err
			}
			if len(child.Files) > 0 {
				child.Role = "lesson"
			}
			node.Children = append(node.Children, child)
			continue
		}
		file, ok := scanFile(root, path, entry)
		if !ok {
			return Node{}, errors.New("failed to read catalog file metadata")
		}
		file.Kind = fileKind(entry.Name(), file.Size)
		if role == "category" {
			node.LooseFiles = append(node.LooseFiles, file)
		} else {
			node.Files = append(node.Files, file)
		}
	}
	if len(node.Files) > 0 && role != "category" {
		node.Role = "lesson"
	}
	sort.Slice(node.Children, func(i, j int) bool { return node.Children[i].Path < node.Children[j].Path })
	sort.Slice(node.Files, func(i, j int) bool { return node.Files[i].Path < node.Files[j].Path })
	return node, nil
}

func fileKind(name string, size int64) string {
	ext := strings.ToLower(filepath.Ext(name))
	if subtitleExt[ext] {
		return "subtitle"
	}
	if IsMediaName(name, size) {
		return "media"
	}
	return "material"
}

// IsMediaExtension reports whether a name has a playable media extension, ignoring the
// `.ts` size rule (which needs file metadata). It is the extension-only half of the
// scanner's media classification, used where a size is unavailable.
func IsMediaExtension(name string) bool {
	return mediaExt[strings.ToLower(filepath.Ext(name))]
}

// IsMediaName reports whether a file name is playable media — not a subtitle and not a
// material such as a PDF — using the same extension and `.ts` size rule as the scanner.
// The study boundary uses it so progress is only attached to playable media.
func IsMediaName(name string, size int64) bool {
	if !IsMediaExtension(name) {
		return false
	}
	return !strings.EqualFold(filepath.Ext(name), ".ts") || size >= 1024*1024
}

// IsUnambiguousMediaExtension reports whether a name's extension identifies playable media
// without needing the `.ts` size rule. `.ts` is deliberately excluded: the scanner treats a
// segment-sized `.ts` as material, so without size metadata (for example a removal
// tombstone that no longer carries the catalog file) the classification is ambiguous and
// must not be resolved in favor of media.
func IsUnambiguousMediaExtension(name string) bool {
	return IsMediaExtension(name) && !strings.EqualFold(filepath.Ext(name), ".ts")
}

// LookupFile returns the persisted catalog file whose canonical relative path equals
// clean, which must already be canonical (as produced by NormalizePath). It walks the
// loose files and the full node tree at any depth, so a file loose in a category or nested
// under a lesson is found. The scanner's file name and size travel with the result, which
// is what the study boundary needs to classify playable media.
func LookupFile(value Catalog, clean string) (File, bool) {
	if file, ok := lookupFile(value.LooseFiles, clean); ok {
		return file, true
	}
	return lookupFileInNodes(value.Nodes, clean)
}

func lookupFile(files []File, clean string) (File, bool) {
	for _, file := range files {
		if path, ok := NormalizePath(file.Path); ok && path == clean {
			return file, true
		}
	}
	return File{}, false
}

func lookupFileInNodes(nodes []Node, clean string) (File, bool) {
	for _, node := range nodes {
		if file, ok := lookupFile(node.Files, clean); ok {
			return file, true
		}
		if file, ok := lookupFile(node.LooseFiles, clean); ok {
			return file, true
		}
		if file, ok := lookupFileInNodes(node.Children, clean); ok {
			return file, true
		}
	}
	return File{}, false
}

// validateReadableTree is deliberately performed before building any result. A directory
// walk that loses a subtree must never look like an empty subtree and replace a valid catalog.
// The HTTP rescan persists only the value returned by Scan, so this is the fail-closed boundary.
func validateReadableTree(root string) error {
	return filepath.WalkDir(root, func(path string, entry fs.DirEntry, err error) error {
		if err != nil {
			return err
		}
		if path != root && entry.IsDir() && ignoredDir(entry.Name()) {
			return filepath.SkipDir
		}
		if entry.IsDir() || entry.Type()&os.ModeSymlink != 0 {
			return nil
		}
		if _, err := entry.Info(); err != nil {
			return err
		}
		return nil
	})
}

func scanFile(root, dir string, entry fs.DirEntry) (File, bool) {
	info, err := entry.Info()
	if err != nil || !info.Mode().IsRegular() {
		return File{}, false
	}
	return File{Name: entry.Name(), Path: relPath(root, filepath.Join(dir, entry.Name())), Size: info.Size()}, true
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
