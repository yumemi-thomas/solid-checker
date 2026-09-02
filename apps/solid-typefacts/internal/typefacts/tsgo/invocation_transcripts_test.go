package tsgo

import (
	"context"
	"os"
	"os/exec"
	"path/filepath"
	"slices"
	"strings"
	"testing"
	"unicode/utf8"

	"github.com/yumemi-thomas/solid-checker/apps/solid-typefacts/internal/typefacts"
)

func TestInvocationTranscriptSelectsGenericSignatureAndExpandsExactTupleSpread(t *testing.T) {
	dir := t.TempDir()
	source := `
type Options = {
  mode: "read" | "write";
  nested?: { callback: (value: number) => void };
};

function execute<T extends Options>(options: T, ...steps: [boolean, (value: number) => void]): Promise<T> {
  options.nested?.callback(1);
  steps[1](1);
  return Promise.resolve(options);
}

func TestInvocationDemandDigestBindsOrderAndEveryDemandOption(t *testing.T) {
	demands := []typefacts.InvocationDemand{
		{Location: typefacts.Location{Path: "/p/a.ts", StartByte: 4, EndByte: 12}, CallableDepth: 3, Census: true},
		{Location: typefacts.Location{Path: "/p/b.ts", StartByte: 0, EndByte: 7}},
	}
	const expected = "sha256:68a28ae5071bb694387c8d372c3a3febd48636f783d4a5881ece0a15656fb88c"
	if got := invocationDemandDigest(demands); got != expected {
		t.Fatalf("invocation demand digest = %q, want cross-language fixture %q", got, expected)
	}
	demands[0], demands[1] = demands[1], demands[0]
	if got := invocationDemandDigest(demands); got == expected {
		t.Fatal("invocation demand digest ignored demand order")
	}
}

const callback = (value: number) => void value;
const steps = [true, callback] as const;
execute({ mode: "read", nested: { callback } }, ...steps);
`
	configPath := filepath.Join(dir, "tsconfig.json")
	sourcePath := filepath.Join(dir, "facts.ts")
	if err := os.WriteFile(configPath, []byte(`{"compilerOptions":{"strict":true,"module":"esnext","target":"esnext"},"include":["*.ts"]}`), 0o644); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(sourcePath, []byte(source), 0o644); err != nil {
		t.Fatal(err)
	}
	opened, err := OpenProject(context.Background(), configPath, nil)
	if err != nil {
		t.Fatal(err)
	}
	defer opened.Close()
	analyzer, ok := opened.(typefacts.InvocationAnalyzer)
	if !ok {
		t.Fatal("TypeScript-Go project does not implement InvocationAnalyzer")
	}
	needle := `execute({ mode: "read", nested: { callback } }, ...steps)`
	start := strings.LastIndex(source, needle)
	answer, err := analyzer.InvocationTranscripts(context.Background(), []typefacts.InvocationDemand{{
		Location:      typefacts.Location{Path: sourcePath, StartByte: start, EndByte: start + len(needle)},
		CallableDepth: 3,
		Census:        true,
	}})
	if err != nil {
		t.Fatal(err)
	}
	if len(answer.Transcripts) != 1 {
		t.Fatalf("transcripts = %d, want 1", len(answer.Transcripts))
	}
	transcript := answer.Transcripts[0]
	if transcript.SelectedSignature == nil || transcript.SelectedSignature.Identity == "" {
		t.Fatalf("selected signature = %#v, want durable identity", transcript.SelectedSignature)
	}
	if got := transcript.SelectedSignature.OverloadCount; got != 1 {
		t.Fatalf("overload count = %d, want exact single-signature census", got)
	}
	if got := transcript.SelectedSignature.MinimumArgumentCount; got != 3 {
		t.Fatalf("minimum argument count = %d, want 3", got)
	}
	if !transcript.SelectedSignature.HasRest || len(transcript.SelectedSignature.Parameters) != 2 {
		t.Fatalf("selected parameters = %#v, want final rest formal", transcript.SelectedSignature.Parameters)
	}
	if len(transcript.Bindings) != 2 {
		t.Fatalf("bindings = %#v, want two written arguments", transcript.Bindings)
	}
	if got := transcript.Bindings[1].Disposition; got != typefacts.ArgumentBindingExactTupleSpread {
		t.Fatalf("spread disposition = %q, want exact tuple", got)
	}
	if got := len(transcript.Bindings[1].Slots); got != 2 {
		t.Fatalf("spread slots = %d, want 2", got)
	}
	if transcript.Bindings[1].Slots[0].ParameterIndex != 1 ||
		transcript.Bindings[1].Slots[1].ParameterIndex != 1 ||
		!transcript.Bindings[1].Slots[1].Rest {
		t.Fatalf("spread mappings = %#v, want both slots mapped to rest formal", transcript.Bindings[1].Slots)
	}
	if !transcript.Completeness.Contains(typefacts.InvocationDomainBindings) ||
		!transcript.Completeness.Contains(typefacts.InvocationDomainOmissions) {
		t.Fatalf("completeness = %#v, want bindings and omissions closed", transcript.Completeness)
	}
	seenCompleteness := make(map[typefacts.InvocationDomain]bool, len(transcript.Completeness))
	for _, domain := range transcript.Completeness {
		if seenCompleteness[domain] {
			t.Fatalf("completeness contains duplicate domain %q: %#v", domain, transcript.Completeness)
		}
		seenCompleteness[domain] = true
	}
	parameter := transcript.SelectedSignature.Parameters[0]
	if !hasCallablePath(parameter.CallablePaths, "nested", "callback") {
		t.Fatalf("callable paths = %#v, want nested.callback", parameter.CallablePaths)
	}
	if !hasFinitePartition(parameter.Value.Partitions, typefacts.FinitePartitionDiscriminant) {
		t.Fatalf("parameter partitions = %#v, want mode discriminant", parameter.Value.Partitions)
	}
	if !hasFinitePartition(transcript.SelectedSignature.Result.Partitions, typefacts.FinitePartitionProtocol) {
		t.Fatalf("result partitions = %#v, want Promise protocol", transcript.SelectedSignature.Result.Partitions)
	}
	for _, fact := range transcript.SelectedSignature.ResultCallablePaths {
		for _, segment := range fact.Path {
			if !utf8.ValidString(segment.Property) {
				t.Fatalf("compiler-internal escaped property crossed the wire: %q", segment.Property)
			}
		}
	}
	if transcript.ParameterUses == nil || transcript.ControlFlow == nil {
		t.Fatalf("census missing: uses=%#v flow=%#v", transcript.ParameterUses, transcript.ControlFlow)
	}
	if answer.Envelope.Generation != 1 || answer.Envelope.DemandSHA256 == "" ||
		answer.Envelope.ModuleGraphSHA256 == "" || len(answer.Envelope.Sources) < 1 {
		t.Fatalf("envelope = %#v, want generation and all proof identities", answer.Envelope)
	}
}

func TestInvocationTranscriptCountsOverloadsAndKeepsUnresolvedGenericLocal(t *testing.T) {
	dir := t.TempDir()
	source := `
function choose(value: string): string;
function choose(value: number): number;
function choose(value: string | number): string | number { return value; }
declare function forward<T>(value: T): T;
function generic<T>(value: T) { return forward(value); }
choose(1);
`
	configPath := filepath.Join(dir, "tsconfig.json")
	sourcePath := filepath.Join(dir, "facts.ts")
	if err := os.WriteFile(configPath, []byte(`{"compilerOptions":{"strict":true,"module":"esnext","target":"esnext"},"include":["*.ts"]}`), 0o644); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(sourcePath, []byte(source), 0o644); err != nil {
		t.Fatal(err)
	}
	opened, err := OpenProject(context.Background(), configPath, nil)
	if err != nil {
		t.Fatal(err)
	}
	defer opened.Close()
	analyzer := opened.(typefacts.InvocationAnalyzer)
	demands := invocationDemandsForNeedles(
		sourcePath,
		source,
		[]string{"forward(value)", "choose(1)"},
		false,
	)
	answer, err := analyzer.InvocationTranscripts(context.Background(), demands)
	if err != nil {
		t.Fatal(err)
	}
	generic := answer.Transcripts[0].SelectedSignature
	if generic == nil || len(generic.Parameters) != 1 {
		t.Fatalf("generic signature = %#v", generic)
	}
	if !slices.Contains(generic.Parameters[0].Value.OpenReasons, "unresolvedGeneric") ||
		!slices.Contains(generic.Result.OpenReasons, "unresolvedGeneric") {
		t.Fatalf("unresolved generic did not stay locally open: %#v", generic)
	}
	overloaded := answer.Transcripts[1].SelectedSignature
	if overloaded == nil || overloaded.OverloadCount != 3 || overloaded.OverloadOrdinal >= overloaded.OverloadCount {
		t.Fatalf("overload census = %#v, want selected ordinal within three declarations", overloaded)
	}
}

