// Package tsoptions re-exports the slice of typescript-go's internal tsoptions
// package that this repository uses — nothing more. Declarations are
// copied from oxc-project/tsgolint's generated shims (MIT); the module
// path claims the typescript-go prefix so the internal imports resolve.
// Regenerate by hand when a compiler bump moves an identifier: the
// compiler reports alias breaks, and the go:linkname signatures below
// must be re-verified against the target revision by eye.
package tsoptions

import ast "github.com/microsoft/typescript-go/internal/ast"
import collections "github.com/microsoft/typescript-go/internal/collections"
import core "github.com/microsoft/typescript-go/internal/core"
import tsoptions "github.com/microsoft/typescript-go/internal/tsoptions"
import _ "unsafe"

//go:linkname GetParsedCommandLineOfConfigFile github.com/microsoft/typescript-go/internal/tsoptions.GetParsedCommandLineOfConfigFile
func GetParsedCommandLineOfConfigFile(configFileName string, options *core.CompilerOptions, optionsRaw *collections.OrderedMap[string, any], sys tsoptions.ParseConfigHost, extendedConfigCache tsoptions.ExtendedConfigCache) (*tsoptions.ParsedCommandLine, []*ast.Diagnostic)
