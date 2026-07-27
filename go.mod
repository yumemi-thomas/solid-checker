module github.com/yumemi-thomas/solid-ts-facts

go 1.26

require (
	github.com/fxamacker/cbor/v2 v2.9.0
	github.com/microsoft/typescript-go/shim/ast v0.0.0
	github.com/microsoft/typescript-go/shim/bundled v0.0.0
	github.com/microsoft/typescript-go/shim/checker v0.0.0
	github.com/microsoft/typescript-go/shim/compiler v0.0.0
	github.com/microsoft/typescript-go/shim/core v0.0.0
	github.com/microsoft/typescript-go/shim/scanner v0.0.0
	github.com/microsoft/typescript-go/shim/tsoptions v0.0.0
	github.com/microsoft/typescript-go/shim/vfs v0.0.0
	github.com/microsoft/typescript-go/shim/vfs/osvfs v0.0.0
)

require (
	github.com/go-json-experiment/json v0.0.0-20260623181947-01eb4420fa68 // indirect
	github.com/klauspost/cpuid/v2 v2.2.10 // indirect
	github.com/microsoft/typescript-go v0.0.0-20260724234109-8d29e62f3585 // indirect
	github.com/x448/float16 v0.8.4 // indirect
	github.com/zeebo/xxh3 v1.1.0 // indirect
	golang.org/x/sync v0.21.0 // indirect
	golang.org/x/sys v0.46.0 // indirect
	golang.org/x/text v0.38.0 // indirect
)

// The shims live in this repository (./shims): each module claims the
// typescript-go path prefix so it may alias and go:linkname that compiler's
// internal packages, scoped to the identifiers this repository actually uses.
// All nine must pin ONE typescript-go revision (pins_test.go enforces it),
// and every compiler bump re-verifies the go:linkname signatures by eye.
replace (
	github.com/microsoft/typescript-go/shim/ast => ./shims/ast
	github.com/microsoft/typescript-go/shim/bundled => ./shims/bundled
	github.com/microsoft/typescript-go/shim/checker => ./shims/checker
	github.com/microsoft/typescript-go/shim/compiler => ./shims/compiler
	github.com/microsoft/typescript-go/shim/core => ./shims/core
	github.com/microsoft/typescript-go/shim/scanner => ./shims/scanner
	github.com/microsoft/typescript-go/shim/tsoptions => ./shims/tsoptions
	github.com/microsoft/typescript-go/shim/vfs => ./shims/vfs
	github.com/microsoft/typescript-go/shim/vfs/osvfs => ./shims/vfs/osvfs
)
