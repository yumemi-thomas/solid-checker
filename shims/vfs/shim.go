// Package vfs re-exports the slice of typescript-go's internal vfs
// package that this repository uses — nothing more. Declarations are
// copied from oxc-project/tsgolint's generated shims (MIT); the module
// path claims the typescript-go prefix so the internal imports resolve.
// Regenerate by hand when a compiler bump moves an identifier: the
// compiler reports alias breaks, and the go:linkname signatures below
// must be re-verified against the target revision by eye.
package vfs

import vfs "github.com/microsoft/typescript-go/internal/vfs"

type Entries = vfs.Entries
type FS = vfs.FS
type FileInfo = vfs.FileInfo
type WalkDirFunc = vfs.WalkDirFunc
