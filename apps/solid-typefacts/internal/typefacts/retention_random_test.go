package typefacts_test

import (
	"context"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"sort"
	"strconv"
	"strings"
	"testing"
	"unicode"

	"github.com/yumemi-thomas/solid-checker/apps/solid-typefacts/internal/typefacts"
)

// Seeded random edit sequences. A retained session follows the sequence; after
// every edit each fact family is compared with a session that replays the same
// updates and materializes the whole batch with nothing retained. The edit
// kinds and worlds mirror the Rust producer's randomized_incremental tests.

// rng is xorshift64*: deterministic, dependency-free.
type rng struct{ state uint64 }

func newRng(seed uint64) *rng { return &rng{state: (seed * 0x9e3779b97f4a7c15) | 1} }

func (r *rng) next() uint64 {
	r.state ^= r.state >> 12
	r.state ^= r.state << 25
	r.state ^= r.state >> 27
	return r.state * 0x2545f4914f6cdd1d
}

func (r *rng) below(bound int) int { return int(r.next() % uint64(bound)) }

func (r *rng) chance(percent int) bool { return r.below(100) < percent }

func pick[T any](r *rng, items []T) T { return items[r.below(len(items))] }

type export struct{ module, name, kind string }

// world is a project of independent statement lines per module, so an edit is
// a line edit and the project is always a valid compilation unit (type errors
// are fine; the producer must stay exact on broken code too).
type world struct {
	// files maps a file name to its statement lines. Script files (g*.ts) have
	// no imports or exports and join the global scope.
	files map[string][]string
	// graveyard holds files deleted earlier, restorable with their last content.
	graveyard map[string][]string
	config    int
	counter   int
	// plain restricts statements to constructs whose ids no other symbol can
	// claim (consts, plain functions, their uses), so answers stay retainable
	// and the retention logic is exercised instead of bypassed.
	plain bool
}

var randomConfigs = []string{
	strictConfig,
	`{"compilerOptions":{"module":"esnext","moduleResolution":"bundler","target":"esnext","strict":false},"include":["*.ts"]}`,
	`{"compilerOptions":{"module":"esnext","moduleResolution":"bundler","target":"es2017","strict":true,"noUncheckedIndexedAccess":true},"include":["*.ts"]}`,
	`{"compilerOptions":{"module":"esnext","moduleResolution":"bundler","target":"esnext","strict":true},"include":["m*.ts","g*.ts"]}`,
}

var randomTypes = []string{"number", "string", "number[]", "string[]", "boolean"}

func sortedKeys(files map[string][]string) []string {
	keys := make([]string, 0, len(files))
	for key := range files {
		keys = append(keys, key)
	}
	sort.Strings(keys)
	return keys
}

func render(lines []string) string { return strings.Join(lines, "\n") + "\n" }

func isAlnum(c rune) bool { return unicode.IsLetter(c) || unicode.IsDigit(c) }

func leadingWord(text string) string {
	end := 0
	for end < len(text) && isAlnum(rune(text[end])) {
		end++
	}
	return text[:end]
}

func (w *world) freshName(prefix string) string {
	w.counter++
	return fmt.Sprintf("%s%d", prefix, w.counter)
}

func (w *world) moduleNames() []string {
	var names []string
	for _, name := range sortedKeys(w.files) {
		if strings.HasPrefix(name, "m") {
			names = append(names, name)
		}
	}
	return names
}

// exports is (module, exported name, kind) for every export of every module.
func (w *world) exports() []export {
	var found []export
	keywords := []struct{ prefix, kind string }{
		{"export const ", "const"},
		{"export function ", "function"},
		{"export async function ", "function"},
		{"export class ", "class"},
		{"export interface ", "interface"},
	}
	for _, file := range sortedKeys(w.files) {
		if !strings.HasPrefix(file, "m") {
			continue
		}
		for _, line := range w.files[file] {
			for _, keyword := range keywords {
				rest, ok := strings.CutPrefix(line, keyword.prefix)
				if !ok {
					continue
				}
				name := leadingWord(rest)
				generic := strings.HasPrefix(rest[len(name):], "<")
				kind := keyword.kind
				switch {
				case kind == "function" && generic:
					kind = "gfunction"
				case kind == "class" && generic:
					kind = "gclass"
				case kind == "interface" && generic:
					kind = "ginterface"
				}
				found = append(found, export{strings.TrimSuffix(file, ".ts"), name, kind})
			}
		}
	}
	return found
}

