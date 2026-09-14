package tsgo

import (
	"path/filepath"
	"testing"

	"github.com/yumemi-thomas/solid-checker/apps/solid-typefacts/internal/typefacts"
)

// TestARestParameterAliasSpreadsThroughTheEngineIterator pins ADR 0106. The
// direct case — spreading a rest parameter's own binding — was already cleared;
// this follows it one hop, to the binding every compiled debounce-alike assigns
// the rest array to. The refusing rows are the point: the premise is "every
// value of this binding is a rest array", so any write that could put something
// else there has to refuse.
func TestARestParameterAliasSpreadsThroughTheEngineIterator(t *testing.T) {
	for _, testCase := range []struct {
		name    string
		source  string
		cleared bool
		why     string
	}{
		{
			name: "aliasOfARestParameter",
			source: `export function subject(fn: (...values: any[]) => void) {
	let args: any;
	return (...a: any[]) => {
		args = a;
		queueMicrotask(() => fn(...args));
	};
}
`,
			cleared: true,
			why:     "`@solid-primitives/utils`'s createMicrotask, and the shape of every compiled debounce-alike",
		},
		{
			name: "aliasAssignedSomethingElseRefuses",
			source: `export function subject(fn: (...values: any[]) => void, other: any) {
	let args: any;
	return (...a: any[]) => {
		args = a;
		args = other;
		queueMicrotask(() => fn(...args));
	};
}
`,
			why: "a caller-supplied array can be a Proxy or carry its own Symbol.iterator, so one such write makes the spread user code",
		},
		{
			name: "aWrittenRestParameterRefuses",
			source: `export function subject(fn: (...values: any[]) => void, other: any) {
	let args: any;
	return (...a: any[]) => {
		a = other;
		args = a;
		queueMicrotask(() => fn(...args));
	};
}
`,
			why: "the hop is only as good as its source: a reassigned rest binding may hold anything, which is why the direct check requires it unwritten",
		},
		{
			name: "aCompoundAssignmentRefuses",
			source: `export function subject(fn: (...values: any[]) => void) {
	let args: any = [];
	return (...a: any[]) => {
		args = a;
		args = args.concat(a);
		queueMicrotask(() => fn(...args));
	};
}
`,
			why: "a value this walk cannot read a single expression out of refuses the whole binding rather than being skipped",
		},
		{
			name: "anUnassignedBindingRefuses",
			source: `export function subject(fn: (...values: any[]) => void) {
	let args: any;
	return () => {
		queueMicrotask(() => fn(...args));
	};
}
`,
			why: "it holds undefined, whose spread throws before any lookup; clearing a form on a value that can only throw states nothing worth stating",
		},
	} {
		t.Run(testCase.name, func(t *testing.T) {
			analyzer, dir := markerProject(t, map[string]string{"subject.ts": testCase.source})
			transcript := implementationTranscriptFor(
				t, analyzer, filepath.Join(dir, "subject.ts"), testCase.source, "subject")
			spread := 0
			for _, form := range transcript.UncensusedInvokingForms {
				if form.Kind == typefacts.UncensusedIterationProtocol {
					spread += 1
				}
			}
			if testCase.cleared && spread != 0 {
				t.Fatalf("recorded %d iteration-protocol form(s), want none: %s", spread, testCase.why)
			}
			if !testCase.cleared && spread == 0 {
				t.Fatalf("recorded no iteration-protocol form, want one: %s", testCase.why)
			}
		})
	}
}
