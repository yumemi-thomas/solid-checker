// Package core re-exports the slice of typescript-go's internal core
// package that this repository uses — nothing more. Declarations are
// copied from oxc-project/tsgolint's generated shims (MIT); the module
// path claims the typescript-go prefix so the internal imports resolve.
// Regenerate by hand when a compiler bump moves an identifier: the
// compiler reports alias breaks, and the go:linkname signatures below
// must be re-verified against the target revision by eye.
package core

import core "github.com/microsoft/typescript-go/internal/core"

type CompilerOptions = core.CompilerOptions

const TSFalse = core.TSFalse