func TestInvocationValueClosesOnlyFixedTupleIndexAndKeepsPathIndexOnItsOwner(t *testing.T) {
	dir := t.TempDir()
	source := `
declare function fixed(): [() => void, number];
declare function optional(): [() => void, number?];
declare function rest(): [() => void, ...number[]];
declare function tupleUnion(): [() => void] | [() => void, number];
declare function array(): number[];
declare function indexed(): { fixed: { nested: () => void }; [key: string]: unknown };
declare function singleOptional(): [number?];
declare function singleRest(): [...number[]];
fixed();
optional();
rest();
tupleUnion();
array();
indexed();
singleOptional();
singleRest();
`
	writeInvocationProject(t, dir, map[string]string{"facts.ts": source})
	analyzer, closeProject := openInvocationAnalyzer(t, dir)
	defer closeProject()
	sourcePath := filepath.Join(dir, "facts.ts")
	needles := []string{
		"fixed()", "optional()", "rest()", "tupleUnion()", "array()", "indexed()",
		"singleOptional()", "singleRest()",
	}
	demands := make([]typefacts.InvocationDemand, len(needles))
	for index, needle := range needles {
		start := strings.LastIndex(source, needle)
		demands[index] = typefacts.InvocationDemand{
			Location:      typefacts.Location{Path: sourcePath, StartByte: start, EndByte: start + len(needle)},
			CallableDepth: 2,
		}
	}
	answer, err := analyzer.InvocationTranscripts(context.Background(), demands)
	if err != nil {
		t.Fatal(err)
	}
	results := make([]typefacts.SelectedSignature, len(answer.Transcripts))
	for index, transcript := range answer.Transcripts {
		if transcript.SelectedSignature == nil {
			t.Fatalf("transcript %d has no selected signature: %#v", index, transcript)
		}
		results[index] = *transcript.SelectedSignature
	}
	if slices.Contains(results[0].Result.OpenReasons, "openIndex") {
		t.Fatalf("fixed tuple result stayed open: %#v", results[0].Result)
	}
	for _, index := range []int{1, 2, 3, 5, 6} {
		if !slices.Contains(results[index].Result.OpenReasons, "openIndex") {
			t.Fatalf("result %d lost its open index: %#v", index, results[index].Result)
		}
	}
	for _, index := range []int{4, 7} {
		if slices.Contains(results[index].Result.OpenReasons, "openIndex") {
			t.Fatalf("array-normalized result %d stayed root-open: %#v", index, results[index].Result)
		}
	}
	for index, wantOpen := range []bool{false, true, true} {
		var root *typefacts.CallablePathFact
		for pathIndex := range results[index].ResultCallablePaths {
			path := &results[index].ResultCallablePaths[pathIndex]
			if len(path.Path) == 0 {
				root = path
				break
			}
		}
		if root == nil || slices.Contains(root.OpenReasons, "openIndex") != wantOpen ||
			root.Complete == wantOpen || (wantOpen && root.SubtreeEnumerated) {
			t.Fatalf("tuple result %d root = %#v, want openIndex=%v", index, root, wantOpen)
		}
	}
	for _, resultIndex := range []int{4, 7} {
		var arrayRoot *typefacts.CallablePathFact
		for index := range results[resultIndex].ResultCallablePaths {
			path := &results[resultIndex].ResultCallablePaths[index]
			if len(path.Path) == 0 {
				arrayRoot = path
				break
			}
		}
		if arrayRoot == nil || arrayRoot.Complete || arrayRoot.SubtreeEnumerated ||
			!slices.Contains(arrayRoot.OpenReasons, "openIndex") {
			t.Fatalf("array-normalized result %d path root = %#v, want openIndex", resultIndex, arrayRoot)
		}
	}

	var root, nested *typefacts.CallablePathFact
	for index := range results[5].ResultCallablePaths {
		path := &results[5].ResultCallablePaths[index]
		switch {
		case len(path.Path) == 0:
			root = path
		case pathNamesEqual(path.Path, "fixed", "nested"):
			nested = path
		}
	}
	if root == nil || root.Complete || root.SubtreeEnumerated || !slices.Contains(root.OpenReasons, "openIndex") {
		t.Fatalf("indexed root = %#v, want its own openIndex", root)
	}
	if nested == nil || !nested.Complete || slices.Contains(nested.OpenReasons, "openIndex") {
		t.Fatalf("indexed descendant = %#v, want closed callable without ancestor openIndex", nested)
	}

	augmentedDir := t.TempDir()
	augmentedSource := `
interface Array<T> { [key: symbol]: unknown }
declare function augmentedFixed(): [() => void, number];
augmentedFixed();
`
	writeInvocationProject(t, augmentedDir, map[string]string{"facts.ts": augmentedSource})
	augmentedAnalyzer, closeAugmentedProject := openInvocationAnalyzer(t, augmentedDir)
	defer closeAugmentedProject()
	augmentedPath := filepath.Join(augmentedDir, "facts.ts")
	needle := "augmentedFixed()"
	start := strings.LastIndex(augmentedSource, needle)
	augmentedAnswer, err := augmentedAnalyzer.InvocationTranscripts(
		context.Background(),
		[]typefacts.InvocationDemand{{
			Location:      typefacts.Location{Path: augmentedPath, StartByte: start, EndByte: start + len(needle)},
			CallableDepth: 2,
		}},
	)
	if err != nil {
		t.Fatal(err)
	}
	augmented := augmentedAnswer.Transcripts[0].SelectedSignature
	if augmented == nil || !slices.Contains(augmented.Result.OpenReasons, "openIndex") {
		t.Fatalf("fixed tuple with an augmented symbol index = %#v, want openIndex", augmented)
	}
	var augmentedRoot *typefacts.CallablePathFact
	for index := range augmented.ResultCallablePaths {
		path := &augmented.ResultCallablePaths[index]
		if len(path.Path) == 0 {
			augmentedRoot = path
			break
		}
	}
	if augmentedRoot == nil || augmentedRoot.Complete || augmentedRoot.SubtreeEnumerated ||
		!slices.Contains(augmentedRoot.OpenReasons, "openIndex") {
		t.Fatalf("fixed tuple with an augmented symbol index path root = %#v, want openIndex", augmentedRoot)
	}

	nonNumericDir := t.TempDir()
	nonNumericSource := `
declare function nonNumericFixed(): [number, number];
nonNumericFixed();
`
	nonNumericFiles := map[string]string{
		"globals.d.ts": `
interface Array<T> { length: number; readonly [key: symbol]: unknown }
interface Boolean {}
interface CallableFunction extends Function {}
interface Function {}
interface IArguments {}
interface NewableFunction extends Function {}
interface Number {}
interface Object {}
interface RegExp {}
interface String {}
`,
		"facts.ts": nonNumericSource,
	}
	for name, contents := range nonNumericFiles {
		if err := os.WriteFile(filepath.Join(nonNumericDir, name), []byte(contents), 0o644); err != nil {
			t.Fatal(err)
		}
	}
	if err := os.WriteFile(
		filepath.Join(nonNumericDir, "tsconfig.json"),
		[]byte(`{"compilerOptions":{"strict":true,"noLib":true,"module":"esnext","target":"esnext"},"include":["*.ts"]}`),
		0o644,
	); err != nil {
		t.Fatal(err)
	}
	nonNumericAnalyzer, closeNonNumericProject := openInvocationAnalyzer(t, nonNumericDir)
	defer closeNonNumericProject()
	nonNumericPath := filepath.Join(nonNumericDir, "facts.ts")
	nonNumericNeedle := "nonNumericFixed()"
	nonNumericStart := strings.LastIndex(nonNumericSource, nonNumericNeedle)
	nonNumericAnswer, err := nonNumericAnalyzer.InvocationTranscripts(
		context.Background(),
		[]typefacts.InvocationDemand{{
			Location: typefacts.Location{
				Path:      nonNumericPath,
				StartByte: nonNumericStart,
				EndByte:   nonNumericStart + len(nonNumericNeedle),
			},
			CallableDepth: 1,
		}},
	)
	if err != nil {
		t.Fatal(err)
	}
	nonNumeric := nonNumericAnswer.Transcripts[0].SelectedSignature
	if nonNumeric == nil || !slices.Contains(nonNumeric.Result.OpenReasons, "openIndex") {
		t.Fatalf("fixed tuple with a sole non-numeric index = %#v, want openIndex", nonNumeric)
	}
	var nonNumericRoot *typefacts.CallablePathFact
	for index := range nonNumeric.ResultCallablePaths {
		path := &nonNumeric.ResultCallablePaths[index]
		if len(path.Path) == 0 {
			nonNumericRoot = path
			break
		}
	}
	if nonNumericRoot == nil || nonNumericRoot.Complete || nonNumericRoot.SubtreeEnumerated ||
		!slices.Contains(nonNumericRoot.OpenReasons, "openIndex") {
		t.Fatalf("fixed tuple with a sole non-numeric index path root = %#v, want openIndex", nonNumericRoot)
	}
}

