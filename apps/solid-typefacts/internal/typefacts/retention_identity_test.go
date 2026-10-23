package typefacts_test

import (
	"context"
	"encoding/json"
	"fmt"
	"path/filepath"
	"sort"
	"strings"
	"testing"

	"github.com/yumemi-thomas/solid-checker/apps/solid-typefacts/internal/typefacts"
	"github.com/yumemi-thomas/solid-checker/apps/solid-typefacts/internal/typefacts/tsgo"
)

// Fresh-versus-incremental oracles for durable-id retention. A retained
// session is driven through scripted and seeded random edits, and every
// generation is compared with a session that replays the same updates and
// materializes the whole batch with nothing retained. Mirrors the Rust
// producer's randomized_incremental tests.

const strictConfig = `{"compilerOptions":{"module":"esnext","moduleResolution":"bundler","target":"esnext","strict":true},"include":["*.ts"]}`

func isIdentByte(b byte) bool {
	return b == '_' || (b >= '0' && b <= '9') || (b >= 'a' && b <= 'z') || (b >= 'A' && b <= 'Z')
}

type demandFamilies int

const (
	symbolsOnly demandFamilies = iota
	allFamilies
	withReferenceSpace
)

// textDemands derives demands from the source text alone, so that deriving
// them mints no identity: every identifier that follows a declaration keyword
// or a dot, or precedes a call's parenthesis, asks for its symbol and
// references (and, per families, more).
func textDemands(t testing.TB, closure *typefacts.DemandClosure, families demandFamilies) []typefacts.EntityDemand {
	t.Helper()
	sources, err := closure.SourceFiles(context.Background())
	if err != nil {
		t.Fatal(err)
	}
	var demands []typefacts.EntityDemand
	for _, source := range sources {
		if strings.Contains(source.Path, "/node_modules/") || strings.HasSuffix(source.Path, ".d.ts") ||
			!(strings.HasSuffix(source.Path, ".ts") || strings.HasSuffix(source.Path, ".tsx")) {
			continue
		}
		path := filepath.Clean(source.Path)
		bytes := source.Source
		seen := map[[2]int]struct{}{}
		for index := 0; index < len(bytes); {
			if !isIdentByte(bytes[index]) || (index > 0 && isIdentByte(bytes[index-1])) {
				index++
				continue
			}
			start := index
			for index < len(bytes) && isIdentByte(bytes[index]) {
				index++
			}
			word := bytes[start:index]
			after := byte(0)
			if index < len(bytes) {
				after = bytes[index]
			}
			before := start - 1
			for before >= 0 && (bytes[before] == ' ' || bytes[before] == '\n' || bytes[before] == '\t' || bytes[before] == '\r') {
				before--
			}
			previousWord := ""
			if before >= 0 {
				from := before + 1
				for from > 0 && isIdentByte(bytes[from-1]) {
					from--
				}
				previousWord = string(bytes[from : before+1])
			}
			declares := false
			switch previousWord {
			case "const", "let", "var", "function", "class", "interface", "type":
				declares = true
			}
			member := before >= 0 && bytes[before] == '.'
			calls := after == '('
			numeric := word[0] >= '0' && word[0] <= '9'
			key := [2]int{start, index}
			if _, dup := seen[key]; dup || numeric || !(declares || member || calls) {
				continue
			}
			seen[key] = struct{}{}
			demand := typefacts.EntityDemand{
				Location:   typefacts.Location{Path: path, StartByte: start, EndByte: index},
				Symbol:     true,
				References: true,
			}
			if families != symbolsOnly {
				demand.TypeDescriptor = true
				demand.ResolvedCall = true
				demand.Callability = true
				demand.RuntimeValueDomain = true
			}
			if families == withReferenceSpace {
				demand.ReferenceSpace = true
			}
			demands = append(demands, demand)
		}
	}
	return demands
}

type retentionSession struct {
	closure *typefacts.DemandClosure
	backend typefacts.Project
}

func openRetentionSession(t testing.TB, projectID string) *retentionSession {
	t.Helper()
	backend, err := tsgo.OpenProject(context.Background(), projectID, nil)
	if err != nil {
		t.Fatal(err)
	}
	closure, err := typefacts.NewDemandClosure(backend, nil)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = closure.Close() })
	return &retentionSession{closure: closure, backend: backend}
}

// describeEntities is one line per entity row, in table order.
func describeEntities(table *typefacts.FactTable) []string {
	var lines []string
	for _, entity := range typefacts.WireEntitiesForTest(table) {
		encoded, err := json.Marshal(entity)
		if err != nil {
			panic(err)
		}
		lines = append(lines, string(encoded))
	}
	return lines
}

