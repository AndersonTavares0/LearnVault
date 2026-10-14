package library

import (
	"errors"
	"os"
	"path/filepath"
	"strings"
)

var ErrReadOnly = errors.New("library is read-only")

type Root struct{ path string }

func (r *Root) Path() string { return r.path }

// Open requires the mount itself to be non-writable, in addition to the
// container's read-only bind mount. This is a startup guard, not a substitute
// for filesystem/container permissions.
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
	path, err := r.resolve(relative)
	if err != nil {
		return nil, err
	}
	return os.ReadFile(path)
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
	return file, nil
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