func TestInvocationValueClosesOnlyIntrinsicStringApparentIndex(t *testing.T) {
	dir := t.TempDir()
	source := `
declare function text(): string;
declare function boxed(): String;
declare function constrained<T extends string>(): T;
function preserve<T extends string>() { return constrained<T>(); }
text();
boxed();
`
	writeInvocationProject(t, dir, map[string]string{"facts.ts": source})
	analyzer, closeProject := openInvocationAnalyzer(t, dir)
	defer closeProject()
	sourcePath := filepath.Join(dir, "facts.ts")
	needles := []string{"text()", "boxed()", "constrained<T>()"}
	demands := make([]typefacts.InvocationDemand, len(needles))
	for index, needle := range needles {
		start := strings.LastIndex(source, needle)
		demands[index] = typefacts.InvocationDemand{
			Location:      typefacts.Location{Path: sourcePath, StartByte: start, EndByte: start + len(needle)},
			CallableDepth: 1,
		}
	}
	answer, err := analyzer.InvocationTranscripts(context.Background(), demands)
	if err != nil {
		t.Fatal(err)
	}
	results := make([]typefacts.InvocationValueFact, len(answer.Transcripts))
	paths := make([][]typefacts.CallablePathFact, len(answer.Transcripts))
	for index, transcript := range answer.Transcripts {
		if transcript.SelectedSignature == nil {
			t.Fatalf("transcript %d has no selected signature: %#v", index, transcript)
		}
		results[index] = transcript.SelectedSignature.Result
		paths[index] = transcript.SelectedSignature.ResultCallablePaths
	}
	if slices.Contains(results[0].OpenReasons, "openIndex") {
		t.Fatalf("primitive string stayed root-open: %#v", results[0])
	}
	if !slices.Contains(results[1].OpenReasons, "openIndex") {
		t.Fatalf("boxed String lost its open index: %#v", results[1])
	}
	if slices.Contains(results[2].OpenReasons, "openIndex") ||
		!slices.Contains(results[2].OpenReasons, "unresolvedGeneric") {
		t.Fatalf("constrained generic = %#v, want only unresolvedGeneric to keep it open", results[2])
	}
	for index := range paths {
		var root *typefacts.CallablePathFact
		for pathIndex := range paths[index] {
			candidate := &paths[index][pathIndex]
			if len(candidate.Path) == 0 {
				root = candidate
				break
			}
		}
		if root == nil || root.Complete || !slices.Contains(root.OpenReasons, "openIndex") {
			t.Fatalf("string-shaped result %d path root = %#v, want member-enumeration openIndex", index, root)
		}
	}

	augmentedDir := t.TempDir()
	augmentedSource := `
interface String { readonly [key: symbol]: unknown }
declare function augmentedText(): string;
augmentedText();
`
	writeInvocationProject(t, augmentedDir, map[string]string{"facts.ts": augmentedSource})
	augmentedAnalyzer, closeAugmentedProject := openInvocationAnalyzer(t, augmentedDir)
	defer closeAugmentedProject()
	augmentedPath := filepath.Join(augmentedDir, "facts.ts")
	needle := "augmentedText()"
	start := strings.LastIndex(augmentedSource, needle)
	augmentedAnswer, err := augmentedAnalyzer.InvocationTranscripts(
		context.Background(),
		[]typefacts.InvocationDemand{{
			Location:      typefacts.Location{Path: augmentedPath, StartByte: start, EndByte: start + len(needle)},
			CallableDepth: 1,
		}},
	)
	if err != nil {
		t.Fatal(err)
	}
	augmented := augmentedAnswer.Transcripts[0].SelectedSignature
	if augmented == nil || !slices.Contains(augmented.Result.OpenReasons, "openIndex") {
		t.Fatalf("primitive string with an augmented wrapper index = %#v, want openIndex", augmented)
	}

	assertNoLibRootOpen := func(name, stringMembers, numberMembers, returnType string) {
		t.Helper()
		projectDir := t.TempDir()
		globals := `
interface Array<T> { readonly length: number }
interface Boolean {}
interface CallableFunction extends Function {}
interface Function {}
interface IArguments {}
interface NewableFunction extends Function {}
interface Number { ` + numberMembers + ` }
interface Object {}
interface RegExp {}
interface String { ` + stringMembers + ` }
`
		projectSource := `
declare function observed(): ` + returnType + `;
observed();
`
		writeInvocationProject(t, projectDir, map[string]string{
			"globals.d.ts": globals,
			"facts.ts":     projectSource,
			"tsconfig.json": `{
  "compilerOptions": { "strict": true, "target": "ESNext", "module": "ESNext", "noLib": true },
  "include": ["*.ts"]
}`,
		})
		projectAnalyzer, closeNoLibProject := openInvocationAnalyzer(t, projectDir)
		defer closeNoLibProject()
		projectPath := filepath.Join(projectDir, "facts.ts")
		projectNeedle := "observed()"
		projectStart := strings.LastIndex(projectSource, projectNeedle)
		projectAnswer, projectErr := projectAnalyzer.InvocationTranscripts(
			context.Background(),
			[]typefacts.InvocationDemand{{
				Location: typefacts.Location{
					Path:      projectPath,
					StartByte: projectStart,
					EndByte:   projectStart + len(projectNeedle),
				},
				CallableDepth: 1,
			}},
		)
		if projectErr != nil {
			t.Fatal(projectErr)
		}
		selected := projectAnswer.Transcripts[0].SelectedSignature
		if selected == nil || !slices.Contains(selected.Result.OpenReasons, "openIndex") {
			t.Fatalf("%s result = %#v, want root openIndex", name, selected)
		}
		var root *typefacts.CallablePathFact
		for index := range selected.ResultCallablePaths {
			candidate := &selected.ResultCallablePaths[index]
			if len(candidate.Path) == 0 {
				root = candidate
				break
			}
		}
		if root == nil || root.Complete || !slices.Contains(root.OpenReasons, "openIndex") {
			t.Fatalf("%s callable-path root = %#v, want openIndex", name, root)
		}
	}

	assertNoLibRootOpen(
		"sole symbol wrapper index",
		"readonly [key: symbol]: string",
		"",
		"string",
	)
	assertNoLibRootOpen(
		"sole non-string wrapper index value",
		"readonly [key: number]: unknown",
		"",
		"string",
	)
	assertNoLibRootOpen(
		"mixed primitive domain",
		"readonly [key: number]: string",
		"readonly [key: number]: string",
		"string | number",
	)
}

func TestInvocationValueClosesOnlyExactArrayIndexAtRoot(t *testing.T) {
	dir := t.TempDir()
	source := `
declare function mutable(): string[];
declare function readonly(): ReadonlyArray<string>;
declare function arrayLike(): { readonly [key: number]: string };
interface Indexed<T> { readonly [key: number]: T }
declare function genericArrayLike(): Indexed<string>;
declare function mixed(): string[] | { readonly [key: number]: string };
mutable();
readonly();
arrayLike();
genericArrayLike();
mixed();
`
	writeInvocationProject(t, dir, map[string]string{"facts.ts": source})
	analyzer, closeProject := openInvocationAnalyzer(t, dir)
	defer closeProject()
	path := filepath.Join(dir, "facts.ts")
	needles := []string{"mutable()", "readonly()", "arrayLike()", "genericArrayLike()", "mixed()"}
	demands := make([]typefacts.InvocationDemand, len(needles))
	for index, needle := range needles {
		start := strings.LastIndex(source, needle)
		demands[index] = typefacts.InvocationDemand{
			Location:      typefacts.Location{Path: path, StartByte: start, EndByte: start + len(needle)},
			CallableDepth: 1,
		}
	}
	answer, err := analyzer.InvocationTranscripts(context.Background(), demands)
	if err != nil {
		t.Fatal(err)
	}
	for index, transcript := range answer.Transcripts {
		selected := transcript.SelectedSignature
		if selected == nil {
			t.Fatalf("transcript %d has no selected signature: %#v", index, transcript)
		}
		rootHasOpenIndex := slices.Contains(selected.Result.OpenReasons, "openIndex")
		if index < 2 && rootHasOpenIndex {
			t.Fatalf("exact array result %d stayed root-open: %#v", index, selected.Result)
		}
		if index >= 2 && !rootHasOpenIndex {
			t.Fatalf("non-array result %d lost root openIndex: %#v", index, selected.Result)
		}
		var root *typefacts.CallablePathFact
		for pathIndex := range selected.ResultCallablePaths {
			candidate := &selected.ResultCallablePaths[pathIndex]
			if len(candidate.Path) == 0 {
				root = candidate
				break
			}
		}
		if root == nil || root.Complete || !slices.Contains(root.OpenReasons, "openIndex") {
			t.Fatalf("array-shaped result %d path root = %#v, want member-enumeration openIndex", index, root)
		}
	}

	augmentedDir := t.TempDir()
	augmentedSource := `
interface Array<T> { readonly [key: symbol]: T }
declare function augmented(): string[];
augmented();
`
	writeInvocationProject(t, augmentedDir, map[string]string{"facts.ts": augmentedSource})
	augmentedAnalyzer, closeAugmentedProject := openInvocationAnalyzer(t, augmentedDir)
	defer closeAugmentedProject()
	augmentedPath := filepath.Join(augmentedDir, "facts.ts")
	augmentedNeedle := "augmented()"
	augmentedStart := strings.LastIndex(augmentedSource, augmentedNeedle)
	augmentedAnswer, err := augmentedAnalyzer.InvocationTranscripts(
		context.Background(),
		[]typefacts.InvocationDemand{{
			Location: typefacts.Location{
				Path:      augmentedPath,
				StartByte: augmentedStart,
				EndByte:   augmentedStart + len(augmentedNeedle),
			},
			CallableDepth: 1,
		}},
	)
	if err != nil {
		t.Fatal(err)
	}
	augmented := augmentedAnswer.Transcripts[0].SelectedSignature
	if augmented == nil || !slices.Contains(augmented.Result.OpenReasons, "openIndex") {
		t.Fatalf("array with augmented index = %#v, want root openIndex", augmented)
	}

	assertNoLibArrayRootOpen := func(name, arrayMembers string) {
		t.Helper()
		projectDir := t.TempDir()
		globals := `
interface Array<T> { ` + arrayMembers + ` }
interface Boolean {}
interface CallableFunction extends Function {}
interface Function {}
interface IArguments {}
interface NewableFunction extends Function {}
interface Number {}
interface Object {}
interface RegExp {}
interface String {}
`
		projectSource := `
declare function observed(): string[];
observed();
`
		writeInvocationProject(t, projectDir, map[string]string{
			"globals.d.ts": globals,
			"facts.ts":     projectSource,
			"tsconfig.json": `{
  "compilerOptions": { "strict": true, "target": "ESNext", "module": "ESNext", "noLib": true },
  "include": ["*.ts"]
}`,
		})
		projectAnalyzer, closeNoLibProject := openInvocationAnalyzer(t, projectDir)
		defer closeNoLibProject()
		projectPath := filepath.Join(projectDir, "facts.ts")
		projectNeedle := "observed()"
		projectStart := strings.LastIndex(projectSource, projectNeedle)
		projectAnswer, projectErr := projectAnalyzer.InvocationTranscripts(
			context.Background(),
			[]typefacts.InvocationDemand{{
				Location: typefacts.Location{
					Path:      projectPath,
					StartByte: projectStart,
					EndByte:   projectStart + len(projectNeedle),
				},
				CallableDepth: 1,
			}},
		)
		if projectErr != nil {
			t.Fatal(projectErr)
		}
		selected := projectAnswer.Transcripts[0].SelectedSignature
		if selected == nil || !slices.Contains(selected.Result.OpenReasons, "openIndex") {
			t.Fatalf("%s result = %#v, want root openIndex", name, selected)
		}
	}

	assertNoLibArrayRootOpen("sole symbol Array index", "readonly [key: symbol]: T")
	assertNoLibArrayRootOpen("sole non-element Array index value", "readonly [key: number]: unknown")
}