func (w *world) otherExports(owner string) []export {
	var out []export
	for _, e := range w.exports() {
		if e.module+".ts" != owner {
			out = append(out, e)
		}
	}
	return out
}

func useStatement(r *rng, name, target, kind string) string {
	switch kind {
	case "function":
		return fmt.Sprintf("export const %s = %s(%d);", name, target, r.below(9))
	case "gfunction":
		return fmt.Sprintf("export const %s = %s(%s);", name, target, pick(r, []string{"1", `"s"`, "[1]"}))
	case "gclass":
		return fmt.Sprintf("export const %s = new %s<%s>().get();", name, target, pick(r, []string{"number", "string"}))
	case "class":
		return fmt.Sprintf("export const %s = new %s().method(%d);", name, target, r.below(9))
	case "ginterface":
		return fmt.Sprintf("export const %s: %s<string> | undefined = undefined;", name, target)
	case "interface":
		return fmt.Sprintf("export const %s: %s | undefined = undefined;", name, target)
	}
	return fmt.Sprintf("export const %s = %s;", name, target)
}

func (w *world) randomStatement(r *rng, owner string) string {
	name := w.freshName("v")
	exports := w.otherExports(owner)
	ty := pick(r, randomTypes)
	selector := 0
	if w.plain {
		selector = []int{0, 1, 2, 3, 4, 99, 99, 99}[r.below(8)]
	} else {
		selector = r.below(24)
	}
	switch selector {
	case 0:
		return fmt.Sprintf("export const %s = %d;", name, r.below(1000))
	case 1:
		return fmt.Sprintf("export const %s = \"s%d\";", name, r.below(100))
	case 2:
		return fmt.Sprintf("export const %s = [%d, %d, %d];", name, r.below(9), r.below(9), r.below(9))
	case 3:
		return fmt.Sprintf("export const %s = { a: %d, b: \"x\" };", name, r.below(9))
	case 4:
		return fmt.Sprintf("export function %s(a: %s): %s { return a; }", name, ty, ty)
	case 5:
		return fmt.Sprintf("export async function %s(): Promise<number> { await Promise.resolve(%d); return %d; }", name, r.below(9), r.below(9))
	case 6:
		return fmt.Sprintf("export class %s<T> { v!: T; get(): T { return this.v; } }", name)
	case 7:
		return fmt.Sprintf("export interface %s<T> { get(): T }", name)
	case 8:
		return fmt.Sprintf("export const %s = [1, 2, 3].map((x) => x + %d);", name, r.below(9))
	case 9:
		return fmt.Sprintf("export const %s = [\"a\", \"b\"].map((s) => s.length + %d);", name, r.below(9))
	case 10:
		return fmt.Sprintf("export const %s = new Map<string, %s>().get(\"k\");", name, pick(r, []string{"number", "string"}))
	case 11:
		return fmt.Sprintf("export const %s = Promise.resolve(%d).then((v) => v);", name, r.below(9))
	case 12:
		return fmt.Sprintf("export function %s<T>(x: T): T { return x; }", name)
	case 13:
		return fmt.Sprintf("export class %s { method(p: number): number { return p; } }", name)
	case 14:
		return fmt.Sprintf("export const %s = { m(a: number) { return a; } }.m(%d);", name, r.below(9))
	case 15:
		return fmt.Sprintf("export const %s = (null as unknown as Readonly<{ x: number; y: string }>).%s;", name, pick(r, []string{"x", "y"}))
	case 16:
		return fmt.Sprintf("export const %s = (null as unknown as Partial<{ a: number }>).a;", name)
	case 17:
		return fmt.Sprintf("export interface %s { plain: number; call(a: string): void }", name)
	}
	// A use of another module's export.
	if len(exports) == 0 {
		return fmt.Sprintf("export const %s = %d;", name, r.below(9))
	}
	target := pick(r, exports)
	return useStatement(r, name, target.name, target.kind)
}

