package tsgo

import "testing"

func TestNormalizeTypeScriptPath(t *testing.T) {
	t.Parallel()

	const windowsPath = `\\?\D:\a\solid-check\solid-check\internal\reactiveir\testdata\tracer-corrected\tsconfig.json`
	const want = `D:/a/solid-check/solid-check/internal/reactiveir/testdata/tracer-corrected/tsconfig.json`
	if got := normalizeTypeScriptPath(windowsPath); got != want {
		t.Fatalf("normalizeTypeScriptPath(%q) = %q, want %q", windowsPath, got, want)
	}
	const wantDirectory = `D:/a/solid-check/solid-check/internal/reactiveir/testdata/tracer-corrected`
	if got := typeScriptPathDir(windowsPath); got != wantDirectory {
		t.Fatalf("typeScriptPathDir(%q) = %q, want %q", windowsPath, got, wantDirectory)
	}

	const extendedUNCPath = `\\?\UNC\server\share\project\tsconfig.json`
	const wantUNCPath = `//server/share/project/tsconfig.json`
	if got := normalizeTypeScriptPath(extendedUNCPath); got != wantUNCPath {
		t.Fatalf("normalizeTypeScriptPath(%q) = %q, want %q", extendedUNCPath, got, wantUNCPath)
	}
}