func TestCallablePathSeparatesLocalShapeFromSubtreeEnumeration(t *testing.T) {
	dir := t.TempDir()
	source := `
interface Recursive { next: Recursive }
declare function nested(): { child: () => void };
declare function leaf(): void;
declare function recursive(): Recursive;
declare function choice(): { map: () => void } | { other: number };
declare function deepChoice(): { next: { callback: () => void } } | Recursive;
declare function genericWrapper<T extends { child: number }>(): { node: T };
function preserve<T extends { child: number }>() { return genericWrapper<T>(); }
nested();
leaf();
recursive();
choice();
deepChoice();
`
	writeInvocationProject(t, dir, map[string]string{"facts.ts": source})
	analyzer, closeProject := openInvocationAnalyzer(t, dir)
	defer closeProject()
	path := filepath.Join(dir, "facts.ts")
	needles := []string{
		"nested()", "leaf()", "recursive()", "choice()", "genericWrapper<T>()", "deepChoice()",
	}
	depths := []int{0, 0, 4, 1, 1, 2}
	demands := make([]typefacts.InvocationDemand, len(needles))
	for index, needle := range needles {
		start := strings.LastIndex(source, needle)
		demands[index] = typefacts.InvocationDemand{
			Location:      typefacts.Location{Path: path, StartByte: start, EndByte: start + len(needle)},
			CallableDepth: depths[index],
		}
	}
	answer, err := analyzer.InvocationTranscripts(context.Background(), demands)
	if err != nil {
		t.Fatal(err)
	}
	selected := make([]*typefacts.SelectedSignature, len(answer.Transcripts))
	for index := range answer.Transcripts {
		selected[index] = answer.Transcripts[index].SelectedSignature
		if selected[index] == nil {
			t.Fatalf("transcript %d has no selected signature: %#v", index, answer.Transcripts[index])
		}
	}
	findPath := func(paths []typefacts.CallablePathFact, names ...string) *typefacts.CallablePathFact {
		t.Helper()
		for index := range paths {
			if pathNamesEqual(paths[index].Path, names...) {
				return &paths[index]
			}
		}
		return nil
	}

	nestedRoot := findPath(selected[0].ResultCallablePaths)
	if nestedRoot == nil || !nestedRoot.Complete || nestedRoot.SubtreeEnumerated || len(nestedRoot.OpenReasons) != 0 {
		t.Fatalf("depth-cut root = %#v, want locally complete with subtreeEnumerated=false", nestedRoot)
	}
	leafRoot := findPath(selected[1].ResultCallablePaths)
	if leafRoot == nil || !leafRoot.Complete || !leafRoot.SubtreeEnumerated || len(leafRoot.OpenReasons) != 0 {
		t.Fatalf("true leaf root = %#v, want local and subtree closure", leafRoot)
	}
	cyclePath := findPath(selected[2].ResultCallablePaths, "next")
	if cyclePath == nil || !cyclePath.Complete || cyclePath.SubtreeEnumerated || len(cyclePath.OpenReasons) != 0 {
		t.Fatalf("cycle-cut path = %#v, want locally complete with subtreeEnumerated=false", cyclePath)
	}
	var absent *typefacts.CallablePathFact
	for index := range selected[3].ResultCallablePaths {
		candidate := &selected[3].ResultCallablePaths[index]
		if candidate.Presence == typefacts.PathAbsent {
			absent = candidate
			break
		}
	}
	if absent == nil || !absent.Complete || !absent.SubtreeEnumerated || len(absent.OpenReasons) != 0 {
		t.Fatalf("synthetic absent alternative = %#v, want a closed empty subtree", absent)
	}
	genericPath := findPath(selected[4].ResultCallablePaths, "node")
	if genericPath == nil || genericPath.Complete ||
		!slices.Contains(genericPath.OpenReasons, "unresolvedGeneric") {
		t.Fatalf("depth-cut constrained generic path = %#v, want unresolvedGeneric", genericPath)
	}
	var cutAlternative *typefacts.CallablePathFact
	for index := range selected[5].ResultCallablePaths {
		candidate := &selected[5].ResultCallablePaths[index]
		if pathNamesEqual(candidate.Path, "next", "callback") &&
			candidate.Presence == typefacts.PathUnknown {
			cutAlternative = candidate
			break
		}
	}
	if cutAlternative == nil || cutAlternative.Complete || cutAlternative.SubtreeEnumerated ||
		!slices.Contains(cutAlternative.OpenReasons, "openAlternative") {
		t.Fatalf("path below a cycle-cut alternative = %#v, want unknown/openAlternative", cutAlternative)
	}
	for index, signature := range selected {
		for _, fact := range signature.ResultCallablePaths {
			if slices.Contains(fact.OpenReasons, "depthLimit") || slices.Contains(fact.OpenReasons, "cycle") {
				t.Fatalf("transcript %d retained a census reason as local openness: %#v", index, fact)
			}
		}
	}
}

func TestCallablePathAbsenceRequiresAClosedRequiredEnumeratedPrefix(t *testing.T) {
	target := []typefacts.PathSegment{{Kind: typefacts.PathSegmentProperty, Property: "child"}}
	prefix := typefacts.CallablePathFact{
		Alternative:       0,
		Presence:          typefacts.PathRequired,
		Callability:       typefacts.CallabilityNonCallable,
		Constructability:  typefacts.InvocationNonConstructable,
		Complete:          true,
		SubtreeEnumerated: true,
	}
	if !callablePathPrefixProvesAbsence([]typefacts.CallablePathFact{prefix}, 0, target) {
		t.Fatal("closed required enumerated prefix did not prove absence")
	}
	mutations := []struct {
		name   string
		mutate func(*typefacts.CallablePathFact)
	}{
		{"optional prefix", func(fact *typefacts.CallablePathFact) { fact.Presence = typefacts.PathOptional }},
		{"incomplete prefix", func(fact *typefacts.CallablePathFact) { fact.Complete = false }},
		{"reason-bearing prefix", func(fact *typefacts.CallablePathFact) {
			fact.OpenReasons = []string{"openType"}
		}},
		{"unenumerated prefix", func(fact *typefacts.CallablePathFact) { fact.SubtreeEnumerated = false }},
	}
	for _, mutation := range mutations {
		t.Run(mutation.name, func(t *testing.T) {
			candidate := prefix
			mutation.mutate(&candidate)
			if callablePathPrefixProvesAbsence([]typefacts.CallablePathFact{candidate}, 0, target) {
				t.Fatalf("%s proved absence: %#v", mutation.name, candidate)
			}
		})
	}
}

func TestInvocationTranscriptDistinguishesOverloadsThroughAnImportAlias(t *testing.T) {
	dir := t.TempDir()
	writeInvocationProject(t, dir, map[string]string{
		"origin.ts": `export function select(value: string): "string";
export function select(value: number): "number";
export function select(value: string | number) { return typeof value === "string" ? "string" : "number"; }
`,
		"use.ts": `import { select as choose } from "./origin";
choose("value");
choose(1);
`,
	})
	analyzer, closeProject := openInvocationAnalyzer(t, dir)
	defer closeProject()
	path := filepath.Join(dir, "use.ts")
	source, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	demands := invocationDemandsForNeedles(path, string(source), []string{`choose("value")`, `choose(1)`}, false)
	answer, err := analyzer.InvocationTranscripts(context.Background(), demands)
	if err != nil {
		t.Fatal(err)
	}
	stringSignature := answer.Transcripts[0].SelectedSignature
	numberSignature := answer.Transcripts[1].SelectedSignature
	if stringSignature == nil || numberSignature == nil {
		t.Fatalf("selected signatures = %#v", answer.Transcripts)
	}
	if stringSignature.Identity == numberSignature.Identity ||
		stringSignature.OverloadOrdinal == numberSignature.OverloadOrdinal {
		t.Fatalf("overload identities collapsed: string=%#v number=%#v", stringSignature, numberSignature)
	}
	if stringSignature.Declaration.Location.Path != filepath.Join(dir, "origin.ts") ||
		numberSignature.Declaration.Location.Path != filepath.Join(dir, "origin.ts") {
		t.Fatalf("alias did not resolve to origin declarations: %#v %#v", stringSignature.Declaration, numberSignature.Declaration)
	}
}

func TestInvocationTranscriptResolvesNamespaceReexportsAndConstructSignatures(t *testing.T) {
	dir := t.TempDir()
	writeInvocationProject(t, dir, map[string]string{
		"origin.ts": `export function select(value: string): string { return value; }
export class Box<T> { constructor(readonly value: T) {} }
`,
		"barrel.ts": `export { select, Box } from "./origin";`,
		"use.ts": `import * as api from "./barrel";
api.select("value");
new api.Box(1);
`,
	})
	analyzer, closeProject := openInvocationAnalyzer(t, dir)
	defer closeProject()
	path := filepath.Join(dir, "use.ts")
	source, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	answer, err := analyzer.InvocationTranscripts(
		context.Background(),
		invocationDemandsForNeedles(path, string(source), []string{`api.select("value")`, `new api.Box(1)`}, false),
	)
	if err != nil {
		t.Fatal(err)
	}
	call, construct := answer.Transcripts[0], answer.Transcripts[1]
	if call.Kind != typefacts.CallKindCall || construct.Kind != typefacts.CallKindConstruct ||
		call.SelectedSignature == nil || construct.SelectedSignature == nil {
		t.Fatalf("namespace call/construct facts = %#v", answer.Transcripts)
	}
	for _, transcript := range []typefacts.InvocationTranscript{call, construct} {
		if transcript.SelectedSignature.Declaration.Location.Path != filepath.Join(dir, "origin.ts") {
			t.Fatalf("reexport did not resolve to origin: %#v", transcript.SelectedSignature.Declaration)
		}
	}
}