// importsFor is the import lines the statements of file need.
func (w *world) importsFor(file string) []string {
	lines := w.files[file]
	wanted := map[string]map[string]struct{}{}
	for _, e := range w.exports() {
		if e.module+".ts" == file {
			continue
		}
		used := false
		for _, line := range lines {
			if strings.HasPrefix(line, "import ") || strings.Contains(line, "export const "+e.name+" =") {
				continue
			}
			for _, word := range strings.FieldsFunc(line, func(c rune) bool { return !isAlnum(c) }) {
				if word == e.name {
					used = true
				}
			}
		}
		if used {
			if wanted[e.module] == nil {
				wanted[e.module] = map[string]struct{}{}
			}
			wanted[e.module][e.name] = struct{}{}
		}
	}
	modules := make([]string, 0, len(wanted))
	for module := range wanted {
		modules = append(modules, module)
	}
	sort.Strings(modules)
	var out []string
	for _, module := range modules {
		names := make([]string, 0, len(wanted[module]))
		for name := range wanted[module] {
			names = append(names, name)
		}
		sort.Strings(names)
		out = append(out, fmt.Sprintf("import { %s } from \"./%s\";", strings.Join(names, ", "), module))
	}
	return out
}

func (w *world) normalizeImports(file string) {
	lines, ok := w.files[file]
	if !ok || !strings.HasPrefix(file, "m") {
		return
	}
	var body []string
	for _, line := range lines {
		if !strings.HasPrefix(line, "import ") {
			body = append(body, line)
		}
	}
	w.files[file] = body
	imports := w.importsFor(file)
	w.files[file] = append(imports, body...)
}

func newWorld(r *rng, modules int, plain bool) *world {
	w := &world{files: map[string][]string{}, graveyard: map[string][]string{}, plain: plain}
	for index := 0; index < modules; index++ {
		file := fmt.Sprintf("m%d.ts", index)
		w.files[file] = nil
		for count := 3 + r.below(4); count > 0; count-- {
			w.files[file] = append(w.files[file], w.randomStatement(r, file))
		}
		w.normalizeImports(file)
	}
	// A global script that augments an instantiable library interface.
	globals := []string{"declare var G0: number;"}
	if !plain {
		globals = append(globals, "interface Array<T> { firstOr(d: T): T }")
	}
	w.files["g0.ts"] = globals
	uses := "export const usesGlobal = [1, 2].firstOr(0) + G0;"
	if plain {
		uses = "export const usesGlobal = 1 + G0;"
	}
	w.files["m0.ts"] = append(w.files["m0.ts"], uses)
	for _, name := range w.moduleNames() {
		w.normalizeImports(name)
	}
	return w
}

func mutateDigits(line string, r *rng) string {
	var digits []int
	for index, c := range line {
		if c >= '0' && c <= '9' {
			digits = append(digits, index)
		}
	}
	if len(digits) == 0 {
		return fmt.Sprintf("%s // %d", line, r.below(100))
	}
	at := pick(r, digits)
	old := line[at] - '0'
	next := (old + 1 + byte(r.below(8))) % 10
	return line[:at] + string('0'+next) + line[at+1:]
}

// edit is one random edit: the files whose text changed (nil is a deletion).
type edit struct {
	description string
	changes     []fileEdit
}

type fileEdit struct {
	name   string
	source *string
}