// counterRenamer renames every generation-scoped counter id
// (symbol:<generation>:<n>) by first appearance, so two tables that name the
// same partition of symbols with differently numbered counters compare equal.
// Declaration-hash ids are left alone: those must match exactly.
type counterRenamer map[string]int

func (r counterRenamer) apply(text string) string {
	var out strings.Builder
	rest := text
	for {
		at := strings.Index(rest, "symbol:")
		if at < 0 {
			out.WriteString(rest)
			return out.String()
		}
		out.WriteString(rest[:at])
		tail := rest[at:]
		end := 0
		for end < len(tail) && (isIdentByteNoUnderscore(tail[end]) || tail[end] == ':') {
			end++
		}
		id := tail[:end]
		counter := true
		parts := strings.Split(strings.TrimPrefix(id, "symbol:"), ":")
		for _, part := range parts {
			if part == "" {
				counter = false
			}
			for i := 0; i < len(part); i++ {
				if part[i] < '0' || part[i] > '9' {
					counter = false
				}
			}
		}
		if counter {
			number, ok := r[id]
			if !ok {
				number = len(r) + 1
				r[id] = number
			}
			fmt.Fprintf(&out, "counter#%d", number)
		} else {
			out.WriteString(id)
		}
		rest = tail[end:]
	}
}

func isIdentByteNoUnderscore(b byte) bool {
	return isIdentByte(b) && b != '_'
}

func renameCounters(lines []string) []string {
	renamer := counterRenamer{}
	out := make([]string, len(lines))
	for i, line := range lines {
		out[i] = renamer.apply(line)
	}
	return out
}

func reportDifference(what string, retained, fresh []string) string {
	if strings.Join(retained, "\n") == strings.Join(fresh, "\n") {
		return ""
	}
	var report strings.Builder
	fmt.Fprintf(&report, "%s differs (%d retained rows, %d fresh rows)\n", what, len(retained), len(fresh))
	contains := func(lines []string, line string) bool {
		for _, other := range lines {
			if other == line {
				return true
			}
		}
		return false
	}
	shown := 0
	for _, line := range retained {
		if !contains(fresh, line) && shown < 4 {
			fmt.Fprintf(&report, "  retained only: %.700s\n", line)
			shown++
		}
	}
	shown = 0
	for _, line := range fresh {
		if !contains(retained, line) && shown < 4 {
			fmt.Fprintf(&report, "  fresh only:    %.700s\n", line)
			shown++
		}
	}
	return report.String()
}

// assertScriptMatchesFresh drives changes through a retained closure and
// compares every generation with a replayed whole-batch one.
func assertScriptMatchesFresh(
	t *testing.T,
	files map[string]string,
	script func(root string) []typefacts.FileChange,
	families demandFamilies,
) {
	t.Helper()
	assertScriptMatchesFreshWith(t, files, script, families, false)
}

func assertScriptMatchesFreshWith(
	t *testing.T,
	files map[string]string,
	script func(root string) []typefacts.FileChange,
	families demandFamilies,
	renameCounterIDs bool,
) {
	t.Helper()
	ctx := context.Background()
	root, _ := writeAdversarialProject(t, files)
	projectID := filepath.Join(root, "tsconfig.json")
	changes := script(root)

	incremental := openRetentionSession(t, projectID).closure
	initial := textDemands(t, incremental, families)
	if _, err := incremental.DemandTableForGroups(ctx, 1, groupedDemands(initial), demandPaths(initial)); err != nil {
		t.Fatal(err)
	}
	for step, change := range changes {
		if _, err := incremental.Update(ctx, []typefacts.FileChange{change}); err != nil {
			t.Fatalf("incremental update %d: %v", step, err)
		}
		generation := uint64(step + 2)
		demands := textDemands(t, incremental, families)
		retained, err := incremental.DemandTableForGroups(ctx, generation, groupedDemands(demands), nil)
		if err != nil {
			t.Fatalf("incremental materialization %d: %v", step, err)
		}
		fresh := openRetentionSession(t, projectID).closure
		for replay := 0; replay <= step; replay++ {
			if _, err := fresh.Update(ctx, []typefacts.FileChange{changes[replay]}); err != nil {
				t.Fatalf("fresh replay %d/%d: %v", replay, step, err)
			}
		}
		freshDemands := textDemands(t, fresh, families)
		whole, err := fresh.DemandTableForGroups(ctx, generation, canonicalDemandGroups(freshDemands), nil)
		if err != nil {
			t.Fatalf("fresh materialization %d: %v", step, err)
		}
		left, right := describeEntities(retained), describeEntities(whole)
		if renameCounterIDs {
			left, right = renameCounters(left), renameCounters(right)
		}
		if report := reportDifference("entity rows", left, right); report != "" {
			t.Fatalf("step %d: retained table differs from fresh\n%s", step, report)
		}
		if !renameCounterIDs {
			assertFullWireTransitionsIdentical(t, "retained table", step, projectID, retained, whole)
		}
	}
}