func TestInvocationDemandMustNameAnExactCallAndStayWithinDepthLimit(t *testing.T) {
	dir := t.TempDir()
	source := `declare function invoke(value: string): void;
invoke("value");
`
	writeInvocationProject(t, dir, map[string]string{"facts.ts": source})
	analyzer, closeProject := openInvocationAnalyzer(t, dir)
	defer closeProject()
	path := filepath.Join(dir, "facts.ts")
	start := strings.LastIndex(source, `invoke("value")`)
	answer, err := analyzer.InvocationTranscripts(context.Background(), []typefacts.InvocationDemand{{
		Location: typefacts.Location{Path: path, StartByte: start, EndByte: start + len("invoke")},
	}})
	if err != nil {
		t.Fatal(err)
	}
	if answer.Transcripts[0].SelectedSignature != nil ||
		!slices.Contains(answer.Transcripts[0].OpenReasons, "callNotExact") {
		t.Fatalf("non-exact demand widened to a call: %#v", answer.Transcripts[0])
	}
	_, err = analyzer.InvocationTranscripts(context.Background(), []typefacts.InvocationDemand{{
		Location:      typefacts.Location{Path: path, StartByte: start, EndByte: start + len(`invoke("value")`)},
		CallableDepth: typefacts.MaxInvocationCallableDepth + 1,
	}})
	if err == nil {
		t.Fatal("over-limit callable depth was silently truncated")
	}
}

func TestInvocationBindingsKeepOptionalRestAndUnknownSpreadsDistinct(t *testing.T) {
	dir := t.TempDir()
	source := `
declare function variadic(required: string, optional?: number, ...callbacks: Array<() => void>): void;
declare function tupled(...values: [string] | [string, number]): void;
const callback = () => {};
const exact = ["value", 1, callback] as const;
declare const open: Array<string | number | (() => void)>;
declare const choice: [string] | [string, number];
variadic("value");
variadic(...exact);
variadic(...open);
tupled(...choice);
`
	writeInvocationProject(t, dir, map[string]string{"facts.ts": source})
	analyzer, closeProject := openInvocationAnalyzer(t, dir)
	defer closeProject()
	path := filepath.Join(dir, "facts.ts")
	needles := []string{`variadic("value")`, `variadic(...exact)`, `variadic(...open)`, `tupled(...choice)`}
	answer, err := analyzer.InvocationTranscripts(
		context.Background(),
		invocationDemandsForNeedles(path, source, needles, false),
	)
	if err != nil {
		t.Fatal(err)
	}
	omitted := answer.Transcripts[0]
	if !omitted.Completeness.Contains(typefacts.InvocationDomainOmissions) ||
		len(omitted.OmittedParameters) != 1 || omitted.OmittedParameters[0] != 1 {
		t.Fatalf("optional omission = %#v", omitted)
	}
	exact := answer.Transcripts[1]
	if got := exact.Bindings[0].Disposition; got != typefacts.ArgumentBindingExactTupleSpread {
		t.Fatalf("exact spread = %q", got)
	}
	if got := []int{
		exact.Bindings[0].Slots[0].ParameterIndex,
		exact.Bindings[0].Slots[1].ParameterIndex,
		exact.Bindings[0].Slots[2].ParameterIndex,
	}; got[0] != 0 || got[1] != 1 || got[2] != 2 || !exact.Bindings[0].Slots[2].Rest {
		t.Fatalf("exact spread mappings = %#v", exact.Bindings[0].Slots)
	}
	for _, index := range []int{2, 3} {
		transcript := answer.Transcripts[index]
		if transcript.Bindings[0].Disposition != typefacts.ArgumentBindingUnknownLengthSpread ||
			transcript.Completeness.Contains(typefacts.InvocationDomainBindings) ||
			transcript.Completeness.Contains(typefacts.InvocationDomainOmissions) {
			t.Fatalf("open spread %d certified binding: %#v", index, transcript)
		}
	}
}

func TestCallablePathsProveUnionSiblingAbsenceLocally(t *testing.T) {
	dir := t.TempDir()
	source := `
type Variant =
  | { kind: "callback"; nested: { run(): void } }
  | { kind: "value"; nested: { value: number } };
declare function consume(variant: Variant): void;
declare const variant: Variant;
consume(variant);
`
	writeInvocationProject(t, dir, map[string]string{"facts.ts": source})
	analyzer, closeProject := openInvocationAnalyzer(t, dir)
	defer closeProject()
	path := filepath.Join(dir, "facts.ts")
	answer, err := analyzer.InvocationTranscripts(
		context.Background(),
		invocationDemandsForNeedles(path, source, []string{"consume(variant)"}, false),
	)
	if err != nil {
		t.Fatal(err)
	}
	paths := answer.Transcripts[0].SelectedSignature.Parameters[0].CallablePaths
	var runPresent, runAbsent bool
	for _, path := range paths {
		if !pathNamesEqual(path.Path, "nested", "run") {
			continue
		}
		runPresent = runPresent || path.Presence != typefacts.PathAbsent && path.Callability == typefacts.CallabilityCallable
		runAbsent = runAbsent || path.Presence == typefacts.PathAbsent && path.Complete
	}
	if !runPresent || !runAbsent {
		t.Fatalf("union-local run facts = %#v", paths)
	}
}

func TestProtocolPartitionUsesTheCompilerForStructuralAsyncIterables(t *testing.T) {
	dir := t.TempDir()
	source := `
interface StructuralStream {
  [Symbol.asyncIterator](): AsyncIterator<number>;
}
declare function choose(): Promise<number> | StructuralStream | number;
choose(/*invoke*/);
`
	writeInvocationProject(t, dir, map[string]string{"facts.ts": source})
	analyzer, closeProject := openInvocationAnalyzer(t, dir)
	defer closeProject()
	path := filepath.Join(dir, "facts.ts")
	answer, err := analyzer.InvocationTranscripts(
		context.Background(),
		invocationDemandsForNeedles(path, source, []string{"choose(/*invoke*/)"}, false),
	)
	if err != nil {
		t.Fatal(err)
	}
	partitions := answer.Transcripts[0].SelectedSignature.Result.Partitions
	var protocol *typefacts.FinitePartition
	for index := range partitions {
		if partitions[index].Axis == typefacts.FinitePartitionProtocol {
			protocol = &partitions[index]
			break
		}
	}
	if protocol == nil || !protocol.Complete || len(protocol.Cases) != 3 {
		t.Fatalf("protocol partition = %#v, want exact plain/Promise/AsyncIterable", protocol)
	}
	got := make(map[typefacts.ValueProtocol]bool)
	for _, candidate := range protocol.Cases {
		got[candidate.Protocol] = true
	}
	for _, expected := range []typefacts.ValueProtocol{
		typefacts.ValueProtocolPlain,
		typefacts.ValueProtocolPromise,
		typefacts.ValueProtocolAsyncIterable,
	} {
		if !got[expected] {
			t.Fatalf("protocol partition lacks %q: %#v", expected, protocol.Cases)
		}
	}
}

func TestFinitePartitionsDoNotInventCallabilityOrDiscriminantCases(t *testing.T) {
	dir := t.TempDir()
	source := `
type Overlap =
  | { kind: "same"; left: "a" }
  | { kind: "same"; right: "b" };
declare function classify(value: (() => void) | Function | number, overlap: Overlap): void;
declare const value: (() => void) | Function | number;
declare const overlap: Overlap;
classify(value, overlap);
`
	writeInvocationProject(t, dir, map[string]string{"facts.ts": source})
	analyzer, closeProject := openInvocationAnalyzer(t, dir)
	defer closeProject()
	path := filepath.Join(dir, "facts.ts")
	answer, err := analyzer.InvocationTranscripts(
		context.Background(),
		invocationDemandsForNeedles(path, source, []string{"classify(value, overlap)"}, false),
	)
	if err != nil {
		t.Fatal(err)
	}
	parameters := answer.Transcripts[0].SelectedSignature.Parameters
	var callability *typefacts.FinitePartition
	for index := range parameters[0].Value.Partitions {
		candidate := &parameters[0].Value.Partitions[index]
		if candidate.Axis == typefacts.FinitePartitionCallability {
			callability = candidate
		}
	}
	if callability == nil || len(callability.Cases) != 3 {
		t.Fatalf("callability partition = %#v, want all three proven categories", callability)
	}
	want := map[string]bool{"callable": true, "untypedCallable": true, "nonCallable": true}
	for _, candidate := range callability.Cases {
		delete(want, candidate.Kind)
	}
	if len(want) != 0 {
		t.Fatalf("callability partition omitted categories: %#v", callability.Cases)
	}
	if hasFinitePartition(parameters[1].Value.Partitions, typefacts.FinitePartitionDiscriminant) {
		t.Fatalf("overlapping object union was called discriminated: %#v", parameters[1].Value.Partitions)
	}
}