// tryEdit applies one random edit; false means the draw was not applicable and
// nothing changed.
func (w *world) tryEdit(r *rng) (description string, configChange string, ok bool) {
	kind := r.below(14)
	names := sortedKeys(w.files)
	moduleNames := w.moduleNames()
	switch kind {
	case 0, 1:
		// Body edit, same span (digits keep their width) or shifting.
		file := pick(r, moduleNames)
		lines := w.files[file]
		if len(lines) == 0 {
			return "", "", false
		}
		at := r.below(len(lines))
		if strings.HasPrefix(lines[at], "import ") {
			return "", "", false
		}
		old := lines[at]
		lines[at] = mutateDigits(old, r)
		return fmt.Sprintf("body edit %s: %s -> %s", file, old, lines[at]), "", true
	case 2:
		// Comment-only edits: a prefix shifts every span, an append does not.
		file := pick(r, names)
		if r.chance(50) {
			w.files[file] = append([]string{fmt.Sprintf("// edit %d", r.below(1000))}, w.files[file]...)
			return "prefix comment " + file, "", true
		}
		w.files[file] = append(w.files[file], fmt.Sprintf("// edit %d", r.below(1000)))
		return "append comment " + file, "", true
	case 3, 4:
		// Signature edit: swap a type keyword in a function or const.
		file := pick(r, moduleNames)
		lines := w.files[file]
		var candidates []int
		for at, line := range lines {
			if strings.Contains(line, "number") || strings.Contains(line, "string") {
				candidates = append(candidates, at)
			}
		}
		if len(candidates) == 0 {
			return "", "", false
		}
		at := pick(r, candidates)
		old := lines[at]
		if strings.Contains(old, "number") {
			lines[at] = strings.Replace(old, "number", "string", 1)
		} else {
			lines[at] = strings.Replace(old, "string", "number", 1)
		}
		return fmt.Sprintf("signature edit %s: %s -> %s", file, old, lines[at]), "", true
	case 5:
		// Export add: a new exported statement, imported by another module.
		file := pick(r, moduleNames)
		statement := w.randomStatement(r, file)
		w.files[file] = append(w.files[file], statement)
		for _, name := range w.moduleNames() {
			if r.chance(40) || name == file {
				w.normalizeImports(name)
			}
		}
		return fmt.Sprintf("export add %s: %s", file, statement), "", true
	case 6:
		// Export remove: drop a statement.
		file := pick(r, moduleNames)
		lines := w.files[file]
		var candidates []int
		for at, line := range lines {
			if strings.HasPrefix(line, "export ") {
				candidates = append(candidates, at)
			}
		}
		if len(candidates) < 2 {
			return "", "", false
		}
		at := pick(r, candidates)
		removed := lines[at]
		w.files[file] = append(lines[:at:at], lines[at+1:]...)
		return fmt.Sprintf("export remove %s: %s", file, removed), "", true
	case 7:
		// Import add: a use of an export the file did not use.
		file := pick(r, moduleNames)
		exports := w.otherExports(file)
		if len(exports) == 0 {
			return "", "", false
		}
		target := pick(r, exports)
		fresh := w.freshName("u")
		statement := useStatement(r, fresh, target.name, target.kind)
		w.files[file] = append(w.files[file], statement)
		w.normalizeImports(file)
		return fmt.Sprintf("import add %s <- %s.%s: %s", file, target.module, target.name, statement), "", true
	case 8:
		// Import remove.
		file := pick(r, moduleNames)
		lines := w.files[file]
		for at, line := range lines {
			if strings.HasPrefix(line, "import ") {
				w.files[file] = append(lines[:at:at], lines[at+1:]...)
				return fmt.Sprintf("import remove %s: %s", file, line), "", true
			}
		}
		return "", "", false
	case 9:
		// File add.
		file := w.freshName("m") + ".ts"
		w.files[file] = nil
		for count := 2 + r.below(3); count > 0; count-- {
			w.files[file] = append(w.files[file], w.randomStatement(r, file))
		}
		w.normalizeImports(file)
		return "file add " + file, "", true
	case 10:
		// File delete, then (a later draw) restore.
		if len(moduleNames) < 3 {
			return "", "", false
		}
		file := pick(r, moduleNames)
		w.graveyard[file] = w.files[file]
		delete(w.files, file)
		return "file delete " + file, "", true
	case 11:
		if len(w.graveyard) == 0 {
			return "", "", false
		}
		file := sortedKeys(w.graveyard)[0]
		w.files[file] = w.graveyard[file]
		delete(w.graveyard, file)
		return "file restore " + file, "", true
	case 12:
		// Rename an exported function or const, in some importers.
		exports := w.exports()
		if len(exports) == 0 {
			return "", "", false
		}
		target := pick(r, exports)
		replacement := w.freshName("r")
		everywhere := r.chance(60)
		for _, file := range sortedKeys(w.files) {
			declaring := file == target.module+".ts"
			if !(declaring || everywhere) || strings.HasPrefix(file, "g") {
				continue
			}
			lines := w.files[file]
			for index, line := range lines {
				var out strings.Builder
				start := 0
				flush := func(end int) {
					part := line[start:end]
					word := leadingWord(part)
					if word == target.name {
						out.WriteString(replacement + part[len(word):])
					} else {
						out.WriteString(part)
					}
					start = end
				}
				for at, c := range line {
					if !isAlnum(c) {
						flush(at + len(string(c)))
					}
				}
				if start < len(line) {
					flush(len(line))
				}
				lines[index] = out.String()
			}
		}
		return fmt.Sprintf("rename %s.%s -> %s (importers %t)", target.module, target.name, replacement, everywhere), "", true
	}
	// Global scope or tsconfig.
	if r.chance(50) {
		lines := w.files["g0.ts"]
		switch r.below(3) {
		case 0:
			name := fmt.Sprintf("G%d", r.below(5))
			w.files["g0.ts"] = append(lines, fmt.Sprintf("declare var %s: %s;", name, pick(r, randomTypes)))
			return "global add " + name, "", true
		case 1:
			var kept []string
			for _, line := range lines {
				if !strings.HasPrefix(line, "interface Array") {
					kept = append(kept, line)
				}
			}
			w.files["g0.ts"] = append(kept, fmt.Sprintf("interface Array<T> { firstOr(d: T): %s }", pick(r, []string{"T", "number", "T | undefined"})))
			return "global interface edit", "", true
		default:
			if len(lines) < 2 {
				return "", "", false
			}
			at := r.below(len(lines))
			removed := lines[at]
			w.files["g0.ts"] = append(lines[:at:at], lines[at+1:]...)
			return "global remove " + removed, "", true
		}
	}
	next := (w.config + 1 + r.below(len(randomConfigs)-1)) % len(randomConfigs)
	w.config = next
	return fmt.Sprintf("tsconfig -> variant %d", next), randomConfigs[next], true
}

