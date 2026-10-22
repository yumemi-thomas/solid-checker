module github.com/yumemi-thomas/solid-checker

go 1.27

require (
	github.com/fxamacker/cbor/v2 v2.9.0
	github.com/microsoft/TypeScript/tsc/shim/ast v0.0.0
	github.com/microsoft/TypeScript/tsc/shim/bundled v0.0.0
	github.com/microsoft/TypeScript/tsc/shim/checker v0.0.0
	github.com/microsoft/TypeScript/tsc/shim/compiler v0.0.0
	github.com/microsoft/TypeScript/tsc/shim/core v0.0.0
	github.com/microsoft/TypeScript/tsc/shim/scanner v0.0.0
	github.com/microsoft/TypeScript/tsc/shim/tsoptions v0.0.0
	github.com/microsoft/TypeScript/tsc/shim/vfs v0.0.0
	github.com/microsoft/TypeScript/tsc/shim/vfs/osvfs v0.0.0
	golang.org/x/sys v0.48.0
)

require (
	github.com/Microsoft/go-winio v0.6.2 // indirect
	github.com/klauspost/compress v1.20.0 // indirect
	github.com/klauspost/cpuid/v2 v2.2.10 // indirect
	github.com/microsoft/TypeScript/tsc v0.0.0-20260929230140-b85298b6a81f // indirect
	github.com/x448/float16 v0.8.4 // indirect
	github.com/zeebo/xxh3 v1.1.0 // indirect
	golang.org/x/sync v0.23.0 // indirect
	golang.org/x/text v0.42.0 // indirect
)

// The shims live in this repository (./shims): each module claims the
// github.com/microsoft/TypeScript/tsc path prefix so it may alias and go:linkname that compiler's
// internal packages, scoped to the identifiers this repository actually uses.
// All nine must pin ONE TypeScript/tsc revision (pins_test.go enforces it),
// and every compiler bump re-verifies the go:linkname signatures by eye.
replace (
	github.com/microsoft/TypeScript/tsc/shim/ast => ./shims/ast
	github.com/microsoft/TypeScript/tsc/shim/bundled => ./shims/bundled
	github.com/microsoft/TypeScript/tsc/shim/checker => ./shims/checker
	github.com/microsoft/TypeScript/tsc/shim/compiler => ./shims/compiler
	github.com/microsoft/TypeScript/tsc/shim/core => ./shims/core
	github.com/microsoft/TypeScript/tsc/shim/scanner => ./shims/scanner
	github.com/microsoft/TypeScript/tsc/shim/tsoptions => ./shims/tsoptions
	github.com/microsoft/TypeScript/tsc/shim/vfs => ./shims/vfs
	github.com/microsoft/TypeScript/tsc/shim/vfs/osvfs => ./shims/vfs/osvfs
)