func TestInvocationCensusesClassifyAliasesCapturesReturnsThrowsAndUnreachableSites(t *testing.T) {
	dir := t.TempDir()
	source := `
declare function accept(callback: () => void): void;
function inspect(callback: () => void, mode: "return" | "throw") {
  const alias = callback;
  alias();
  accept(callback);
  if (mode === "return") return () => callback();
  throw new Error("stop");
  callback();
}
inspect(() => {}, "return");
`
	writeInvocationProject(t, dir, map[string]string{"facts.ts": source})
	analyzer, closeProject := openInvocationAnalyzer(t, dir)
	defer closeProject()
	path := filepath.Join(dir, "facts.ts")
	demand := invocationDemandsForNeedles(path, source, []string{`inspect(() => {}, "return")`}, true)
	answer, err := analyzer.InvocationTranscripts(context.Background(), demand)
	if err != nil {
		t.Fatal(err)
	}
	transcript := answer.Transcripts[0]
	kinds := make(map[typefacts.ParameterUseKind]bool)
	for _, use := range transcript.ParameterUses {
		kinds[use.Kind] = true
	}
	for _, kind := range []typefacts.ParameterUseKind{
		typefacts.ParameterUseStorage,
		typefacts.ParameterUseAliasCall,
		typefacts.ParameterUseArgumentKnown,
		typefacts.ParameterUseCapture,
	} {
		if !kinds[kind] {
			t.Fatalf("parameter uses lack %q: %#v", kind, transcript.ParameterUses)
		}
	}
	flow := transcript.ControlFlow
	if flow == nil || len(flow.Returns) != 1 || len(flow.Throws) != 1 || len(flow.Branches) != 1 {
		t.Fatalf("control-flow census = %#v", flow)
	}
	if len(flow.Returns[0].CarriedCallables) != 1 {
		t.Fatalf("returned carried callables = %#v", flow.Returns[0].CarriedCallables)
	}
	if got := returnedCaptures(t, transcript); !slices.Equal(got, []int{0}) {
		t.Fatalf("returned closure captures = %v, want [0]", got)
	}
	var unreachableCall bool
	for _, use := range transcript.ParameterUses {
		if strings.Contains(source[use.Location.StartByte:use.Location.EndByte], "callback") &&
			use.Location.StartByte > flow.Throws[0].Location.StartByte {
			unreachableCall = true
		}
	}
	if !unreachableCall {
		t.Fatalf("parameter-use census dropped the use after throw: %#v", transcript.ParameterUses)
	}
}

func TestUnsupportedFlowKeepsOnlyControlFlowOpen(t *testing.T) {
	dir := t.TempDir()
	source := `
function repeat(callback: () => void) { while (Math.random()) callback(); }
repeat(() => {});
`
	writeInvocationProject(t, dir, map[string]string{"facts.ts": source})
	analyzer, closeProject := openInvocationAnalyzer(t, dir)
	defer closeProject()
	path := filepath.Join(dir, "facts.ts")
	answer, err := analyzer.InvocationTranscripts(
		context.Background(),
		invocationDemandsForNeedles(path, source, []string{"repeat(() => {})"}, true),
	)
	if err != nil {
		t.Fatal(err)
	}
	transcript := answer.Transcripts[0]
	if !transcript.Completeness.Contains(typefacts.InvocationDomainSignature) ||
		!transcript.Completeness.Contains(typefacts.InvocationDomainUses) ||
		transcript.Completeness.Contains(typefacts.InvocationDomainControlFlow) ||
		transcript.ControlFlow == nil || len(transcript.ControlFlow.Unsupported) == 0 {
		t.Fatalf("unsupported flow completeness leaked: %#v", transcript)
	}
}

// TestReturnedCaptureDescentAcceptsOnlyIdentityPreservingConstructions pins the
// whitelist in returnedCallablesLocked arm by arm. Every accepted arm keeps the
// callable's runtime identity, so naming its captures is true by construction;
// every refused arm contributes nothing and leaves the demand open.
func TestReturnedCaptureDescentAcceptsOnlyIdentityPreservingConstructions(t *testing.T) {
	cases := []struct {
		name string
		body string
		want []int
	}{{
		name: "directArrowIsUnchanged",
		body: `return () => callback();`,
		want: []int{0},
	}, {
		name: "objectAssignArgumentZero",
		body: `const fn = () => callback();
  const clear = () => {};
  return Object.assign(fn, { clear });`,
		want: []int{0},
	}, {
		// Only argument 0 is descended. `clear` really is carried by the returned
		// value — Object.assign copies the property by reference — but the identity
		// guarantee that was verified here is the one about the target, so `wait`
		// stays unproven rather than being inferred from the source object.
		name: "objectAssignThroughTypeOnlyWrappersDescendsOnlyTheTarget",
		body: `const fn = () => callback();
  const clear = () => { sink(wait); };
  return (Object.assign(fn, { clear })! as (() => void) & { clear: () => void });`,
		want: []int{0},
	}, {
		name: "shadowedLocalObjectAssignIsRefused",
		body: `const fn = () => callback();
  const Object = { assign: (target: () => void, source: object) => target };
  return Object.assign(fn, {});`,
		want: nil,
	}, {
		name: "arrayLiteralElements",
		body: `const fn = () => callback();
  const clear = () => { sink(wait); };
  return [fn, clear];`,
		want: []int{0, 1},
	}, {
		name: "arrayLiteralSpreadRefusesOnlyItsOwnSlot",
		body: `const fn = () => callback();
  return [...pool, fn];`,
		want: []int{0},
	}, {
		name: "objectLiteralPropertyValue",
		body: `const fn = () => callback();
  return { run: fn, wait };`,
		want: []int{0},
	}, {
		name: "objectLiteralMethod",
		body: `return { run() { callback(); }, later: () => { sink(wait); } };`,
		want: []int{0, 1},
	}, {
		name: "objectLiteralShorthandIsRefused",
		body: `const fn = () => callback();
  return { fn };`,
		want: nil,
	}, {
		name: "objectLiteralSpreadIsRefused",
		body: `return { ...pool };`,
		want: nil,
	}, {
		name: "constVariableIndirection",
		body: `const fn = () => callback();
  return fn;`,
		want: []int{0},
	}, {
		name: "functionDeclarationIndirectionIsUnchanged",
		body: `function fn() { callback(); }
  return fn;`,
		want: []int{0},
	}, {
		name: "reassignableBindingIsRefused",
		body: `let fn = () => callback();
  return fn;`,
		want: nil,
	}, {
		name: "destructuredBindingIsRefused",
		body: `const [fn] = [() => callback()];
  return fn;`,
		want: nil,
	}, {
		name: "unresolvedCallResultIsRefused",
		body: `const fn = () => callback();
  return wrap(fn);`,
		want: nil,
	}, {
		name: "conditionalExpressionIsRefused",
		body: `const fn = () => callback();
  const idle = () => {};
  return wait > 0 ? fn : idle;`,
		want: nil,
	}, {
		name: "elementAccessObjectAssignIsRefused",
		body: `const fn = () => callback();
  return Object["assign"](fn, {});`,
		want: nil,
	}, {
		// A hoisted function declaration binds a mutable variable. The
		// declaration proves what `fn` was, never what it holds at the return,
		// so any write to the binding withdraws the whole descent.
		name: "reassignedFunctionDeclarationIsRefused",
		body: `function fn() { callback(); }
  fn = () => { sink(wait); };
  return fn;`,
		want: nil,
	}, {
		name: "conditionallyReassignedFunctionDeclarationIsRefused",
		body: `function fn() { callback(); }
  if (wait > 0) { fn = () => { sink(wait); }; }
  return fn;`,
		want: nil,
	}, {
		// The write must be to *this* binding. A same-named local of an inner
		// scope is a different symbol and leaves the declaration provable.
		name: "shadowedInnerReassignmentLeavesTheDeclarationProvable",
		body: `function fn() { callback(); }
  function other() { let fn = () => {}; fn = () => { sink(wait); }; return fn; }
  sink(other);
  return fn;`,
		want: []int{0},
	}}
	for _, testCase := range cases {
		t.Run(testCase.name, func(t *testing.T) {
			source := `
declare function sink(value: unknown): void;
declare function wrap(value: () => void): () => void;
declare const pool: Array<() => void>;
function make(callback: () => void, wait: number) {
  ` + testCase.body + `
}
make(() => {}, 1);
`
			transcript := invocationTranscriptForMake(t, source)
			captures := returnedCaptures(t, transcript)
			if !slices.Equal(captures, testCase.want) {
				t.Fatalf(
					"returned captures = %v, want %v (returns=%#v)",
					captures, testCase.want, transcript.ControlFlow.Returns,
				)
			}
		})
	}
}

// TestObjectAssignIdentityRequiresTheUnaugmentedDefaultLibrarySymbol pins the two
// exactness sub-guards of isDefaultLibrarySymbolLocked that no other case
// reaches: the ObjectConstructor container check, and the requirement that
// *every* declaration of the symbol sit in a default library. A single
// `declare global` augmentation adds a user-file declaration to
// `ObjectConstructor.assign`, at which point the ES identity guarantee is no
// longer the one that was verified and the descent must contribute nothing.
func TestObjectAssignIdentityRequiresTheUnaugmentedDefaultLibrarySymbol(t *testing.T) {
	cases := []struct {
		name   string
		prefix string
		want   []int
	}{{
		name:   "unaugmentedObjectAssignDescends",
		prefix: `export {};`,
		want:   []int{0},
	}, {
		name: "globalObjectConstructorAugmentationIsRefused",
		prefix: `declare global { interface ObjectConstructor { assign<T>(target: T, extra: object): T; } }
export {};`,
		want: nil,
	}}
	for _, testCase := range cases {
		t.Run(testCase.name, func(t *testing.T) {
			source := `
declare function sink(value: unknown): void;
` + testCase.prefix + `
function make(callback: () => void, wait: number) {
  const fn = () => callback();
  return Object.assign(fn, {});
}
make(() => {}, 1);
`
			transcript := invocationTranscriptForMake(t, source)
			captures := returnedCaptures(t, transcript)
			if !slices.Equal(captures, testCase.want) {
				t.Fatalf(
					"returned captures = %v, want %v (returns=%#v)",
					captures, testCase.want, transcript.ControlFlow.Returns,
				)
			}
		})
	}
}