// randomEdit applies one random edit and returns it with the files whose text
// changed (a nil source is a deletion).
func (w *world) randomEdit(r *rng) edit {
	before := make(map[string][]string, len(w.files))
	for name, lines := range w.files {
		before[name] = append([]string(nil), lines...)
	}
	var description, configChange string
	for {
		var ok bool
		description, configChange, ok = w.tryEdit(r)
		if ok {
			break
		}
	}
	var changes []fileEdit
	for _, file := range sortedKeys(w.files) {
		old, existed := before[file]
		if !existed || strings.Join(old, "\n") != strings.Join(w.files[file], "\n") || len(old) != len(w.files[file]) {
			source := render(w.files[file])
			changes = append(changes, fileEdit{file, &source})
		}
	}
	for _, file := range sortedKeys(before) {
		if _, kept := w.files[file]; !kept {
			changes = append(changes, fileEdit{file, nil})
		}
	}
	if configChange != "" {
		config := configChange
		changes = append(changes, fileEdit{"tsconfig.json", &config})
	}
	return edit{description, changes}
}

func randomSeeds(t *testing.T) (from, to uint64) {
	value := os.Getenv("SOLID_RANDOM_SEEDS")
	if value == "" {
		return 0, 6
	}
	if low, high, ok := strings.Cut(value, ".."); ok {
		a, err1 := strconv.ParseUint(low, 10, 64)
		b, err2 := strconv.ParseUint(high, 10, 64)
		if err1 != nil || err2 != nil {
			t.Fatalf("bad SOLID_RANDOM_SEEDS %q", value)
		}
		return a, b
	}
	n, err := strconv.ParseUint(value, 10, 64)
	if err != nil {
		t.Fatalf("bad SOLID_RANDOM_SEEDS %q", value)
	}
	return 0, n
}