func contentChange(root, name string, version uint64, source string) typefacts.FileChange {
	return typefacts.FileChange{Path: filepath.Join(root, name), Version: version, Source: []byte(source)}
}

func counterEdits(root string) []typefacts.FileChange {
	var changes []typefacts.FileChange
	for version := uint64(1); version <= 3; version++ {
		changes = append(changes, contentChange(root, "c.ts", version, fmt.Sprintf("export const c = %d;\n", version)))
	}
	return changes
}

// Two files use different instantiations of one generic member: the first
// mint claims the declaration's durable id, the other gets a counter. An edit
// to a third file must not let the second claim the id the first retained.
func TestInstantiationsOfOneMemberKeepDistinctIdentitiesAcrossEdits(t *testing.T) {
	files := map[string]string{
		"tsconfig.json": strictConfig,
		"a.ts":          "declare const xs: string[];\nexport const ma = xs.map((x) => x);\n",
		"b.ts":          "declare const ys: number[];\nexport const mb = ys.map((y) => y);\n",
		"c.ts":          "export const c = 1;\n",
	}
	assertScriptMatchesFresh(t, files, counterEdits, allFamilies)
}

func TestSymbolOnlyInstantiationsKeepDistinctIdentitiesAcrossEdits(t *testing.T) {
	files := map[string]string{
		"tsconfig.json": strictConfig,
		"a.ts":          "declare const xs: string[];\nexport const ma = xs.map((x) => x);\n",
		"b.ts":          "declare const ys: number[];\nexport const mb = ys.map((y) => y);\n",
		"c.ts":          "export const c = 1;\n",
	}
	assertScriptMatchesFresh(t, files, counterEdits, symbolsOnly)
}

// A mapped type over a namespace object copies its functions into transient
// members that share the originals' declarations. Another file holds an
// original's id from before the spread appeared; the retained answer must not
// be reused with the copy claiming the same id.
func TestMappedCopiesOfNamespaceFunctionsDoNotShareTheOriginalID(t *testing.T) {
	files := map[string]string{
		"tsconfig.json": strictConfig,
		"ns.ts":         "export function f(): number { return 1; }\n",
		"a.ts":          "import { f } from \"./ns\";\nexport const x = f();\n",
		"b.ts":          "import * as ns from \"./ns\";\nexport const b = ns;\n",
		"z.ts":          "export const z = 1;\n",
	}
	assertScriptMatchesFresh(t, files, func(root string) []typefacts.FileChange {
		return []typefacts.FileChange{
			contentChange(root, "b.ts", 1, "import * as ns from \"./ns\";\nexport const b: Readonly<typeof ns> = ns;\nexport const c = b.f();\n"),
			contentChange(root, "z.ts", 2, "export const z = 2;\n"),
			contentChange(root, "z.ts", 3, "export const z = 3;\n"),
		}
	}, symbolsOnly)
}

// A tsconfig change can change any file's facts, and no import edge leads from
// the configuration to the files it governs.
func TestConfigToggleDoesNotKeepRetainedContributions(t *testing.T) {
	loose := `{"compilerOptions":{"module":"esnext","moduleResolution":"bundler","target":"esnext","strict":false},"include":["*.ts"]}`
	files := map[string]string{
		"tsconfig.json": strictConfig,
		"a.ts":          "declare const xs: string[] | undefined;\nexport const first = xs?.[0];\nexport const n = xs ? xs.length : 0;\n",
		"b.ts":          "export function id<T>(x: T): T { return x; }\nexport const v = id(null);\n",
		"c.ts":          "export const c = 1;\n",
	}
	assertScriptMatchesFresh(t, files, func(root string) []typefacts.FileChange {
		return []typefacts.FileChange{
			contentChange(root, "tsconfig.json", 1, loose),
			contentChange(root, "tsconfig.json", 2, strictConfig),
			contentChange(root, "c.ts", 3, "export const c = 2;\n"),
		}
	}, allFamilies)
}

var _ = sort.Strings

