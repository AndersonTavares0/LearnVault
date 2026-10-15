package library

import (
	"errors"
	"io"
	"os"
	"path/filepath"
	"strings"
)

var ErrReadOnly = errors.New("library is read-only")

type Root struct{ path string }

func (r *Root) Path() string { return r.path }

// Open requires the mount itself to be non-writable, in addition to the
// container's read-only bind mount. This is a startup guard, not a substitute
// for filesystem/container permissions. Operationally, the library root and
// contents must be immutable to untrusted actors while the process runs; this
// is not a complete cross-platform guarantee against concurrent replacement.
func Open(path string) (*Root, error) {
	abs, err := filepath.Abs(path)
	if err != nil {
		return nil, err
	}
	info, err := os.Stat(abs)
	if err != nil {
		return nil, err
	}
	if !info.IsDir() {
		return nil, errors.New("library path is not a directory")
	}
	return &Root{path: abs}, nil
}

func (r *Root) ReadFile(relative string) ([]byte, error) {
	file, err := r.OpenFile(relative)
	if err != nil {
		return nil, err
	}
	defer file.Close()
	return io.ReadAll(file)
}

func (r *Root) OpenFile(relative string) (*os.File, error) {
	path, err := r.resolve(relative)
	if err != nil {
		return nil, err
	}
	file, err := os.Open(path)
	if err != nil {
		return nil, err
	}
	if mismatch, err := identityMismatch(path, file); err != nil {
		file.Close()
		return nil, err
	} else if mismatch {
		file.Close()
		return nil, errors.New("library file changed during open")
	}
	return file, nil
}

func identityMismatch(path string, file *os.File) (bool, error) {
	pathInfo, err := os.Stat(path)
	if err != nil {
		return false, err
	}
	openInfo, err := file.Stat()
	if err != nil {
		return false, err
	}
	if !pathInfo.Mode().IsRegular() || !openInfo.Mode().IsRegular() {
		return true, nil
	}
	// SameFile uses the platform's available file identity (inode/device on
	// Unix and file index/volume on Windows) without requiring native callers.
	return !os.SameFile(pathInfo, openInfo), nil
}

func (r *Root) WriteFile(string, []byte) error { return ErrReadOnly }

func (r *Root) resolve(relative string) (string, error) {
	if filepath.IsAbs(relative) {
		return "", errors.New("absolute paths are not allowed")
	}
	clean := filepath.Clean(relative)
	if clean == "." || clean == ".." || strings.HasPrefix(clean, ".."+string(filepath.Separator)) {
		return "", errors.New("path escapes library root")
	}
	path := filepath.Join(r.path, clean)
	resolved, err := filepath.EvalSymlinks(path)
	if err != nil {
		return "", err
	}
	if resolved != r.path && !strings.HasPrefix(resolved, r.path+string(filepath.Separator)) {
		return "", errors.New("symlink escapes library root")
	}
	return resolved, nil
}
