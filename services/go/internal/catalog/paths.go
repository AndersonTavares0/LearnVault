package catalog

import (
	"path/filepath"
	"strings"
)

// NormalizePath canonicalizes a library-relative path and reports whether it is
// acceptable. Absolute paths and any traversal segment are rejected; the accepted
// form is cleaned and slash-separated so the same physical file yields one identity
// on every platform.
//
// This is the single normalization used by the media authorization boundary and by
// the removal tombstone, so a removed path always matches the catalog path it hides.
func NormalizePath(value string) (string, bool) {
	if value == "" || filepath.IsAbs(value) {
		return "", false
	}
	for _, part := range strings.FieldsFunc(value, func(r rune) bool { return r == '/' || r == '\\' }) {
		if part == ".." {
			return "", false
		}
	}
	clean := filepath.Clean(value)
	if clean == "." || clean == ".." || strings.HasPrefix(clean, ".."+string(filepath.Separator)) {
		return "", false
	}
	return filepath.ToSlash(clean), true
}