func randomSteps(t *testing.T) int {
	value := os.Getenv("SOLID_RANDOM_STEPS")
	if value == "" {
		return 12
	}
	n, err := strconv.Atoi(value)
	if err != nil {
		t.Fatalf("bad SOLID_RANDOM_STEPS %q", value)
	}
	return n
}

var symbolIDPattern = func() func(string) []typefacts.SymbolID {
	return func(line string) []typefacts.SymbolID {
		var ids []typefacts.SymbolID
		rest := line
		for {
			at := strings.Index(rest, `"symbol:`)
			if at < 0 {
				return ids
			}
			tail := rest[at+1:]
			end := strings.IndexByte(tail, '"')
			if end < 0 {
				end = len(tail)
			}
			ids = append(ids, typefacts.SymbolID(tail[:end]))
			rest = tail[end:]
		}
	}
}()

type symbolEvidenceSource interface {
	SymbolEvidence(context.Context, []typefacts.SymbolQueryV6) ([]typefacts.SymbolFact, error)
	SourceCalls(context.Context, string) ([]typefacts.SourceCall, error)
	SourceBindings(context.Context, string) ([]typefacts.SourceBinding, error)
	SourceFunctions(context.Context, string) ([]typefacts.SourceFunction, error)
	SourceAsyncFunctions(context.Context, string) ([]typefacts.AsyncFunctionFact, error)
}

type probeResult struct {
	table       *typefacts.FactTable
	lines       []string
	symbols     []string
	files       []string
	modules     string
	sourceFacts []string
}

// marshalRows is marshal with a nil slice and an empty one spelled alike: a
// memoized copy of an empty list is nil, a fresh answer is empty, and no
// consumer can tell them apart.
func marshalRows[T any](rows []T) string {
	if len(rows) == 0 {
		return "[]"
	}
	return marshal(rows)
}

func marshal(value any) string {
	encoded, err := json.Marshal(value)
	if err != nil {
		return fmt.Sprintf("<unmarshalable %T: %v>", value, err)
	}
	return string(encoded)
}

// probe is everything the producer answers for a generation, as comparable
// text: the entity table, the symbol evidence for every id it names, the file
// rows, the module graph and the per-file source facts.
func probe(t *testing.T, session *retentionSession, generation uint64, retained bool) probeResult {
	t.Helper()
	ctx := context.Background()
	demands := textDemands(t, session.closure, allFamilies)
	groups := canonicalDemandGroups(demands)
	var changed []string
	if !retained {
		changed = demandPaths(demands)
	}
	if retained {
		groups = groupedDemands(demands)
	}
	table, err := session.closure.DemandTableForGroups(ctx, generation, groups, changed)
	if err != nil {
		t.Fatalf("materialization at %d: %v", generation, err)
	}
	lines := describeEntities(table)
	seen := map[typefacts.SymbolID]struct{}{}
	var queries []typefacts.SymbolQueryV6
	for _, line := range lines {
		for _, id := range symbolIDPattern(line) {
			if _, dup := seen[id]; !dup {
				seen[id] = struct{}{}
				queries = append(queries, typefacts.SymbolQueryV6{ID: id, References: true})
			}
		}
	}
	source := session.backend.(symbolEvidenceSource)
	evidence, err := source.SymbolEvidence(ctx, queries)
	if err != nil {
		t.Fatalf("symbol evidence at %d: %v", generation, err)
	}
	var symbols []string
	for _, fact := range evidence {
		symbols = append(symbols, marshal(fact))
	}
	var files []string
	for _, file := range table.Files {
		files = append(files, marshal(file))
	}
	graph, err := session.closure.ModuleGraph(ctx, typefacts.ModuleInventoryDemand{Imports: true, Packages: true})
	if err != nil {
		t.Fatalf("module graph at %d: %v", generation, err)
	}
	sources, err := session.closure.SourceFiles(ctx)
	if err != nil {
		t.Fatal(err)
	}
	var sourceFacts []string
	for _, file := range sources {
		if strings.HasSuffix(file.Path, ".d.ts") || !strings.HasSuffix(file.Path, ".ts") {
			continue
		}
		path := filepath.Clean(file.Path)
		calls, _ := source.SourceCalls(ctx, path)
		bindings, _ := source.SourceBindings(ctx, path)
		functions, _ := source.SourceFunctions(ctx, path)
		asyncFunctions, _ := source.SourceAsyncFunctions(ctx, path)
		sourceFacts = append(sourceFacts, fmt.Sprintf("%s\n%s\n%s\n%s\n%s", path, marshalRows(calls), marshalRows(bindings), marshalRows(functions), marshalRows(asyncFunctions)))
	}
	renamer := counterRenamer{}
	rename := func(in []string) []string {
		out := make([]string, len(in))
		for i, line := range in {
			out[i] = renamer.apply(line)
		}
		return out
	}
	return probeResult{
		table:       table,
		lines:       rename(lines),
		symbols:     rename(symbols),
		files:       rename(files),
		modules:     marshal(graph),
		sourceFacts: rename(sourceFacts),
	}
}

