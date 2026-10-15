package catalog

import (
	"os"
	"path/filepath"
	"testing"
)

func TestScanBuildsCourseHierarchyAndGroupsLessonFiles(t *testing.T) {
	root := t.TempDir()
	writeFixture(t, filepath.Join(root, "01 - Programacao", "Go Essencial", "01 - Fundamentos", "01 - Variaveis", "aula.mp4"), "video")
	writeFixture(t, filepath.Join(root, "01 - Programacao", "Go Essencial", "01 - Fundamentos", "01 - Variaveis", "aula.srt"), "subtitle")
	writeFixture(t, filepath.Join(root, "01 - Programacao", "Go Essencial", "01 - Fundamentos", "01 - Variaveis", "apoio.pdf"), "pdf")
	writeFixture(t, filepath.Join(root, "01 - Programacao", "Go Essencial", "02 - Modulos", "02 - Pacotes", "parte-1.mp4"), "video")

	result, err := Scan(root)
	if err != nil {
		t.Fatal(err)
	}
	if len(result.Categories) != 1 || len(result.Categories[0].Courses) != 1 {
		t.Fatalf("unexpected hierarchy: %#v", result)
	}
	course := result.Categories[0].Courses[0]
	if course.Title != "Go Essencial" || len(course.Modules) != 2 {
		t.Fatalf("unexpected course: %#v", course)
	}
	lesson := course.Modules[0].Lessons[0]
	if len(lesson.Media) != 1 || len(lesson.Subtitles) != 1 || len(lesson.Materials) != 1 {
		t.Fatalf("lesson files were not classified: %#v", lesson)
	}
}

func TestScanIgnoresTemporaryDirectoriesAndUnsafeTypeScript(t *testing.T) {
	root := t.TempDir()
	writeFixture(t, filepath.Join(root, "Cat", "Curso", "Modulo", "Aula", "lesson.ts"), "const x = 1")
	writeFixture(t, filepath.Join(root, "Cat", "Curso", "Modulo", "Aula", "real.ts"), string(make([]byte, 2*1024*1024)))
	writeFixture(t, filepath.Join(root, "Cat", "Curso", "temp_download", "ignored.mp4"), "video")

	result, err := Scan(root)
	if err != nil {
		t.Fatal(err)
	}
	lesson := result.Categories[0].Courses[0].Modules[0].Lessons[0]
	if len(lesson.Media) != 1 || filepath.Base(lesson.Media[0].Path) != "real.ts" {
		t.Fatalf("unexpected media classification: %#v", lesson.Media)
	}
}

func TestScanPreservesFilesAtLibraryRoot(t *testing.T) {
	root := t.TempDir()
	writeFixture(t, filepath.Join(root, "intro.mp4"), "video")
	result, err := Scan(root)
	if err != nil { t.Fatal(err) }
	if len(result.LooseFiles) != 1 || result.LooseFiles[0].Path != "intro.mp4" { t.Fatalf("root files = %#v", result.LooseFiles) }
}

func writeFixture(t *testing.T, path, contents string) {
	t.Helper()
	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(path, []byte(contents), 0o644); err != nil {
		t.Fatal(err)
	}
}
