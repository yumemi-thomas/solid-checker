module github.com/yumemi-thomas/solid-checker

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
	github.com/microsoft/typescript-go v0.0.0-20260708042240-2bd066d87f5b // indirect
	github.com/x448/float16 v0.8.4 // indirect
	github.com/zeebo/xxh3 v1.1.0 // indirect
	golang.org/x/sync v0.21.0 // indirect
	golang.org/x/sys v0.46.0 // indirect
	golang.org/x/text v0.38.0 // indirect
)

// Keep every unstable shim on one reviewed tsgolint revision.
replace (
	github.com/microsoft/typescript-go/shim/ast => github.com/oxc-project/tsgolint/shim/ast v0.0.0-20260714154531-c3269c01a0c8
	github.com/microsoft/typescript-go/shim/bundled => github.com/oxc-project/tsgolint/shim/bundled v0.0.0-20260714154531-c3269c01a0c8
	github.com/microsoft/typescript-go/shim/checker => github.com/oxc-project/tsgolint/shim/checker v0.0.0-20260714154531-c3269c01a0c8
	github.com/microsoft/typescript-go/shim/compiler => github.com/oxc-project/tsgolint/shim/compiler v0.0.0-20260714154531-c3269c01a0c8
	github.com/microsoft/typescript-go/shim/core => github.com/oxc-project/tsgolint/shim/core v0.0.0-20260714154531-c3269c01a0c8
	github.com/microsoft/typescript-go/shim/scanner => github.com/oxc-project/tsgolint/shim/scanner v0.0.0-20260714154531-c3269c01a0c8
	github.com/microsoft/typescript-go/shim/tsoptions => github.com/oxc-project/tsgolint/shim/tsoptions v0.0.0-20260714154531-c3269c01a0c8
	github.com/microsoft/typescript-go/shim/vfs => github.com/oxc-project/tsgolint/shim/vfs v0.0.0-20260714154531-c3269c01a0c8
	github.com/microsoft/typescript-go/shim/vfs/osvfs => github.com/oxc-project/tsgolint/shim/vfs/osvfs v0.0.0-20260714154531-c3269c01a0c8
)