// runSeed follows steps random edits with a retained closure; after each, every
// family is compared with a closure that replays the same updates and
// materializes the whole batch. It returns the file-generations reused within
// a generation (never across an update).
func runSeed(t *testing.T, seed uint64, steps, modules int, plain bool) int {
	t.Helper()
	ctx := context.Background()
	r := newRng(seed)
	w := newWorld(r, modules, plain)
	files := map[string]string{"tsconfig.json": randomConfigs[0]}
	for name, lines := range w.files {
		files[name] = render(lines)
	}
	root, _ := writeAdversarialProject(t, files)
	projectID := filepath.Join(root, "tsconfig.json")
	incremental := openRetentionSession(t, projectID)
	probe(t, incremental, 1, false)
	var log []string
	var history [][]typefacts.FileChange
	versions := map[string]uint64{}
	retainedTotal := 0
	var sameGeneration []string
	for step := 0; step < steps; step++ {
		// An update that changes no file is not sent by the client, and the
		// producer keeps its checker across it, so draw again.
		var drawn edit
		for {
			drawn = w.randomEdit(r)
			if len(drawn.changes) != 0 {
				break
			}
		}
		log = append(log, fmt.Sprintf("step %d: %s", step, drawn.description))
		var changes []typefacts.FileChange
		for _, c := range drawn.changes {
			versions[c.name]++
			change := typefacts.FileChange{Path: filepath.Join(root, c.name), Version: versions[c.name]}
			if c.source == nil {
				change.Deleted = true
			} else {
				change.Source = []byte(*c.source)
			}
			changes = append(changes, change)
		}
		// One update per edit, however many files it touches.
		if _, err := incremental.closure.Update(ctx, changes); err != nil {
			t.Fatalf("incremental update %d: %v\n%s", step, err, strings.Join(log, "\n"))
		}
		history = append(history, changes)
		generation := uint64(step + 2)
		sameGeneration = nil
		retained := probe(t, incremental, generation, true)
		// ADR 0364: an update leaves nothing to retain.
		if kept := incremental.closure.Stats().Retention.RetainedFiles; kept != 0 {
			t.Fatalf("seed %d step %d: an update retained %d files", seed, step, kept)
		}
		// Within one generation a second analyze that names only one path as
		// changed must still equal a whole-batch run (and reuses nothing).
		demands := textDemands(t, incremental.closure, allFamilies)
		paths := demandPaths(demands)
		var grown []string
		if len(paths) > 1 {
			// The demand set shrinks to a random subset and grows back within
			// the generation: each table is a function of its demands alone,
			// so the grown table's durable ids equal a fresh whole batch's
			// exactly (counters up to renaming).
			subset := demands[:0:0]
			skipped := paths[r.below(len(paths))]
			for _, demand := range demands {
				if filepath.Clean(demand.Location.Path) != skipped {
					subset = append(subset, demand)
				}
			}
			if _, err := incremental.closure.DemandTableForGroups(ctx, generation, groupedDemands(subset), paths); err != nil {
				t.Fatalf("seed %d step %d: subset analyze: %v", seed, step, err)
			}
			regrown, err := incremental.closure.DemandTableForGroups(ctx, generation, groupedDemands(demands), []string{skipped})
			if err != nil {
				t.Fatalf("seed %d step %d: regrown analyze: %v", seed, step, err)
			}
			retainedTotal += incremental.closure.Stats().Retention.RetainedFiles
			renamer := counterRenamer{}
			for _, line := range describeEntities(regrown) {
				grown = append(grown, renamer.apply(line))
			}
		}
		if len(paths) != 0 {
			again, err := incremental.closure.DemandTableForGroups(
				ctx, generation, groupedDemands(demands), []string{paths[r.below(len(paths))]})
			if err != nil {
				t.Fatalf("seed %d step %d: same-generation analyze: %v", seed, step, err)
			}
			retainedTotal += incremental.closure.Stats().Retention.RetainedFiles
			renamer := counterRenamer{}
			for _, line := range describeEntities(again) {
				sameGeneration = append(sameGeneration, renamer.apply(line))
			}
		}

		// The oracle replays every edit so far as its own update, as the
		// retained side saw them, and then materializes the whole batch.
		fresh := openRetentionSession(t, projectID)
		for replay, changes := range history {
			if _, err := fresh.closure.Update(ctx, changes); err != nil {
				t.Fatalf("fresh replay %d/%d: %v", replay, step, err)
			}
		}
		whole := probe(t, fresh, generation, false)
		var failures []string
		for _, comparison := range []struct {
			what        string
			left, right []string
		}{
			{"entity rows", retained.lines, whole.lines},
			{"symbol evidence", retained.symbols, whole.symbols},
			{"file rows", retained.files, whole.files},
			{"source facts", retained.sourceFacts, whole.sourceFacts},
		} {
			if report := reportDifference(comparison.what, comparison.left, comparison.right); report != "" {
				failures = append(failures, report)
			}
		}
		if sameGeneration != nil {
			if report := reportDifference("same-generation entity rows", sameGeneration, whole.lines); report != "" {
				failures = append(failures, report)
			}
		}
		if grown != nil {
			if report := reportDifference("regrown-demand entity rows", grown, whole.lines); report != "" {
				failures = append(failures, report)
			}
		}
		if retained.modules != whole.modules {
			failures = append(failures, "module graph differs")
		}
		if len(failures) != 0 {
			t.Fatalf("seed %d step %d: retained generation differs from fresh\n%s\n%s",
				seed, step, strings.Join(failures, "\n"), strings.Join(log, "\n"))
		}
	}
	t.Logf("seed %d: %d steps, %d same-generation reused files", seed, steps, retainedTotal)
	return retainedTotal
}

func TestSeededRandomEditSequencesMatchFresh(t *testing.T) {
	from, to := randomSeeds(t)
	for seed := from; seed < to; seed++ {
		runSeed(t, seed, randomSteps(t), 5, false)
	}
}

// The same edit kinds over files whose answers stay retainable, so the
// retention, affected-set and dependent-invalidation logic is what is compared.
func TestSeededRandomEditSequencesOverRetainableFilesMatchFresh(t *testing.T) {
	from, to := randomSeeds(t)
	retained := 0
	for seed := from; seed < to; seed++ {
		retained += runSeed(t, seed, randomSteps(t), 8, true)
	}
	// ADR 0364: every table build discards derived state, so no same-generation
	// analyze reuses a file.
	if retained != 0 {
		t.Fatalf("same-generation analyzes reused %d files; each table build discards derived state", retained)
	}
}
