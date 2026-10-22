package tsgo

import (
	"io/fs"
	"path/filepath"
	"sort"
	"strings"
	"sync"
	"time"

	"github.com/microsoft/TypeScript/tsc/shim/vfs"
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

func (o *overlayFS) clone() *overlayFS {
	o.mu.RLock()
	defer o.mu.RUnlock()
	cloned := newOverlayFS(o.base)
	for path, source := range o.files {
		cloned.files[path] = source
	}
	for path := range o.deleted {
		cloned.deleted[path] = struct{}{}
	}
	return cloned
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
func (o *overlayFS) DirectoryExists(path string) bool {
	if o.base.DirectoryExists(path) {
		return true
	}
	o.mu.RLock()
	defer o.mu.RUnlock()
	directory := cleanOverlayPath(path)
	for file := range o.files {
		if isDescendant(file, directory) {
			return true
		}
	}
	return false
}

func (o *overlayFS) GetAccessibleEntries(path string) vfs.Entries {
	base := o.base.GetAccessibleEntries(path)
	files := make(map[string]struct{}, len(base.Files))
	directories := make(map[string]struct{}, len(base.Directories))
	for _, name := range base.Files {
		files[name] = struct{}{}
	}
	for _, name := range base.Directories {
		directories[name] = struct{}{}
	}

	o.mu.RLock()
	defer o.mu.RUnlock()
	directory := cleanOverlayPath(path)
	for file := range o.deleted {
		if filepath.Dir(file) == directory {
			delete(files, filepath.Base(file))
		}
	}
	for file := range o.files {
		relative, err := filepath.Rel(directory, file)
		if err != nil || relative == "." || relative == ".." || strings.HasPrefix(relative, ".."+string(filepath.Separator)) {
			continue
		}
		parts := strings.Split(relative, string(filepath.Separator))
		if len(parts) == 1 {
			files[parts[0]] = struct{}{}
		} else {
			directories[parts[0]] = struct{}{}
		}
	}

	base.Files = sortedEntryNames(files)
	base.Directories = sortedEntryNames(directories)
	if base.Symlinks != nil {
		for name := range base.Symlinks {
			if _, file := files[name]; !file {
				if _, directory := directories[name]; !directory {
					delete(base.Symlinks, name)
				}
			}
		}
	}
	return base
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

func isDescendant(path, directory string) bool {
	relative, err := filepath.Rel(directory, path)
	return err == nil && relative != "." && relative != ".." && !strings.HasPrefix(relative, ".."+string(filepath.Separator))
}

func sortedEntryNames(entries map[string]struct{}) []string {
	names := make([]string, 0, len(entries))
	for name := range entries {
		names = append(names, name)
	}
	sort.Strings(names)
	return names
}

type overlayFileInfo struct {
	name      string
	size      int64
	directory bool
}

func (i overlayFileInfo) Name() string { return i.name }
func (i overlayFileInfo) Size() int64  { return i.size }
func (i overlayFileInfo) Mode() fs.FileMode {
	if i.directory {
		return fs.ModeDir
	}
	return 0
}
func (i overlayFileInfo) ModTime() time.Time { return time.Time{} }
func (i overlayFileInfo) IsDir() bool        { return i.directory }
func (i overlayFileInfo) Sys() any           { return nil }
