package tsgo

import (
	"context"
	"path/filepath"
	"strings"
	"testing"

	"github.com/yumemi-thomas/solid-checker/apps/solid-typefacts/internal/typefacts"
)

func TestAnonymousLocalCallableBindsItsOwnCompilerSymbol(t *testing.T) {
	const source = `const factory = (token: string) => (key: string) => key.startsWith(token);
export const entry = factory("--");`
	analyzer, dir := markerProject(t, map[string]string{"anonymous.ts": source})
	path := filepath.Join(dir, "anonymous.ts")
	entryStart := strings.Index(source, "entry")
	entry := typefacts.Location{Path: path, StartByte: entryStart, EndByte: entryStart + len("entry")}
	start := strings.Index(source, "(key: string)")
	end := strings.Index(source, ";")
	for _, shift := range []int{0, 1} {
		arrow := typefacts.Location{Path: path, StartByte: start + shift, EndByte: end}
		answer, err := analyzer.ExportValueTranscripts(context.Background(), []typefacts.ExportValueDemand{{Location: entry, LocalDeclarationLocation: &arrow}})
		if err != nil {
			t.Fatal(err)
		}
		transcript := answer.Transcripts[0].LocalDeclaration
		if transcript == nil {
			t.Fatal("missing local transcript")
		}
		if shift == 0 {
			if transcript.ControlFlow == nil || transcript.Declaration == nil || transcript.Declaration.Symbol == "" || transcript.Declaration.Location != arrow {
				t.Fatalf("exact anonymous node lacks independently bound census: %+v", transcript)
			}
		} else if transcript.ControlFlow != nil || len(transcript.OpenReasons) == 0 {
			t.Fatalf("shifted span obtained an anonymous census: %+v", transcript)
		}
	}
}
