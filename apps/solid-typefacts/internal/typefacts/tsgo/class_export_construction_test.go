package tsgo

import (
	"context"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/yumemi-thomas/solid-checker/apps/solid-typefacts/internal/typefacts"
)

// TestAClassExportCensusesItsConstruction pins ADR 0105. A class export is
// constructed, not called: `SignatureKindCall` yields nothing for it, and the
// `!= 1` check reported the same `callSignatureNotUnique` for "none" as for
// "several", so the transcript returned before any implementation was looked
// for and the construction census was unreachable for an export.
//
// The refusing rows are the point. Selecting the construct signature finds a
// constructor *body*, and a construction runs more than that — so every reason
// `classConstructorAt` draws (ADR 0047's line) has to survive this path, or
// the premise would certify a class whose field initializers and heritage
// constructor were never censused.
func TestAClassExportCensusesItsConstruction(t *testing.T) {
	for _, testCase := range []struct {
		name           string
		runtime        string
		declaration    string
		wantName       string
		wantReason     string
		lastOccurrence bool
		why            string
	}{
		{
			name: "bundlerClassExpression",
			runtime: `var Store = class {
	constructor(value, actionsFactory) {
		this.value = value;
		if (actionsFactory) this.actions = actionsFactory(this);
	}
	get() { return this.value; }
};
export { Store };
`,
			declaration: "export declare class Store<T> {\n\tconstructor(value: T, actionsFactory?: (store: Store<T>) => unknown);\n\tget(): T;\n}\n",
			wantName:    "Store",
			why:         "`var Store = class {…}` is what every bundler emits for `class Store {…}`, and @tanstack/store ships exactly this; the class expression is anonymous, so the enclosing binding is what names the transcript",
		},
		{
			name: "classDeclaration",
			runtime: `class Store {
	constructor(value) {
		this.value = value;
	}
}
export { Store };
`,
			declaration:    "export declare class Store<T> {\n\tconstructor(value: T);\n}\n",
			wantName:       "Store",
			lastOccurrence: true,
			why:            "the unbundled spelling answers the same when the demand lands on a value position, or the census would be an accident of compilation",
		},
		{
			name: "heritageClauseRefuses",
			runtime: `var Store = class extends WeakMap {
	constructor(value) {
		super();
		this.value = value;
	}
};
export { Store };
`,
			declaration: "export declare class Store<T> extends WeakMap<object, unknown> {\n\tconstructor(value: T);\n}\n",
			wantReason:  "classHeritageClause",
			why:         "`extends WeakMap` runs another constructor this body census is silent about",
		},
		{
			name: "fieldInitializerRefuses",
			runtime: `var Store = class {
	triggers = new Map();
	constructor(value) {
		this.value = value;
	}
};
export { Store };
`,
			declaration: "export declare class Store<T> {\n\ttriggers: Map<unknown, unknown>;\n\tconstructor(value: T);\n}\n",
			wantReason:  "classFieldInitializer",
			why:         "a field initializer runs at construction as surely as the constructor body, and it is a different node the demand does not name",
		},
		{
			name: "implicitConstructorRefuses",
			runtime: `var Store = class {
	get() { return 1; }
};
export { Store };
`,
			declaration: "export declare class Store {\n\tget(): number;\n}\n",
			wantReason:  "implementationUnavailable",
			why: "an implicit constructor has no body to select, so the refusal lands before the class gate; " +
				"`classConstructorAt` would refuse it too, and either way nothing is censused",
		},
		{
			// The limitation, pinned rather than left to be rediscovered.
			name: "aDemandAtTheDeclaringNameRefuses",
			runtime: `class Store {
	constructor(value) {
		this.value = value;
	}
}
export { Store };
`,
			declaration: "export declare class Store<T> {\n\tconstructor(value: T);\n}\n",
			wantReason:  "callSignatureNotUnique",
			why: "the type at a class declaration's own name is the *instance* type, which has neither a call " +
				"nor a construct signature -- the trap ADR 0099's comment records from the `Box` fixture. " +
				"Asking the class node itself answers the same instance type, so this demand states nothing",
		},
	} {
		t.Run(testCase.name, func(t *testing.T) {
			dir := t.TempDir()
			write := func(name, source string) {
				if err := os.WriteFile(filepath.Join(dir, name), []byte(source), 0o644); err != nil {
					t.Fatal(err)
				}
			}
			write("tsconfig.json", `{"compilerOptions":{"strict":true,"module":"esnext","target":"esnext","moduleResolution":"bundler","allowJs":true,"checkJs":false},"files":["harness.ts","index.js","index.d.ts"]}`)
			write("index.d.ts", testCase.declaration)
			write("index.js", testCase.runtime)
			harness := "import { Store } from \"./index.js\";\nvoid Store;\n"
			write("harness.ts", harness)

			opened, err := OpenProject(context.Background(), filepath.Join(dir, "tsconfig.json"), nil)
			if err != nil {
				t.Fatal(err)
			}
			defer opened.Close()
			analyzer := opened.(typefacts.ExportValueAnalyzer)
			queryStart := strings.LastIndex(harness, "void Store") + len("void ")
			implStart := strings.Index(testCase.runtime, "Store")
			if testCase.lastOccurrence {
				implStart = strings.LastIndex(testCase.runtime, "Store")
			}
			answer, err := analyzer.ExportValueTranscripts(context.Background(),
				[]typefacts.ExportValueDemand{{
					Location: typefacts.Location{
						Path:      filepath.Join(dir, "harness.ts"),
						StartByte: queryStart, EndByte: queryStart + len("Store"),
					},
					ImplementationLocation: &typefacts.Location{
						Path:      filepath.Join(dir, "index.js"),
						StartByte: implStart, EndByte: implStart + len("Store"),
					},
				}})
			if err != nil {
				t.Fatal(err)
			}
			if len(answer.Transcripts) != 1 || answer.Transcripts[0].Implementation == nil {
				t.Fatalf("transcripts = %#v, want one with an implementation", answer.Transcripts)
			}
			got := answer.Transcripts[0].Implementation
			if testCase.wantReason != "" {
				if got.Complete {
					t.Fatalf("transcript went complete; %s", testCase.why)
				}
				if len(got.OpenReasons) != 1 || got.OpenReasons[0] != testCase.wantReason {
					t.Fatalf("open reasons = %v, want exactly [%s]: %s",
						got.OpenReasons, testCase.wantReason, testCase.why)
				}
				return
			}
			if !got.Complete {
				t.Fatalf("transcript is open with %v; %s", got.OpenReasons, testCase.why)
			}
			// The transcript is about the class the consumer demanded. A
			// constructor has no name of its own, and answering "constructor"
			// to a query for "Store" is what the session refuses outright.
			if got.Declaration == nil || got.Declaration.Name != testCase.wantName {
				t.Fatalf("declaration = %#v, want the class named %q: %s",
					got.Declaration, testCase.wantName, testCase.why)
			}
			if got.Signature == nil {
				t.Fatal("no signature selected for a construction")
			}
		})
	}
}