// The reference space of a symbol is read from the retained per-file
// reference entries. A span-shifting edit must not change what a fresh open
// answers.
func TestReferenceSpaceAfterSpanShiftMatchesFresh(t *testing.T) {
	projects := map[string]map[string]string{
		"generic members": {
			"tsconfig.json": strictConfig,
			"a.ts":          "declare const xs: string[];\nexport const ma = xs.map((x) => x);\n",
			"b.ts":          "declare const ys: number[];\nexport const mb = ys.map((y) => y);\n",
			"c.ts":          "export const c = 1;\n",
		},
		"imports": {
			"tsconfig.json": strictConfig,
			"lib.ts":        "export interface Box<T> { get(): T }\nexport class Cell<T> { v!: T; get(): T { return this.v; } }\nexport function make<T>(v: T): Cell<T> { const c = new Cell<T>(); c.v = v; return c; }\n",
			"a.ts":          "import { Cell, make, type Box } from \"./lib\";\nexport const a: Box<string> = make(\"s\");\nexport const ac = new Cell<number>().get();\n",
			"b.ts":          "import { Cell, make } from \"./lib\";\nexport const b = make(1).get();\nexport const bc = new Cell<string>().get();\n",
		},
	}
	for name, files := range projects {
		for _, target := range []string{"a.ts", "b.ts", "c.ts", "lib.ts"} {
			source, ok := files[target]
			if !ok {
				continue
			}
			t.Run(name+"/"+target, func(t *testing.T) {
				assertScriptMatchesFreshWith(t, files, func(root string) []typefacts.FileChange {
					return []typefacts.FileChange{
						contentChange(root, target, 1, "\n"+source),
						contentChange(root, target, 2, source),
					}
				}, withReferenceSpace, true)
			})
		}
	}
}

// Within one generation the ids a table carries must not depend on the order
// the demands arrived in: a demand set that grows (b.ts joins) is answered as
// a fresh closure answers the whole set, durable ids exactly. Mirrors the Rust
// producer's ids_within_a_generation_do_not_depend_on_demand_order.
func TestIDsWithinAGenerationDoNotDependOnDemandOrder(t *testing.T) {
	ctx := context.Background()
	files := map[string]string{
		"tsconfig.json": strictConfig,
		"a.ts":          "declare const xs: string[];\nexport const ma = xs.map((x) => x);\n",
		"b.ts":          "declare const ys: number[];\nexport const mb = ys.map((y) => y);\n",
		"c.ts":          "export const c = 1;\n",
	}
	root, _ := writeAdversarialProject(t, files)
	projectID := filepath.Join(root, "tsconfig.json")
	for _, families := range []demandFamilies{allFamilies, symbolsOnly} {
		for _, first := range []string{"a.ts", "b.ts"} {
			t.Run(fmt.Sprintf("families%d/%s", families, first), func(t *testing.T) {
				fresh := openRetentionSession(t, projectID).closure
				all := textDemands(t, fresh, families)
				whole, err := fresh.DemandTableForGroups(ctx, 1, canonicalDemandGroups(all), nil)
				if err != nil {
					t.Fatal(err)
				}
				want := renameCounters(describeEntities(whole))

				growing := openRetentionSession(t, projectID).closure
				all = textDemands(t, growing, families)
				firstPath := filepath.Join(root, first)
				var only, rest []typefacts.EntityDemand
				for _, demand := range all {
					if filepath.Clean(demand.Location.Path) == filepath.Clean(firstPath) {
						only = append(only, demand)
					} else {
						rest = append(rest, demand)
					}
				}
				if _, err := growing.DemandTableForGroups(ctx, 1, groupedDemands(only), demandPaths(only)); err != nil {
					t.Fatal(err)
				}
				grown, err := growing.DemandTableForGroups(ctx, 1, groupedDemands(all), demandPaths(rest))
				if err != nil {
					t.Fatal(err)
				}
				got := renameCounters(describeEntities(grown))
				if report := reportDifference("entity rows", got, want); report != "" {
					t.Fatalf("a demand set that grew within the generation differs from a fresh whole batch\n%s", report)
				}

				reversed := openRetentionSession(t, projectID).closure
				rev := textDemands(t, reversed, families)
				for i, j := 0, len(rev)-1; i < j; i, j = i+1, j-1 {
					rev[i], rev[j] = rev[j], rev[i]
				}
				table, err := reversed.DemandTableForGroups(ctx, 1, groupedDemands(rev), nil)
				if err != nil {
					t.Fatal(err)
				}
				rows := describeEntities(table)
				expected := describeEntities(whole)
				sort.Strings(rows)
				sort.Strings(expected)
				if report := reportDifference("reversed-order rows", renameCounters(rows), renameCounters(expected)); report != "" {
					t.Fatalf("demand order changed the ids of a generation\n%s", report)
				}
			})
		}
	}
}