// TestCarriedCallablesBindContainmentNotMention pins F5: the fact a consumer
// joins against is the *range* of each carried callable, so a call made inside a
// closure the implementation never returns is outside every one of them, even
// when a returned closure names the same parameter.
func TestCarriedCallablesBindContainmentNotMention(t *testing.T) {
	source := `
declare function sink(value: unknown): void;
function make(callback: () => void, wait: number) {
  const orphan = () => { callback(); };
  const returned = () => { sink(callback); sink(wait); };
  sink(orphan);
  return returned;
}
make(() => {}, 1);
`
	transcript := invocationTranscriptForMake(t, source)
	var carried []typefacts.Location
	for _, site := range transcript.ControlFlow.Returns {
		carried = append(carried, site.CarriedCallables...)
	}
	if len(carried) != 1 {
		t.Fatalf("expected exactly the returned closure to be carried: %#v", carried)
	}
	// The orphan's `callback()` call is a direct-call parameter use; it must not
	// fall inside the carried range, while the returned closure's mentions do.
	orphanCall := strings.Index(source, "callback(); }")
	if orphanCall < 0 {
		t.Fatal("probe source lost its orphan call")
	}
	if carried[0].StartByte <= orphanCall && orphanCall < carried[0].EndByte {
		t.Fatalf("orphan call at %d is inside the carried range %#v", orphanCall, carried[0])
	}
	if got := returnedCaptures(t, transcript); !slices.Equal(got, []int{0, 1}) {
		t.Fatalf("returned closure mentions = %v, want [0 1]", got)
	}
}

// TestFallThroughConstructsKeepFollowingStatementsReachable pins S3: a loop, try,
// or switch that can only finish normally no longer reports every statement after
// it as reach:unknown, while every shape whose completion is not proven — a jump
// out, a swallowed throw, a loop with no exit edge and no break — keeps it.
func TestFallThroughConstructsKeepFollowingStatementsReachable(t *testing.T) {
	cases := []struct {
		name string
		body string
		want typefacts.Reachability
	}{{
		name: "forOfWithoutJumpsFallsThrough",
		body: `for (const key of keys) { sink(key); }`,
		want: typefacts.Reachable,
	}, {
		name: "forOfWithContinueFallsThrough",
		body: `for (const key of keys) { if (key) continue; sink(key); }`,
		want: typefacts.Reachable,
	}, {
		name: "nestedLoopBreakFallsThrough",
		body: `for (const key of keys) { for (const other of keys) { if (other) break; } }`,
		want: typefacts.Reachable,
	}, {
		name: "forOfWithReturnStaysUnknown",
		body: `for (const key of keys) { if (key) return; }`,
		want: typefacts.ReachUnknown,
	}, {
		name: "whileTrueWithoutBreakStaysUnknown",
		body: `while (true) { if (keys.length) sink(1); }`,
		want: typefacts.ReachUnknown,
	}, {
		name: "whileTrueWithBreakFallsThrough",
		body: `while (true) { if (keys.length) break; }`,
		want: typefacts.Reachable,
	}, {
		name: "forEverWithoutBreakStaysUnknown",
		body: `for (;;) { sink(1); }`,
		want: typefacts.ReachUnknown,
	}, {
		name: "forEverWithBreakFallsThrough",
		body: `for (;;) { if (keys.length) break; }`,
		want: typefacts.Reachable,
	}, {
		name: "doWhileTrueWithBreakFallsThrough",
		body: `do { if (keys.length) break; } while (true);`,
		want: typefacts.Reachable,
	}, {
		name: "whileWithOrdinaryConditionFallsThrough",
		body: `while (keys.length > 0) { sink(1); break; }`,
		want: typefacts.Reachable,
	}, {
		name: "tryCatchWithoutJumpsFallsThrough",
		body: `try { sink(1); } catch (error) { sink(error); }`,
		want: typefacts.Reachable,
	}, {
		name: "tryFinallyWithoutJumpsFallsThrough",
		body: `try { sink(1); } finally { sink(2); }`,
		want: typefacts.Reachable,
	}, {
		name: "swallowedThrowStaysUnknownConservatively",
		body: `try { throw new Error("x"); } catch (error) { sink(error); }`,
		want: typefacts.ReachUnknown,
	}, {
		name: "rethrowingCatchStaysUnknown",
		body: `try { sink(1); } catch (error) { throw error; }`,
		want: typefacts.ReachUnknown,
	}, {
		name: "switchWithBreaksFallsThrough",
		body: `switch (keys.length) { case 0: sink(0); break; default: sink(1); }`,
		want: typefacts.Reachable,
	}, {
		name: "labeledBreakLeavingTheConstructStaysUnknown",
		body: `outer: { for (const key of keys) { if (key) break outer; } sink(1); }`,
		want: typefacts.ReachUnknown,
	}, {
		name: "labeledBreakTargetingTheConstructFallsThrough",
		body: `outer: for (const key of keys) { for (const other of keys) { if (other) break outer; } }`,
		want: typefacts.Reachable,
	}, {
		name: "nestedCallableJumpsAreNotOurControlFlow",
		body: `for (const key of keys) { sink(() => { if (key) return 1; throw new Error("x"); }); }`,
		want: typefacts.Reachable,
	}, {
		// A loop that cannot end is a loop that cannot end wherever it sits.
		// Reading only the outer construct's own header answered "falls
		// through" for every one of these.
		name: "tryFinallyWrappingInfiniteLoopStaysUnknown",
		body: `try { while (true) { sink(1); } } finally { sink(2); }`,
		want: typefacts.ReachUnknown,
	}, {
		name: "nestedTryInfiniteInFinallyStaysUnknown",
		body: `try { sink(1); } finally { while (true) { sink(2); } }`,
		want: typefacts.ReachUnknown,
	}, {
		name: "tryWithNestedForEverStaysUnknown",
		body: `try { for (;;) { sink(1); } } catch (error) { sink(error); }`,
		want: typefacts.ReachUnknown,
	}, {
		name: "switchDefaultInfiniteLoopStaysUnknown",
		body: `switch (keys.length) { default: while (true) { sink(1); } }`,
		want: typefacts.ReachUnknown,
	}, {
		name: "forOfWrappingInfiniteLoopStaysUnknown",
		body: `for (const key of keys) { while (true) { sink(key); } }`,
		want: typefacts.ReachUnknown,
	}, {
		// A nested construct that leaves through a jump is not a trapped one.
		// The enclosing scan classifies that jump itself, so the precision of
		// the labeled-break case survives the nested descent.
		name: "nestedLoopLeavingThroughALabeledBreakStillFallsThrough",
		body: `outer: for (const key of keys) { while (true) { if (key) break outer; } }`,
		want: typefacts.Reachable,
	}, {
		// Always-truthy literal conditions have no exit edge either. `true` was
		// the only spelling the header test recognized.
		name: "whileOneStaysUnknown",
		body: `while (1) { sink(1); }`,
		want: typefacts.ReachUnknown,
	}, {
		name: "whileNotZeroStaysUnknown",
		body: `while (!0) { sink(1); }`,
		want: typefacts.ReachUnknown,
	}, {
		name: "whileNonEmptyStringStaysUnknown",
		body: `while ("go") { sink(1); }`,
		want: typefacts.ReachUnknown,
	}, {
		name: "doWhileOneWithBreakFallsThrough",
		body: `do { if (keys.length) break; } while (1);`,
		want: typefacts.Reachable,
	}, {
		// A falsy literal is decidable too, and decidedly *has* an exit edge.
		name: "whileZeroFallsThrough",
		body: `while (0) { sink(1); }`,
		want: typefacts.Reachable,
	}, {
		// Truthiness of a non-literal is not read here at all: the condition
		// keeps its exit edge, which is the conservative direction for this
		// construct even though the loop is in fact endless.
		name: "nonLiteralTruthyConditionKeepsItsExitEdge",
		body: `while (keys) { sink(1); }`,
		want: typefacts.Reachable,
	}}
	for _, testCase := range cases {
		t.Run(testCase.name, func(t *testing.T) {
			source := `
declare function sink(value: unknown): void;
declare const keys: string[];
function make(callback: () => void) {
  ` + testCase.body + `
  return () => callback();
}
make(() => {});
`
			flow := controlFlowCensusForMake(t, source)
			last := lastReturnSite(t, flow)
			if last.Reach != testCase.want {
				t.Fatalf("trailing return reach = %q, want %q (flow=%#v)", last.Reach, testCase.want, flow)
			}
			// Reachability after the construct is a strictly separate claim from
			// reachability inside it. The marker must survive so the control-flow
			// domain stays open and no consumer reads interior certainty here.
			if len(flow.Unsupported) == 0 {
				t.Fatalf("fall-through construct dropped its unsupported marker: %#v", flow)
			}
		})
	}
}

// TestFallThroughReachabilityNeverPromotesAnUnreachableConstruct pins the one-way
// direction of S3: a construct entered from an already-unreachable point never
// makes the statement after it reachable.
func TestFallThroughReachabilityNeverPromotesAnUnreachableConstruct(t *testing.T) {
	source := `
declare function sink(value: unknown): void;
declare const keys: string[];
function make(callback: () => void) {
  throw new Error("stop");
  for (const key of keys) { sink(key); }
  return () => callback();
}
make(() => {});
`
	flow := controlFlowCensusForMake(t, source)
	if last := lastReturnSite(t, flow); last.Reach == typefacts.Reachable {
		t.Fatalf("return after an unreachable construct was promoted to reachable: %#v", flow)
	}
}

func controlFlowCensusForMake(t *testing.T, source string) *typefacts.ControlFlowCensus {
	t.Helper()
	return invocationTranscriptForMake(t, source).ControlFlow
}

