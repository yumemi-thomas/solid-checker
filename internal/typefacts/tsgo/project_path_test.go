package tsgo

import "testing"

func TestNormalizeTypeScriptPath(t *testing.T) {
	t.Parallel()

	const windowsPath = `D:\a\solid-check\solid-check\internal\reactiveir\testdata\tracer-corrected\tsconfig.json`
	const want = `D:/a/solid-check/solid-check/internal/reactiveir/testdata/tracer-corrected/tsconfig.json`
	if got := normalizeTypeScriptPath(windowsPath); got != want {
		t.Fatalf("normalizeTypeScriptPath(%q) = %q, want %q", windowsPath, got, want)
	}
}
