package tsgo

import (
	"path/filepath"
	"testing"

	"github.com/yumemi-thomas/solid-checker/apps/solid-typefacts/internal/typefacts"
)

// TestASpreadAssignmentStatesItsSubjectRoot pins where the object-literal
// spread currently roots and, more importantly, where it stops.
//
// `{ ...a }` is the `reads` frontier's largest single shape — eighty rows on
// `@corvu/utils` and `@corvu-next/utils`' `combineStyle` alone — and the
// premise those rows need is *not* "a spread of a parameter". The producer
// already states that one. What refuses is `written-parameter`: a parameter
// assigned anywhere in the body, whose value at the spread is therefore a join
// rather than the caller's argument.
//
// The boundary is narrower than it looks. A self-assignment already clears,
// because the walk finds the parameter itself as the only source. Assigning
// *another parameter* refuses — even though a join over two caller-supplied
// values is the same argument ADR 0034 already makes — and so does an own
// literal, a local call's result, and a default-library construction. Those are
// four different premises behind one refusal, and this test is what will show
// which of them a later change actually admitted.
//
// `any` throughout, never `unknown`: the corpus code these shapes come from is
// untyped, and a stricter annotation can clear a form through the type rather
// than through the premise under test — which is how a refusal case passes for
// the wrong reason.
func TestASpreadAssignmentStatesItsSubjectRoot(t *testing.T) {
	const (
		rootParameter = typefacts.SubjectRootParameter
		rootDefault   = typefacts.SubjectRootParameterDefaultLiteral
		rootOwn       = typefacts.SubjectRootOwnLiteral
	)
	for _, testCase := range []struct {
		name   string
		source string
		// The root each SpreadAssignment states, in source order.
		roots []typefacts.SubjectRootDerivation
		why   string
	}{
		{
			name: "unwrittenParameters",
			source: `export function subject(a: any, b: any) {
	return { ...a, ...b };
}
`,
			roots: []typefacts.SubjectRootDerivation{rootParameter, rootParameter},
			why:   "the spread reads the caller's own enumerable properties, which is ADR 0034's case exactly",
		},
		{
			name: "selfAssignmentStillRoots",
			source: `export function subject(a: any, b: any) {
	if (typeof b === "string") { b = b; }
	return { ...a, ...b };
}
`,
			roots: []typefacts.SubjectRootDerivation{rootParameter, rootParameter},
			why:   "the binding walk finds the parameter itself as the only assigned value, so the write changes nothing",
		},
		{
			name: "anotherParameterRefuses",
			source: `export function subject(a: any, b: any) {
	if (typeof b === "string") { b = a; }
	return { ...a, ...b };
}
`,
			roots: []typefacts.SubjectRootDerivation{rootParameter, ""},
			why:   "both arms are the caller's, so this is the narrowest premise the frontier wants — and it refuses today",
		},
		{
			name: "ownLiteralRefuses",
			source: `export function subject(a: any, b: any) {
	if (typeof b === "string") { b = { x: 1 }; }
	return { ...a, ...b };
}
`,
			roots: []typefacts.SubjectRootDerivation{rootParameter, ""},
			why:   "a join of the caller's value with a literal this body created is two roots, and the form carries one",
		},
		{
			name: "localCallResultRefuses",
			source: `export function subject(a: any, b: any) {
	if (typeof b === "string") { b = toObject(b); }
	return { ...a, ...b };
}
function toObject(s: string): any { return { y: 2 }; }
`,
			roots: []typefacts.SubjectRootDerivation{rootParameter, ""},
			why:   "`combineStyle`'s own shape: the second arm is a package-own function's result, which needs a hop this census does not take",
		},
		{
			name: "defaultLibraryConstructionRefuses",
			source: `export function subject(a: any, b: any) {
	if (typeof b === "string") { b = Object.create(null); }
	return { ...a, ...b };
}
`,
			roots: []typefacts.SubjectRootDerivation{rootParameter, ""},
			why:   "a reviewed default-library alias roots a call, but nothing here carries that to a written parameter",
		},
		{
			name: "aDefaultedParameterRootsWithoutAWrite",
			source: `export function subject(a: any, b: any = {}) {
	return { ...a, ...b };
}
`,
			roots: []typefacts.SubjectRootDerivation{rootParameter, rootDefault},
			why:   "ADR 0090 already covers the initializer the engine runs; it is not a write in the body",
		},
		{
			name: "anOwnLiteralBindingRoots",
			source: `export function subject(a: any) {
	const own = { x: 1 };
	return { ...own, ...a };
}
`,
			roots: []typefacts.SubjectRootDerivation{rootOwn, rootParameter},
			why:   "ADR 0044: the body created the object, so its own properties are this package's",
		},
	} {
		t.Run(testCase.name, func(t *testing.T) {
			analyzer, dir := markerProject(t, map[string]string{"subject.ts": testCase.source})
			transcript := implementationTranscriptFor(
				t, analyzer, filepath.Join(dir, "subject.ts"), testCase.source, "subject")
			var roots []typefacts.SubjectRootDerivation
			for _, form := range transcript.UncensusedInvokingForms {
				if form.NodeKind == "SpreadAssignment" {
					roots = append(roots, form.SubjectRoot)
				}
			}
			if len(roots) != len(testCase.roots) {
				t.Fatalf("recorded %d spread form(s) %q, want %d: %s",
					len(roots), roots, len(testCase.roots), testCase.why)
			}
			for i, want := range testCase.roots {
				if roots[i] != want {
					t.Fatalf("spread %d states root %q, want %q: %s",
						i, roots[i], want, testCase.why)
				}
			}
		})
	}
}
