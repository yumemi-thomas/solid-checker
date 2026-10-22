// Package osvfs re-exports the slice of TypeScript's tsc internal osvfs
// package that this repository uses — nothing more. Declarations are
// copied from oxc-project/tsgolint's generated shims (MIT); the module
// path claims the TypeScript/tsc prefix so the internal imports resolve.
// Regenerate by hand when a compiler bump moves an identifier: the
// compiler reports alias breaks, and the go:linkname signatures below
// must be re-verified against the target revision by eye.
package osvfs

import _ "github.com/microsoft/TypeScript/tsc/internal/vfs/osvfs"
import vfs "github.com/microsoft/TypeScript/tsc/internal/vfs"
import _ "unsafe"

//go:linkname FS github.com/microsoft/TypeScript/tsc/internal/vfs/osvfs.FS
func FS() vfs.FS
