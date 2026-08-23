// Package core re-exports the slice of typescript-go's internal core
// package that this repository uses — nothing more. Declarations are
// copied from oxc-project/tsgolint's generated shims (MIT); the module
// path claims the typescript-go prefix so the internal imports resolve.
// Regenerate by hand when a compiler bump moves an identifier: the
// compiler reports alias breaks, and the go:linkname signatures below
// must be re-verified against the target revision by eye.
package core

import core "github.com/microsoft/typescript-go/internal/core"
import _ "unsafe"

type CompilerOptions = core.CompilerOptions

const TSFalse = core.TSFalse

// ModuleKind and the two constants below are needed to name the emit module
// format Program.GetEmitModuleFormatOfFile returns. ModuleKind's own
// IsNonNodeESM predicate covers the contiguous ES module kinds, so no ES
// constant is aliased.
type ModuleKind = core.ModuleKind

const ModuleKindCommonJS = core.ModuleKindCommonJS
const ModuleKindPreserve = core.ModuleKindPreserve

// Pattern is the compiler's own tsconfig `paths` key matcher, including its
// exact/star classification and its Matches predicate.
type Pattern = core.Pattern

//go:linkname TryParsePattern github.com/microsoft/typescript-go/internal/core.TryParsePattern
func TryParsePattern(pattern string) core.Pattern
