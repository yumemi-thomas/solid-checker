package tsgo

import (
	"io/fs"
	"path/filepath"
	"strings"
	"sync"
	"time"

	"github.com/microsoft/typescript-go/shim/vfs"
)

type overlayFS struct {
	base    vfs.FS
	mu      sync.RWMutex
	files   map[string]string
	deleted map[string]struct{}
}

var _ vfs.FS = (*overlayFS)(nil)

func newOverlayFS(base vfs.FS) *overlayFS {
	return &overlayFS{
		base:    base,
		files:   make(map[string]string),
		deleted: make(map[string]struct{}),
	}
}

func (o *overlayFS) set(path, source string) {
	o.mu.Lock()
	defer o.mu.Unlock()
	path = cleanOverlayPath(path)
	o.files[path] = source
	delete(o.deleted, path)
}

func (o *overlayFS) delete(path string) {
	o.mu.Lock()
	defer o.mu.Unlock()
	path = cleanOverlayPath(path)
	delete(o.files, path)
	o.deleted[path] = struct{}{}
}

func (o *overlayFS) UseCaseSensitiveFileNames() bool { return o.base.UseCaseSensitiveFileNames() }

func (o *overlayFS) FileExists(path string) bool {
	o.mu.RLock()
	defer o.mu.RUnlock()
	path = cleanOverlayPath(path)
	if _, deleted := o.deleted[path]; deleted {
		return false
	}
	if _, ok := o.files[path]; ok {
		return true
	}
	return o.base.FileExists(path)
}

func (o *overlayFS) ReadFile(path string) (string, bool) {
	o.mu.RLock()
	defer o.mu.RUnlock()
	path = cleanOverlayPath(path)
	if _, deleted := o.deleted[path]; deleted {
		return "", false
	}
	if source, ok := o.files[path]; ok {
		return source, true
	}
	return o.base.ReadFile(path)
}

func (o *overlayFS) WriteFile(path, data string) error           { return o.base.WriteFile(path, data) }
func (o *overlayFS) AppendFile(path, data string) error          { return o.base.AppendFile(path, data) }
func (o *overlayFS) Remove(path string) error                    { return o.base.Remove(path) }
func (o *overlayFS) Chtimes(path string, at, mt time.Time) error { return o.base.Chtimes(path, at, mt) }
func (o *overlayFS) DirectoryExists(path string) bool            { return o.base.DirectoryExists(path) }
func (o *overlayFS) GetAccessibleEntries(path string) vfs.Entries {
	return o.base.GetAccessibleEntries(path)
}

func (o *overlayFS) Stat(path string) vfs.FileInfo {
	o.mu.RLock()
	defer o.mu.RUnlock()
	path = cleanOverlayPath(path)
	if _, deleted := o.deleted[path]; deleted {
		return nil
	}
	if source, ok := o.files[path]; ok {
		return overlayFileInfo{name: filepath.Base(path), size: int64(len(source))}
	}
	return o.base.Stat(path)
}

func (o *overlayFS) WalkDir(root string, walkFn vfs.WalkDirFunc) error {
	return o.base.WalkDir(root, walkFn)
}

func (o *overlayFS) Realpath(path string) string {
	o.mu.RLock()
	defer o.mu.RUnlock()
	path = cleanOverlayPath(path)
	if _, ok := o.files[path]; ok {
		return path
	}
	return o.base.Realpath(path)
}

func cleanOverlayPath(path string) string {
	if strings.Contains(path, "://") {
		return path
	}
	return filepath.Clean(path)
}

type overlayFileInfo struct {
	name string
	size int64
}

func (i overlayFileInfo) Name() string       { return i.name }
func (i overlayFileInfo) Size() int64        { return i.size }
func (i overlayFileInfo) Mode() fs.FileMode  { return 0 }
func (i overlayFileInfo) ModTime() time.Time { return time.Time{} }
func (i overlayFileInfo) IsDir() bool        { return false }
func (i overlayFileInfo) Sys() any           { return nil }
