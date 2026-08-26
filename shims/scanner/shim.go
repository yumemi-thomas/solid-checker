// Package scanner re-exports the slice of typescript-go's internal scanner
// package that this repository uses — nothing more. Declarations are
// copied from oxc-project/tsgolint's generated shims (MIT); the module
// path claims the typescript-go prefix so the internal imports resolve.
// Regenerate by hand when a compiler bump moves an identifier: the
// compiler reports alias breaks, and the go:linkname signatures below
// must be re-verified against the target revision by eye.
package scanner

import _ "github.com/microsoft/typescript-go/internal/scanner"
import _ "unsafe"

//go:linkname SkipTrivia github.com/microsoft/typescript-go/internal/scanner.SkipTrivia
func SkipTrivia(text string, pos int) int