func invocationTranscriptForMake(t *testing.T, source string) typefacts.InvocationTranscript {
	t.Helper()
	dir := t.TempDir()
	writeInvocationProject(t, dir, map[string]string{"facts.ts": source})
	analyzer, closeProject := openInvocationAnalyzer(t, dir)
	defer closeProject()
	path := filepath.Join(dir, "facts.ts")
	needle := source[strings.LastIndex(source, "make(") : len(source)-2]
	answer, err := analyzer.InvocationTranscripts(
		context.Background(),
		invocationDemandsForNeedles(path, source, []string{needle}, true),
	)
	if err != nil {
		t.Fatal(err)
	}
	transcript := answer.Transcripts[0]
	if transcript.ControlFlow == nil {
		t.Fatalf("no control-flow census for %q: %#v", needle, transcript)
	}
	return transcript
}

// returnedCaptures answers the historical "which parameters do the returned
// callables close over" question as a *join* of two published facts: the
// parameter-use census, whose locations are symbol-resolved, and the carried
// callable ranges of every return site. The producer no longer reports the union
// directly, because a union cannot say which callable did the mentioning; the
// join reconstructs it here for tests that are about the descent whitelist
// rather than about containment.
func returnedCaptures(
	t *testing.T,
	transcript typefacts.InvocationTranscript,
) []int {
	t.Helper()
	var indices []int
	for _, use := range transcript.ParameterUses {
		for _, site := range transcript.ControlFlow.Returns {
			if locationWithinAny(use.Location, site.CarriedCallables) {
				indices = append(indices, use.ParameterIndex)
				break
			}
		}
	}
	slices.Sort(indices)
	return slices.Compact(indices)
}

func locationWithinAny(inner typefacts.Location, outers []typefacts.Location) bool {
	for _, outer := range outers {
		if outer.Path == inner.Path &&
			outer.StartByte <= inner.StartByte && inner.EndByte <= outer.EndByte {
			return true
		}
	}
	return false
}

func lastReturnSite(t *testing.T, flow *typefacts.ControlFlowCensus) typefacts.ReturnSite {
	t.Helper()
	if len(flow.Returns) == 0 {
		t.Fatalf("control-flow census has no return sites: %#v", flow)
	}
	last := flow.Returns[0]
	for _, site := range flow.Returns[1:] {
		if site.Location.StartByte > last.Location.StartByte {
			last = site
		}
	}
	return last
}

func TestFunctionPrototypeCallApplyAndBindRefuseTheWrapperSignature(t *testing.T) {
	dir := t.TempDir()
	source := `
function target(value: string, count: number): boolean { return value.length === count; }
target.call(undefined, "x", 1);
target.apply(undefined, ["x", 1]);
target.bind(undefined, "x");
`
	writeInvocationProject(t, dir, map[string]string{"facts.ts": source})
	analyzer, closeProject := openInvocationAnalyzer(t, dir)
	defer closeProject()
	path := filepath.Join(dir, "facts.ts")
	answer, err := analyzer.InvocationTranscripts(
		context.Background(),
		invocationDemandsForNeedles(path, source, []string{
			`target.call(undefined, "x", 1)`,
			`target.apply(undefined, ["x", 1])`,
			`target.bind(undefined, "x")`,
		}, false),
	)
	if err != nil {
		t.Fatal(err)
	}
	for index, transcript := range answer.Transcripts {
		if transcript.SelectedSignature != nil || len(transcript.Completeness) != 0 ||
			!slices.Contains(transcript.OpenReasons, "indirectFunctionInvocation") {
			t.Fatalf("indirect invocation %d certified the wrapper signature: %#v", index, transcript)
		}
	}
}

func TestInvocationCensusPreservesDestructuredBindingPaths(t *testing.T) {
	dir := t.TempDir()
	source := `
function inspect({ nested: { callback }, pair: [, second] }: {
  nested: { callback: () => void };
  pair: [number, () => void];
}) {
  callback();
  second();
}
inspect({ nested: { callback() {} }, pair: [1, () => {}] });
`
	writeInvocationProject(t, dir, map[string]string{"facts.ts": source})
	analyzer, closeProject := openInvocationAnalyzer(t, dir)
	defer closeProject()
	path := filepath.Join(dir, "facts.ts")
	answer, err := analyzer.InvocationTranscripts(
		context.Background(),
		invocationDemandsForNeedles(path, source, []string{
			`inspect({ nested: { callback() {} }, pair: [1, () => {}] })`,
		}, true),
	)
	if err != nil {
		t.Fatal(err)
	}
	uses := answer.Transcripts[0].ParameterUses
	var callbackPath, secondPath bool
	for _, use := range uses {
		callbackPath = callbackPath || pathNamesEqual(use.BindingPath, "nested", "callback")
		secondPath = secondPath || len(use.BindingPath) == 2 &&
			use.BindingPath[0].Property == "pair" && use.BindingPath[1].Index != nil &&
			*use.BindingPath[1].Index == 1
	}
	if !callbackPath || !secondPath {
		t.Fatalf("destructured binding paths = %#v", uses)
	}
}

func TestInvocationValidityMatchesPublishedTypeScriptForOverloadAndSpreadCases(t *testing.T) {
	repositoryRoot, err := filepath.Abs(filepath.Join("..", "..", "..", "..", ".."))
	if err != nil {
		t.Fatal(err)
	}
	tsc := filepath.Join(repositoryRoot, "packages", "cli", "node_modules", ".bin", "tsc")
	if _, err := os.Stat(tsc); err != nil {
		t.Fatalf("published TypeScript oracle is unavailable at %s: %v", tsc, err)
	}
	cases := []struct {
		name  string
		call  string
		valid bool
	}{
		{"selected string overload", `choose("x")`, true},
		{"selected number overload", `choose(1)`, true},
		{"exact tuple spread", `rest(...(["x", 1] as const))`, true},
		{"unknown array to rest", `rest(...openValues)`, true},
		{"invalid overload", `choose(true)`, false},
	}
	for _, testCase := range cases {
		t.Run(testCase.name, func(t *testing.T) {
			dir := t.TempDir()
			source := `
declare function choose(value: string): string;
declare function choose(value: number): number;
declare function rest(...values: Array<string | number>): void;
declare const openValues: Array<string | number>;
` + testCase.call + `;
`
			writeInvocationProject(t, dir, map[string]string{"facts.ts": source})
			command := exec.Command(tsc, "--noEmit", "--strict", "--skipLibCheck", "--target", "esnext", filepath.Join(dir, "facts.ts"))
			output, tscErr := command.CombinedOutput()
			if (tscErr == nil) != testCase.valid {
				t.Fatalf("published tsc validity = %v, want %v: %s", tscErr == nil, testCase.valid, output)
			}
			analyzer, closeProject := openInvocationAnalyzer(t, dir)
			defer closeProject()
			path := filepath.Join(dir, "facts.ts")
			answer, err := analyzer.InvocationTranscripts(
				context.Background(),
				invocationDemandsForNeedles(path, source, []string{testCase.call}, false),
			)
			if err != nil {
				t.Fatal(err)
			}
			got := answer.Transcripts[0].Validity == typefacts.ResolvedCallValid
			if got != testCase.valid {
				t.Fatalf("Type Facts validity = %v, published tsc = %v; transcript=%#v", got, testCase.valid, answer.Transcripts[0])
			}
		})
	}
}

func writeInvocationProject(t *testing.T, dir string, files map[string]string) {
	t.Helper()
	if err := os.WriteFile(
		filepath.Join(dir, "tsconfig.json"),
		[]byte(`{"compilerOptions":{"strict":true,"module":"esnext","target":"esnext","moduleResolution":"bundler"},"include":["*.ts"]}`),
		0o644,
	); err != nil {
		t.Fatal(err)
	}
	for name, source := range files {
		if err := os.WriteFile(filepath.Join(dir, name), []byte(source), 0o644); err != nil {
			t.Fatal(err)
		}
	}
}

func openInvocationAnalyzer(t *testing.T, dir string) (typefacts.InvocationAnalyzer, func()) {
	t.Helper()
	opened, err := OpenProject(context.Background(), filepath.Join(dir, "tsconfig.json"), nil)
	if err != nil {
		t.Fatal(err)
	}
	analyzer, ok := opened.(typefacts.InvocationAnalyzer)
	if !ok {
		_ = opened.Close()
		t.Fatal("TypeScript-Go project does not implement InvocationAnalyzer")
	}
	return analyzer, func() { _ = opened.Close() }
}

func invocationDemandsForNeedles(path, source string, needles []string, census bool) []typefacts.InvocationDemand {
	demands := make([]typefacts.InvocationDemand, len(needles))
	for index, needle := range needles {
		start := strings.Index(source, needle)
		demands[index] = typefacts.InvocationDemand{
			Location:      typefacts.Location{Path: path, StartByte: start, EndByte: start + len(needle)},
			CallableDepth: 3,
			Census:        census,
		}
	}
	return demands
}

func pathNamesEqual(path []typefacts.PathSegment, names ...string) bool {
	if len(path) != len(names) {
		return false
	}
	for index := range names {
		if path[index].Property != names[index] {
			return false
		}
	}
	return true
}

func hasCallablePath(paths []typefacts.CallablePathFact, names ...string) bool {
	for _, path := range paths {
		if len(path.Path) != len(names) {
			continue
		}
		matches := true
		for index := range names {
			if path.Path[index].Property != names[index] {
				matches = false
				break
			}
		}
		if matches && path.Callability == typefacts.CallabilityCallable {
			return true
		}
	}
	return false
}

func hasFinitePartition(partitions []typefacts.FinitePartition, axis typefacts.FinitePartitionAxis) bool {
	for _, partition := range partitions {
		if partition.Axis == axis && partition.Complete {
			return true
		}
	}
	return false
}
