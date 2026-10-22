// Package compiler re-exports the slice of TypeScript's tsc internal compiler
// package that this repository uses — nothing more. Declarations are
// copied from oxc-project/tsgolint's generated shims (MIT); the module
// path claims the TypeScript/tsc prefix so the internal imports resolve.
// Regenerate by hand when a compiler bump moves an identifier: the
// compiler reports alias breaks, and the go:linkname signatures below
// must be re-verified against the target revision by eye.
package compiler

import compiler "github.com/microsoft/TypeScript/tsc/internal/compiler"
import contentmapper "github.com/microsoft/TypeScript/tsc/internal/contentmapper"
import diagnostics "github.com/microsoft/TypeScript/tsc/internal/diagnostics"
import tsoptions "github.com/microsoft/TypeScript/tsc/internal/tsoptions"
import vfs "github.com/microsoft/TypeScript/tsc/internal/vfs"
import _ "unsafe"

type CheckerPool = compiler.CheckerPool
type CompilerHost = compiler.CompilerHost

const EmitOnlyBuilderSignature = compiler.EmitOnlyBuilderSignature

type EmitOptions = compiler.EmitOptions

//go:linkname NewCompilerHost github.com/microsoft/TypeScript/tsc/internal/compiler.NewCompilerHost
func NewCompilerHost(currentDirectory string, fs vfs.FS, defaultLibraryPath string, extendedConfigCache tsoptions.ExtendedConfigCache, trace func(msg *diagnostics.Message, args ...any), contentMapperProject contentmapper.Project) compiler.CompilerHost

//go:linkname NewProgram github.com/microsoft/TypeScript/tsc/internal/compiler.NewProgram
func NewProgram(opts compiler.ProgramOptions) *compiler.Program

type Program = compiler.Program
type ProgramOptions = compiler.ProgramOptions
type WriteFileData = compiler.WriteFileData
