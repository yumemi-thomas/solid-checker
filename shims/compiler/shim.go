// Package compiler re-exports the slice of typescript-go's internal compiler
// package that this repository uses — nothing more. Declarations are
// copied from oxc-project/tsgolint's generated shims (MIT); the module
// path claims the typescript-go prefix so the internal imports resolve.
// Regenerate by hand when a compiler bump moves an identifier: the
// compiler reports alias breaks, and the go:linkname signatures below
// must be re-verified against the target revision by eye.
package compiler

import compiler "github.com/microsoft/typescript-go/internal/compiler"
import diagnostics "github.com/microsoft/typescript-go/internal/diagnostics"
import tsoptions "github.com/microsoft/typescript-go/internal/tsoptions"
import vfs "github.com/microsoft/typescript-go/internal/vfs"
import _ "unsafe"

type CheckerPool = compiler.CheckerPool

const EmitOnlyForcedDts = compiler.EmitOnlyForcedDts

type EmitOptions = compiler.EmitOptions

//go:linkname NewCompilerHost github.com/microsoft/typescript-go/internal/compiler.NewCompilerHost
func NewCompilerHost(currentDirectory string, fs vfs.FS, defaultLibraryPath string, extendedConfigCache tsoptions.ExtendedConfigCache, trace func(msg *diagnostics.Message, args ...any)) compiler.CompilerHost

//go:linkname NewProgram github.com/microsoft/typescript-go/internal/compiler.NewProgram
func NewProgram(opts compiler.ProgramOptions) *compiler.Program

type Program = compiler.Program
type ProgramOptions = compiler.ProgramOptions
type WriteFileData = compiler.WriteFileData
