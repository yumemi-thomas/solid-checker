// Package tsgo adapts the pinned tsgolint/typescript-go integration to the
// compiler-independent typefacts seam. No shim or compiler types escape it.
package tsgo

import (
	"context"
	"crypto/sha256"
	"encoding/hex"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"sort"
	"strings"
	"sync"
	"time"
	"unicode/utf8"

	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/bundled"
	"github.com/microsoft/typescript-go/shim/checker"
	"github.com/microsoft/typescript-go/shim/compiler"
	"github.com/microsoft/typescript-go/shim/core"
	"github.com/microsoft/typescript-go/shim/scanner"
	"github.com/microsoft/typescript-go/shim/tsoptions"
	"github.com/microsoft/typescript-go/shim/vfs"
	"github.com/microsoft/typescript-go/shim/vfs/osvfs"

	"github.com/yumemi-thomas/solid-check/internal/typefacts"
)

var ErrClosed = errors.New("type facts project is closed")

var _ typefacts.Project = (*project)(nil)

type project struct {
	mu             sync.Mutex
	configPath     string
	fs             *overlayFS
	versions       map[string]uint64
	program        *compiler.Program
	checker        *checker.Checker
	release        func()
	closed         bool
	generation     uint64
	nextSymbol     uint64
	idsBySymbol    map[*ast.Symbol]typefacts.SymbolID
	symbolsByID    map[typefacts.SymbolID]*ast.Symbol
	nextType       uint64
	idsByType      map[*checker.Type]typefacts.TypeID
	referenceIndex referenceIndex
	// sourceFactsMemo carries per-file Source* facts across generations. An
	// entry is stored only when every symbol identity it contains is durable,
	// so its facts stay resolvable after an update. Update drops the affected
	// set on the incremental path and clears the memo on full rebuilds.
	sourceFactsMemo map[string]*fileFactsMemo
	// durableRefs maps a durable SymbolID back to its declaration so the
	// symbol can be re-resolved lazily in a later generation.
	durableRefs map[typefacts.SymbolID]durableSymbolRef
	// filesByName is a generation-scoped index of program files keyed by
	// their cleaned file name. Program.GetSourceFile does not round-trip
	// virtual bundled-lib names (bundled:/… is resolved against the working
	// directory), so durable re-resolution of lib-declared symbols falls
	// back to this index. Nil until the first fallback of a generation.
	filesByName map[string]*ast.SourceFile
	// declarationShapes caches diagnostic-free exported contracts for the
	// accepted program generation. Incremental updates need only emit the
	// candidate generation's shape; semantically affected files are evicted
	// and broad rebuilds clear the cache.
	declarationShapes map[string]declarationShape
	// exportedIdentities assigns module-visible target symbols a
	// span-insensitive identity derived from declaring path and symbol name.
	// Module-scope export uniqueness makes this deterministic across process
	// restart while nested/non-exported symbols keep span-based identities.
	exportedIdentities map[*ast.Symbol]preservedExportIdentity
}

// OpenProject loads and binds the TypeScript project at configPath.
func OpenProject(ctx context.Context, configPath string) (typefacts.Project, error) {
	absConfigPath, err := filepath.Abs(configPath)
	if err != nil {
		return nil, fmt.Errorf("resolve tsconfig path: %w", err)
	}
	absConfigPath = normalizeTypeScriptPath(absConfigPath)
	fs := newOverlayFS(bundled.WrapFS(osvfs.FS()))
	program, typeChecker, release, err := buildProgram(ctx, absConfigPath, fs)
	if err != nil {
		return nil, err
	}

	opened := &project{
		configPath:      absConfigPath,
		fs:              fs,
		versions:        make(map[string]uint64),
		program:         program,
		checker:         typeChecker,
		release:         release,
		generation:      1,
		idsBySymbol:     make(map[*ast.Symbol]typefacts.SymbolID),
		symbolsByID:     make(map[typefacts.SymbolID]*ast.Symbol),
		idsByType:       make(map[*checker.Type]typefacts.TypeID),
		sourceFactsMemo: make(map[string]*fileFactsMemo),
		durableRefs:     make(map[typefacts.SymbolID]durableSymbolRef),
	}
	opened.exportedIdentities = collectExportedIdentities(program, typeChecker)
	return opened, nil
}

// TypeScript paths always use forward slashes, including on Windows. The
// underlying OS filesystem accepts them, while TypeScript-Go's path and VFS
// helpers do not consistently treat backslashes as directory separators.
func normalizeTypeScriptPath(path string) string {
	return strings.ReplaceAll(path, `\`, "/")
}

// singleCheckerPool serves this adapter's one retained checker. UpdateProgram
// inherits it through program options, so incremental updates construct one
// checker instead of the default pool's four.
type singleCheckerPool struct {
	program *compiler.Program
	once    sync.Once
	checker *checker.Checker
	lock    *sync.Mutex
}

func newSingleCheckerPool(program *compiler.Program) compiler.CheckerPool {
	return &singleCheckerPool{program: program}
}

func (p *singleCheckerPool) GetChecker(ctx context.Context, file *ast.SourceFile) (*checker.Checker, func()) {
	p.once.Do(func() {
		p.checker, p.lock = checker.NewChecker(p.program, nil)
	})
	if file != nil {
		// Program.Emit asks for a file-affine checker and its declaration
		// resolver takes the checker's internal mutex itself. Returning the
		// lifetime lease here would deadlock on that reentrant lock. All
		// adapter entry points are already serialized by project.mu, and a
		// targeted emit processes one source file, so match the compiler's
		// built-in pool by making file-affine access non-exclusive.
		return p.checker, func() {}
	}
	p.lock.Lock()
	return p.checker, sync.OnceFunc(p.lock.Unlock)
}

func buildProgram(ctx context.Context, configPath string, fs vfs.FS) (*compiler.Program, *checker.Checker, func(), error) {
	cwd := filepath.Dir(configPath)
	host := compiler.NewCompilerHost(cwd, fs, bundled.LibPath(), nil, nil)
	config, diagnostics := tsoptions.GetParsedCommandLineOfConfigFile(configPath, &core.CompilerOptions{}, nil, host, nil)
	if len(diagnostics) != 0 {
		return nil, nil, nil, fmt.Errorf("parse tsconfig: %d diagnostic(s)", len(diagnostics))
	}
	if config == nil {
		return nil, nil, nil, errors.New("parse tsconfig: no configuration returned")
	}
	if len(config.Errors) != 0 {
		return nil, nil, nil, fmt.Errorf("parse tsconfig: %d configuration error(s)", len(config.Errors))
	}

	program := compiler.NewProgram(compiler.ProgramOptions{
		Config: config,
		// Parse and bind in parallel, but keep exactly one checker: this
		// adapter acquires a single checker for the project's lifetime, and
		// the default non-single-threaded pool constructs four checkers on
		// every program update, tripling editor-path allocation.
		SingleThreaded:              core.TSFalse,
		CreateCheckerPool:           newSingleCheckerPool,
		Host:                        host,
		UseSourceOfProjectReference: true,
	})
	if program == nil {
		return nil, nil, nil, errors.New("create TypeScript program")
	}
	return finishProgram(ctx, program)
}

func updateProgram(ctx context.Context, oldProgram *compiler.Program, configPath string, fs vfs.FS, changedPath string) (*compiler.Program, *checker.Checker, func(), error) {
	if err := ctx.Err(); err != nil {
		return nil, nil, nil, err
	}
	oldFile := oldProgram.GetSourceFile(changedPath)
	if oldFile == nil {
		return buildProgram(ctx, configPath, fs)
	}
	host := compiler.NewCompilerHost(filepath.Dir(configPath), fs, bundled.LibPath(), nil, nil)
	program, _, _ := oldProgram.UpdateProgram(oldFile.Path(), host, nil)
	if program == nil {
		return nil, nil, nil, errors.New("update TypeScript program")
	}
	return finishProgram(ctx, program)
}

func finishProgram(ctx context.Context, program *compiler.Program) (*compiler.Program, *checker.Checker, func(), error) {
	program.BindSourceFiles()
	typeChecker, release := program.GetTypeChecker(ctx)
	if typeChecker == nil {
		if release != nil {
			release()
		}
		return nil, nil, nil, errors.New("create TypeScript checker")
	}
	return program, typeChecker, release, nil
}

func (p *project) SourceFiles(ctx context.Context) ([]typefacts.SourceFile, error) {
	if err := ctx.Err(); err != nil {
		return nil, err
	}
	p.mu.Lock()
	defer p.mu.Unlock()
	if p.closed {
		return nil, ErrClosed
	}
	files := make([]typefacts.SourceFile, 0)
	for _, sourceFile := range p.program.SourceFiles() {
		if sourceFile.IsDeclarationFile {
			continue
		}
		files = append(files, typefacts.SourceFile{
			Path:   filepath.Clean(sourceFile.FileName()),
			Source: []byte(sourceFile.Text()),
		})
	}
	sort.Slice(files, func(i, j int) bool { return files[i].Path < files[j].Path })
	return files, nil
}

func (p *project) Update(ctx context.Context, changes []typefacts.FileChange) (typefacts.AffectedSet, error) {
	updateStarted := time.Now()
	if err := ctx.Err(); err != nil {
		return typefacts.AffectedSet{}, err
	}
	p.mu.Lock()
	defer p.mu.Unlock()
	if p.closed {
		return typefacts.AffectedSet{}, ErrClosed
	}

	stageStarted := time.Now()
	candidateFS := p.fs.clone()
	candidateVersions := make(map[string]uint64, len(p.versions)+len(changes))
	for path, version := range p.versions {
		candidateVersions[path] = version
	}
	changedPaths := make([]string, 0, len(changes))
	incrementalPath := ""
	incremental := true
	for _, change := range changes {
		absolutePath, err := filepath.Abs(change.Path)
		if err != nil {
			return typefacts.AffectedSet{}, fmt.Errorf("resolve changed path: %w", err)
		}
		if version, ok := candidateVersions[absolutePath]; ok && change.Version <= version {
			continue
		}
		if incrementalPath != "" || change.Deleted || absolutePath == p.configPath {
			incremental = false
		} else {
			incrementalPath = absolutePath
		}
		candidateVersions[absolutePath] = change.Version
		if change.Deleted {
			candidateFS.delete(absolutePath)
		} else {
			candidateFS.set(absolutePath, string(change.Source))
		}
		changedPaths = append(changedPaths, absolutePath)
	}
	if len(changedPaths) == 0 {
		return typefacts.AffectedSet{Files: []string{}}, nil
	}
	overlayDuration := time.Since(stageStarted)

	oldProgram := p.program
	var oldShape declarationShape
	var oldShapeOK bool
	oldShapeCached := false
	stageStarted = time.Now()
	if incremental && incrementalPath != "" {
		oldShape, oldShapeCached = p.declarationShapes[incrementalPath]
		oldShapeOK = oldShapeCached
		if !oldShapeCached {
			oldShape, p.checker, p.release, oldShapeOK = declarationShapeFor(
				ctx,
				oldProgram,
				p.checker,
				p.release,
				incrementalPath,
			)
		}
		if err := ctx.Err(); err != nil {
			return typefacts.AffectedSet{}, err
		}
	}
	oldShapeDuration := time.Since(stageStarted)
	var program *compiler.Program
	var typeChecker *checker.Checker
	var release func()
	var err error
	stageStarted = time.Now()
	if incremental && incrementalPath != "" {
		program, typeChecker, release, err = updateProgram(ctx, oldProgram, p.configPath, candidateFS, incrementalPath)
	} else {
		program, typeChecker, release, err = buildProgram(ctx, p.configPath, candidateFS)
	}
	if err != nil {
		return typefacts.AffectedSet{}, err
	}
	programDuration := time.Since(stageStarted)
	semanticCutoff := false
	var newShape declarationShape
	var newShapeOK bool
	var currentExports map[*ast.Symbol]preservedExportIdentity
	stageStarted = time.Now()
	if oldShapeOK {
		newShape, typeChecker, release, newShapeOK = declarationShapeFor(
			ctx,
			program,
			typeChecker,
			release,
			incrementalPath,
		)
		if newShapeOK {
			currentExports = declarationExportIdentities(newShape)
			_, semanticCutoff = preserveExportIdentities(oldShape, newShape)
		}
		if err := ctx.Err(); err != nil {
			if release != nil {
				release()
			}
			return typefacts.AffectedSet{}, err
		}
	}
	newShapeDuration := time.Since(stageStarted)
	if p.release != nil {
		p.release()
	}
	p.program = program
	p.checker = typeChecker
	p.release = release
	p.fs = candidateFS
	p.versions = candidateVersions
	p.generation++
	if incremental && incrementalPath != "" && newShapeOK {
		for symbol, identity := range p.exportedIdentities {
			if identity.ref.path == incrementalPath {
				delete(p.exportedIdentities, symbol)
			}
		}
		for symbol, identity := range currentExports {
			p.exportedIdentities[symbol] = identity
		}
	} else {
		p.exportedIdentities = collectExportedIdentities(program, typeChecker)
	}
	clear(p.idsBySymbol)
	clear(p.symbolsByID)
	clear(p.idsByType)
	for symbol, preserved := range currentExports {
		p.idsBySymbol[symbol] = preserved.id
		p.symbolsByID[preserved.id] = symbol
		p.durableRefs[preserved.id] = preserved.ref
	}
	p.filesByName = nil
	p.nextSymbol = 0
	p.nextType = 0

	stageStarted = time.Now()
	var affected []string
	if semanticCutoff {
		// A diagnostic-free declaration emit proves that the module's
		// exported TypeScript shape is unchanged, and every external export
		// slot was paired bijectively with its prior canonical ID. Retained
		// importer facts can therefore keep those IDs even when declaration
		// spans inside the edited module moved.
		affected = append([]string(nil), changedPaths...)
	} else {
		affected = affectedFiles(changedPaths, oldProgram, program)
	}
	sort.Strings(affected)
	affectedDuration := time.Since(stageStarted)
	stageStarted = time.Now()
	if incremental && incrementalPath != "" {
		// Source facts and reference contributions of files outside the
		// affected set survive the generation: their text is unchanged and
		// every durable identity they reference declares in an unchanged
		// file (a changed declaring file would have put the referencing
		// file in the affected set). Files that left the program are
		// evicted now, not when they are next queried, so an entry cannot
		// go stale while its file is outside the program and be reused if
		// the file later re-enters.
		dropped := make(map[string]struct{}, len(affected))
		for _, path := range affected {
			dropped[path] = struct{}{}
		}
		retained := func(path string) bool {
			if _, hit := dropped[path]; hit {
				return false
			}
			return program.GetSourceFile(path) != nil
		}
		for key, memo := range p.sourceFactsMemo {
			if !retained(memo.absPath) {
				delete(p.sourceFactsMemo, key)
			}
		}
		p.referenceIndex.invalidate(program, affected, retained)
	} else {
		// Full rebuilds (deletes, tsconfig changes, multi-file updates) can
		// change resolution outside the module graph; fail closed.
		clear(p.sourceFactsMemo)
		p.referenceIndex.reset()
	}
	if p.declarationShapes == nil {
		p.declarationShapes = make(map[string]declarationShape)
	}
	if incremental && incrementalPath != "" {
		for _, path := range affected {
			delete(p.declarationShapes, filepath.Clean(path))
		}
		if newShapeOK {
			p.declarationShapes[incrementalPath] = newShape
		}
	} else {
		clear(p.declarationShapes)
	}
	invalidationDuration := time.Since(stageStarted)
	if os.Getenv("SOLID_TYPEFACTS_TIMINGS") != "" {
		fmt.Fprintf(os.Stderr,
			"{\"typefactsUpdate\":{\"totalNs\":%d,\"overlayNs\":%d,\"oldShapeNs\":%d,\"oldShapeCached\":%t,\"programNs\":%d,\"newShapeNs\":%d,\"affectedNs\":%d,\"invalidationNs\":%d}}\n",
			time.Since(updateStarted), overlayDuration, oldShapeDuration, oldShapeCached,
			programDuration, newShapeDuration, affectedDuration, invalidationDuration)
	}
	return typefacts.AffectedSet{Files: affected}, nil
}

type declarationShape struct {
	signature [sha256.Size]byte
	exports   []declarationExport
	imports   []string
}

type declarationExport struct {
	name   string
	id     typefacts.SymbolID
	ref    durableSymbolRef
	symbol *ast.Symbol
}

type preservedExportIdentity struct {
	id  typefacts.SymbolID
	ref durableSymbolRef
}

func declarationExportIdentities(shape declarationShape) map[*ast.Symbol]preservedExportIdentity {
	identities := make(map[*ast.Symbol]preservedExportIdentity, len(shape.exports))
	for _, exported := range shape.exports {
		identities[exported.symbol] = preservedExportIdentity{id: exported.id, ref: exported.ref}
	}
	return identities
}

// preserveExportIdentities proves that two module generations expose the same
// declaration contract and pairs their canonical target symbols by external
// export name. Export IDs are derived from declaring module path and symbol
// name, so an equal result is reproducible after process restart. Any
// non-bijective pairing fails closed.
func preserveExportIdentities(previous declarationShape, next declarationShape) (map[*ast.Symbol]preservedExportIdentity, bool) {
	if previous.signature != next.signature ||
		len(previous.exports) != len(next.exports) ||
		len(previous.imports) != len(next.imports) {
		return nil, false
	}
	for index := range previous.imports {
		if previous.imports[index] != next.imports[index] {
			return nil, false
		}
	}
	preserved := make(map[*ast.Symbol]preservedExportIdentity, len(next.exports))
	symbolByID := make(map[typefacts.SymbolID]*ast.Symbol, len(next.exports))
	for index := range previous.exports {
		oldExport := previous.exports[index]
		newExport := next.exports[index]
		if oldExport.name != newExport.name ||
			oldExport.id != newExport.id ||
			newExport.symbol == nil {
			return nil, false
		}
		if existing, ok := preserved[newExport.symbol]; ok && existing.id != newExport.id {
			return nil, false
		}
		if existing, ok := symbolByID[newExport.id]; ok && existing != newExport.symbol {
			return nil, false
		}
		preserved[newExport.symbol] = preservedExportIdentity{id: newExport.id, ref: newExport.ref}
		symbolByID[newExport.id] = newExport.symbol
	}
	return preserved, true
}

func declarationShapeFor(
	ctx context.Context,
	program *compiler.Program,
	typeChecker *checker.Checker,
	release func(),
	path string,
) (declarationShape, *checker.Checker, func(), bool) {
	sourceFile := program.GetSourceFile(path)
	if sourceFile == nil || !ast.IsExternalModule(sourceFile) || hasGlobalOrModuleAugmentation(sourceFile) {
		return declarationShape{}, typeChecker, release, false
	}

	var declarationText string
	var writeDiagnostics bool
	if release != nil {
		release()
	}
	result := program.Emit(ctx, compiler.EmitOptions{
		TargetSourceFile: sourceFile,
		EmitOnly:         compiler.EmitOnlyForcedDts,
		WriteFile: func(_ string, text string, data *compiler.WriteFileData) error {
			if data != nil && (len(data.Diagnostics) != 0 || data.SkippedDtsWrite) {
				writeDiagnostics = true
			}
			if declarationText != "" {
				// A source file should produce one declaration output. A
				// second output is not part of this slice's proof.
				writeDiagnostics = true
				return nil
			}
			declarationText = text
			return nil
		},
	})
	// Emit acquires the program's checker from the same single-checker pool.
	// The project mutex excludes external users while the lifetime lease is
	// temporarily released. Reacquire even after cancellation so the
	// retained project remains usable if this update is rejected.
	typeChecker, release = program.GetTypeChecker(context.WithoutCancel(ctx))
	if typeChecker == nil {
		return declarationShape{}, nil, nil, false
	}
	if ctx.Err() != nil || result == nil || result.EmitSkipped ||
		len(result.Diagnostics) != 0 || writeDiagnostics || declarationText == "" {
		return declarationShape{}, typeChecker, release, false
	}

	exports, ok := exportedDurableSymbols(typeChecker, sourceFile)
	if !ok {
		return declarationShape{}, typeChecker, release, false
	}
	imports, ok := resolvedImportPaths(program, sourceFile)
	if !ok {
		return declarationShape{}, typeChecker, release, false
	}
	return declarationShape{
		signature: sha256.Sum256([]byte(declarationText)),
		exports:   exports,
		imports:   imports,
	}, typeChecker, release, true
}

func hasGlobalOrModuleAugmentation(sourceFile *ast.SourceFile) bool {
	unsafe := false
	var visit func(*ast.Node) bool
	visit = func(node *ast.Node) bool {
		if ast.IsExternalModuleAugmentation(node) || ast.IsGlobalScopeAugmentation(node) {
			unsafe = true
			return true
		}
		node.ForEachChild(visit)
		return unsafe
	}
	for _, statement := range sourceFile.Statements.Nodes {
		if visit(statement) {
			return true
		}
	}
	return false
}

func exportedDurableSymbols(typeChecker *checker.Checker, sourceFile *ast.SourceFile) ([]declarationExport, bool) {
	if sourceFile.Symbol == nil {
		return nil, false
	}
	moduleExports := typeChecker.GetExportsOfModule(sourceFile.Symbol)
	exports := make([]declarationExport, 0, len(moduleExports))
	for _, moduleExport := range moduleExports {
		name := moduleExport.Name
		symbol := moduleExport
		if symbol.Flags&ast.SymbolFlagsAlias != 0 {
			symbol = typeChecker.GetAliasedSymbol(symbol)
		}
		if symbol == nil {
			return nil, false
		}
		ref, ok := durableRefFor(symbol)
		if !ok {
			return nil, false
		}
		exports = append(exports, declarationExport{
			name:   name,
			id:     ref.exportedID(),
			ref:    ref,
			symbol: symbol,
		})
	}
	sort.Slice(exports, func(i, j int) bool { return exports[i].name < exports[j].name })
	for index := 1; index < len(exports); index++ {
		if exports[index-1].name == exports[index].name {
			return nil, false
		}
	}
	return exports, true
}

func collectExportedIdentities(program *compiler.Program, typeChecker *checker.Checker) map[*ast.Symbol]preservedExportIdentity {
	identities := make(map[*ast.Symbol]preservedExportIdentity)
	for _, sourceFile := range program.SourceFiles() {
		if !ast.IsExternalModule(sourceFile) {
			continue
		}
		exports, ok := exportedDurableSymbols(typeChecker, sourceFile)
		if !ok {
			continue
		}
		for _, exported := range exports {
			if existing, ok := identities[exported.symbol]; ok && existing.id != exported.id {
				// The same target reached through incompatible deterministic
				// identities is not safe to canonicalize globally.
				delete(identities, exported.symbol)
				continue
			}
			identities[exported.symbol] = preservedExportIdentity{id: exported.id, ref: exported.ref}
		}
	}
	return identities
}

func resolvedImportPaths(program *compiler.Program, sourceFile *ast.SourceFile) ([]string, bool) {
	imports := make([]string, 0, len(sourceFile.Imports()))
	for _, specifier := range sourceFile.Imports() {
		resolved := program.GetResolvedModuleFromModuleSpecifier(sourceFile, specifier)
		if resolved == nil {
			return nil, false
		}
		imports = append(imports, filepath.Clean(resolved.ResolvedFileName))
	}
	sort.Strings(imports)
	return imports, true
}

func affectedFiles(changedPaths []string, programs ...*compiler.Program) []string {
	affected := make(map[string]struct{}, len(changedPaths))
	queue := make([]string, 0, len(changedPaths))
	for _, path := range changedPaths {
		path = filepath.Clean(path)
		affected[path] = struct{}{}
		queue = append(queue, path)
	}
	reverseDependencies := make(map[string]map[string]struct{})
	for _, program := range programs {
		for _, sourceFile := range program.SourceFiles() {
			if sourceFile.IsDeclarationFile {
				continue
			}
			importer := filepath.Clean(sourceFile.FileName())
			for _, specifier := range sourceFile.Imports() {
				resolved := program.GetResolvedModuleFromModuleSpecifier(sourceFile, specifier)
				if resolved == nil {
					continue
				}
				dependency := filepath.Clean(resolved.ResolvedFileName)
				importers := reverseDependencies[dependency]
				if importers == nil {
					importers = make(map[string]struct{})
					reverseDependencies[dependency] = importers
				}
				importers[importer] = struct{}{}
			}
		}
	}
	for len(queue) != 0 {
		dependency := queue[0]
		queue = queue[1:]
		for importer := range reverseDependencies[dependency] {
			if _, seen := affected[importer]; seen {
				continue
			}
			affected[importer] = struct{}{}
			queue = append(queue, importer)
		}
	}
	files := make([]string, 0, len(affected))
	for path := range affected {
		files = append(files, path)
	}
	return files
}

func (p *project) SymbolAt(ctx context.Context, location typefacts.Location) (typefacts.SymbolID, error) {
	if err := ctx.Err(); err != nil {
		return "", err
	}
	p.mu.Lock()
	defer p.mu.Unlock()
	if p.closed {
		return "", ErrClosed
	}
	sourceFile, err := p.sourceFileFor(location)
	if err != nil {
		return "", err
	}
	node := deepestNodeAt(ast.GetNodeAtPosition(sourceFile, location.StartByte, false), location.StartByte)
	if node == nil {
		return "", fmt.Errorf("%w: node at byte %d", typefacts.ErrNotFound, location.StartByte)
	}
	symbol := p.checker.GetSymbolAtLocation(node)
	if symbol == nil {
		return "", fmt.Errorf("%w: symbol at byte %d (node kind %v, range %d:%d)", typefacts.ErrNotFound, location.StartByte, node.Kind, node.Pos(), node.End())
	}
	return p.idFor(symbol), nil
}

func deepestNodeAt(node *ast.Node, position int) *ast.Node {
	if node == nil {
		return nil
	}
	best := node
	node.ForEachChild(func(child *ast.Node) bool {
		if child.Pos() <= position && position < child.End() {
			best = deepestNodeAt(child, position)
			return true
		}
		return false
	})
	return best
}

func (p *project) ResolveAlias(ctx context.Context, id typefacts.SymbolID) (typefacts.SymbolID, error) {
	if err := ctx.Err(); err != nil {
		return "", err
	}
	p.mu.Lock()
	defer p.mu.Unlock()
	if p.closed {
		return "", ErrClosed
	}
	symbol, ok := p.symbolFor(id)
	if !ok {
		return "", fmt.Errorf("%w: symbol %s", typefacts.ErrNotFound, id)
	}
	if symbol.Flags&ast.SymbolFlagsAlias == 0 {
		return "", fmt.Errorf("%w: symbol %s is not an alias", typefacts.ErrNotFound, id)
	}
	original := p.checker.GetAliasedSymbol(symbol)
	if original == nil {
		return "", fmt.Errorf("%w: aliased symbol %s", typefacts.ErrNotFound, id)
	}
	return p.idFor(original), nil
}

func (p *project) Declarations(ctx context.Context, id typefacts.SymbolID) ([]typefacts.Declaration, error) {
	if err := ctx.Err(); err != nil {
		return nil, err
	}
	p.mu.Lock()
	defer p.mu.Unlock()
	if p.closed {
		return nil, ErrClosed
	}
	symbol, ok := p.symbolFor(id)
	if !ok {
		return nil, fmt.Errorf("%w: symbol %s", typefacts.ErrNotFound, id)
	}
	declarations := make([]typefacts.Declaration, 0, len(symbol.Declarations))
	for _, node := range symbol.Declarations {
		sourceFile := ast.GetSourceFileOfNode(node)
		if sourceFile == nil {
			continue
		}
		nameNode := node.Name()
		if nameNode == nil {
			nameNode = node
		}
		declarations = append(declarations, typefacts.Declaration{
			Name: symbol.Name,
			Kind: declarationKind(node),
			Location: typefacts.Location{
				Path:      filepath.Clean(sourceFile.FileName()),
				StartByte: scanner.SkipTrivia(sourceFile.Text(), nameNode.Pos()),
				EndByte:   nameNode.End(),
			},
		})
	}
	if len(declarations) == 0 {
		return nil, fmt.Errorf("%w: declarations for symbol %s", typefacts.ErrNotFound, id)
	}
	return declarations, nil
}

func (p *project) TypeAt(ctx context.Context, location typefacts.Location) (typefacts.TypeID, error) {
	if err := ctx.Err(); err != nil {
		return "", err
	}
	p.mu.Lock()
	defer p.mu.Unlock()
	if p.closed {
		return "", ErrClosed
	}
	sourceFile, err := p.sourceFileFor(location)
	if err != nil {
		return "", err
	}
	node := ast.GetNodeAtPosition(sourceFile, location.StartByte, false)
	if node == nil {
		return "", fmt.Errorf("%w: node at byte %d", typefacts.ErrNotFound, location.StartByte)
	}
	value := p.checker.GetTypeAtLocation(node)
	if value == nil {
		return "", fmt.Errorf("%w: type at byte %d", typefacts.ErrNotFound, location.StartByte)
	}
	return p.idForType(value), nil
}

func (p *project) DescribeTypeAt(ctx context.Context, location typefacts.Location) (typefacts.TypeDescriptor, error) {
	if err := ctx.Err(); err != nil {
		return typefacts.TypeDescriptor{}, err
	}
	p.mu.Lock()
	defer p.mu.Unlock()
	if p.closed {
		return typefacts.TypeDescriptor{}, ErrClosed
	}
	sourceFile, err := p.sourceFileFor(location)
	if err != nil {
		return typefacts.TypeDescriptor{}, err
	}
	node := deepestNodeAt(ast.GetNodeAtPosition(sourceFile, location.StartByte, false), location.StartByte)
	if node == nil {
		return typefacts.TypeDescriptor{}, fmt.Errorf("%w: node at byte %d", typefacts.ErrNotFound, location.StartByte)
	}
	value := p.checker.GetTypeAtLocation(node)
	if value == nil {
		return typefacts.TypeDescriptor{}, fmt.Errorf("%w: type at byte %d", typefacts.ErrNotFound, location.StartByte)
	}
	descriptor := typefacts.TypeDescriptor{Text: p.checker.TypeToString(value)}
	if alias := value.Alias(); alias != nil && alias.Symbol() != nil {
		descriptor.AliasDeclarations = declarationsForSymbol(alias.Symbol())
		descriptor.OriginModule = declarationModule(alias.Symbol())
	}
	return descriptor, nil
}

func declarationModule(symbol *ast.Symbol) string {
	for _, declaration := range symbol.Declarations {
		for node := declaration; node != nil; node = node.Parent {
			if !ast.IsModuleDeclaration(node) {
				continue
			}
			name := node.Name()
			sourceFile := ast.GetSourceFileOfNode(node)
			if name == nil || sourceFile == nil {
				continue
			}
			start := scanner.SkipTrivia(sourceFile.Text(), name.Pos())
			end := name.End()
			if start < 0 || end > len(sourceFile.Text()) || start >= end {
				continue
			}
			return strings.Trim(string(sourceFile.Text()[start:end]), "\"'")
		}
	}
	return ""
}

func declarationsForSymbol(symbol *ast.Symbol) []typefacts.Declaration {
	declarations := make([]typefacts.Declaration, 0, len(symbol.Declarations))
	for _, node := range symbol.Declarations {
		sourceFile := ast.GetSourceFileOfNode(node)
		if sourceFile == nil {
			continue
		}
		nameNode := node.Name()
		if nameNode == nil {
			nameNode = node
		}
		declarations = append(declarations, typefacts.Declaration{Name: symbol.Name, Kind: declarationKind(node), Location: typefacts.Location{Path: filepath.Clean(sourceFile.FileName()), StartByte: scanner.SkipTrivia(sourceFile.Text(), nameNode.Pos()), EndByte: nameNode.End()}})
	}
	return declarations
}

func (p *project) ResolvedCall(ctx context.Context, location typefacts.Location) (typefacts.Call, error) {
	if err := ctx.Err(); err != nil {
		return typefacts.Call{}, err
	}
	p.mu.Lock()
	defer p.mu.Unlock()
	if p.closed {
		return typefacts.Call{}, ErrClosed
	}
	sourceFile, err := p.sourceFileFor(location)
	if err != nil {
		return typefacts.Call{}, err
	}
	node := ast.GetNodeAtPosition(sourceFile, location.StartByte, false)
	for node != nil && !ast.IsCallExpression(node) {
		node = node.Parent
	}
	if node == nil {
		return typefacts.Call{}, fmt.Errorf("%w: call at byte %d", typefacts.ErrNotFound, location.StartByte)
	}
	callee := node.AsCallExpression().Expression
	target := p.checker.GetSymbolAtLocation(callee)
	if target == nil {
		return typefacts.Call{}, fmt.Errorf("%w: call target at byte %d", typefacts.ErrNotFound, location.StartByte)
	}
	target = p.canonicalSymbol(target)
	signature := checker.Checker_getResolvedSignature(p.checker, node, nil, checker.CheckModeNormal)
	if signature == nil {
		return typefacts.Call{}, fmt.Errorf("%w: call signature at byte %d", typefacts.ErrNotFound, location.StartByte)
	}
	returnType := checker.Checker_getReturnTypeOfSignature(p.checker, signature)
	if returnType == nil {
		return typefacts.Call{}, fmt.Errorf("%w: return type at byte %d", typefacts.ErrNotFound, location.StartByte)
	}
	return typefacts.Call{
		Target:         p.idFor(target),
		ReturnType:     p.idForType(returnType),
		ReturnTypeText: p.checker.TypeToString(returnType),
	}, nil
}

// fileFactsMemo is one file's source-fact memo entry. Each fact set is
// stored only when every symbol identity it carries is durable, so a reused
// set never hands out an ID the current generation cannot resolve.
type fileFactsMemo struct {
	absPath      string
	calls        []typefacts.SourceCall
	hasCalls     bool
	bindings     []typefacts.SourceBinding
	hasBindings  bool
	functions    []typefacts.SourceFunction
	hasFunctions bool
	async        []typefacts.AsyncFunctionFact
	hasAsync     bool
	asyncAt      map[asyncLocationKey][]typefacts.AsyncFunctionFact
}

// memoFor returns the memo entry for a Source* path argument, keyed by the
// argument itself so memoized facts repeat the caller's own path form, with
// the normalized path retained for affected-set eviction.
func (p *project) memoFor(path string) *fileFactsMemo {
	if memo, ok := p.sourceFactsMemo[path]; ok {
		return memo
	}
	absPath, err := filepath.Abs(path)
	if err != nil {
		return nil
	}
	memo := &fileFactsMemo{absPath: filepath.Clean(absPath)}
	p.sourceFactsMemo[path] = memo
	return memo
}

func sourceCallsDurable(calls []typefacts.SourceCall) bool {
	for _, call := range calls {
		if !durableSymbolID(call.Target) {
			return false
		}
	}
	return true
}

func sourceBindingsDurable(bindings []typefacts.SourceBinding) bool {
	for _, binding := range bindings {
		if !durableSymbolID(binding.Initializer.Target) {
			return false
		}
	}
	return true
}

func (p *project) SourceCalls(ctx context.Context, path string) ([]typefacts.SourceCall, error) {
	if err := ctx.Err(); err != nil {
		return nil, err
	}
	p.mu.Lock()
	defer p.mu.Unlock()
	if p.closed {
		return nil, ErrClosed
	}
	memo := p.memoFor(path)
	if memo != nil && memo.hasCalls {
		return append([]typefacts.SourceCall(nil), memo.calls...), nil
	}
	sourceFile, err := p.sourceFileFor(typefacts.Location{Path: path})
	if err != nil {
		return nil, err
	}
	calls := make([]typefacts.SourceCall, 0)
	var visit func(*ast.Node) bool
	visit = func(node *ast.Node) bool {
		if ast.IsCallExpression(node) {
			if call, ok := p.sourceCallFact(path, sourceFile, node); ok {
				calls = append(calls, call)
			}
		}
		node.ForEachChild(visit)
		return false
	}
	for _, statement := range sourceFile.Statements.Nodes {
		visit(statement)
	}
	if memo != nil && sourceCallsDurable(calls) {
		memo.calls = calls
		memo.hasCalls = true
	}
	return calls, nil
}

func (p *project) SourceBindings(ctx context.Context, path string) ([]typefacts.SourceBinding, error) {
	if err := ctx.Err(); err != nil {
		return nil, err
	}
	p.mu.Lock()
	defer p.mu.Unlock()
	if p.closed {
		return nil, ErrClosed
	}
	memo := p.memoFor(path)
	if memo != nil && memo.hasBindings {
		return append([]typefacts.SourceBinding(nil), memo.bindings...), nil
	}
	sourceFile, err := p.sourceFileFor(typefacts.Location{Path: path})
	if err != nil {
		return nil, err
	}
	bindings := make([]typefacts.SourceBinding, 0)
	var visit func(*ast.Node) bool
	visit = func(node *ast.Node) bool {
		if ast.IsVariableDeclaration(node) {
			declaration := node.AsVariableDeclaration()
			if declaration.Initializer != nil && ast.IsCallExpression(declaration.Initializer) {
				if call, ok := p.sourceCallFact(path, sourceFile, declaration.Initializer); ok {
					array, names := bindingNameLocations(path, sourceFile.Text(), declaration.Name())
					bindings = append(bindings, typefacts.SourceBinding{Array: array, Names: names, Initializer: call})
				}
			}
		}
		node.ForEachChild(visit)
		return false
	}
	for _, statement := range sourceFile.Statements.Nodes {
		visit(statement)
	}
	if memo != nil && sourceBindingsDurable(bindings) {
		memo.bindings = bindings
		memo.hasBindings = true
	}
	return bindings, nil
}

func (p *project) SourceFunctions(ctx context.Context, path string) ([]typefacts.SourceFunction, error) {
	if err := ctx.Err(); err != nil {
		return nil, err
	}
	p.mu.Lock()
	defer p.mu.Unlock()
	if p.closed {
		return nil, ErrClosed
	}
	memo := p.memoFor(path)
	if memo != nil && memo.hasFunctions {
		return append([]typefacts.SourceFunction(nil), memo.functions...), nil
	}
	sourceFile, err := p.sourceFileFor(typefacts.Location{Path: path})
	if err != nil {
		return nil, err
	}
	functions := make([]typefacts.SourceFunction, 0)
	var visit func(*ast.Node) bool
	visit = func(node *ast.Node) bool {
		if ast.IsFunctionDeclaration(node) {
			declaration := node.AsFunctionDeclaration()
			if declaration.Name() != nil && declaration.Body != nil {
				functions = append(functions, sourceFunctionFact(path, sourceFile.Text(), declaration.Name(), declaration.Body, declaration.Parameters.Nodes, node))
			}
		} else if ast.IsVariableDeclaration(node) {
			declaration := node.AsVariableDeclaration()
			if ast.IsIdentifier(declaration.Name()) && declaration.Initializer != nil && ast.IsArrowFunction(declaration.Initializer) {
				arrow := declaration.Initializer.AsArrowFunction()
				if arrow.Body != nil && ast.IsBlock(arrow.Body) {
					owner := node
					for owner.Parent != nil && !ast.IsVariableStatement(owner) {
						owner = owner.Parent
					}
					function := sourceFunctionFact(path, sourceFile.Text(), declaration.Name(), arrow.Body, arrow.Parameters.Nodes, owner)
					function.Async = ast.HasSyntacticModifier(declaration.Initializer, ast.ModifierFlagsAsync)
					function.Arrow = true
					functions = append(functions, function)
				}
			}
		}
		node.ForEachChild(visit)
		return false
	}
	for _, statement := range sourceFile.Statements.Nodes {
		visit(statement)
	}
	if memo != nil {
		memo.functions = functions
		memo.hasFunctions = true
	}
	return functions, nil
}

func sourceFunctionFact(path, source string, name, body *ast.Node, parameters []*ast.Node, owner *ast.Node) typefacts.SourceFunction {
	parameterLocations := make([]typefacts.Location, 0, len(parameters))
	for _, parameter := range parameters {
		parameterLocations = append(parameterLocations, typefacts.Location{Path: path, StartByte: scanner.SkipTrivia(source, parameter.Pos()), EndByte: parameter.End()})
	}
	return typefacts.SourceFunction{
		Name:       typefacts.Location{Path: path, StartByte: scanner.SkipTrivia(source, name.Pos()), EndByte: name.End()},
		Body:       typefacts.Location{Path: path, StartByte: scanner.SkipTrivia(source, body.Pos()), EndByte: body.End() - 1},
		Parameters: parameterLocations,
		Exported:   ast.HasSyntacticModifier(owner, ast.ModifierFlagsExport),
		Async:      ast.HasSyntacticModifier(owner, ast.ModifierFlagsAsync),
	}
}

func (p *project) sourceCallFact(path string, sourceFile *ast.SourceFile, node *ast.Node) (typefacts.SourceCall, bool) {
	call := node.AsCallExpression()
	target := p.checker.GetSymbolAtLocation(call.Expression)
	if target == nil {
		return typefacts.SourceCall{}, false
	}
	target = p.canonicalSymbol(target)
	arguments := make([]typefacts.Location, 0, len(call.Arguments.Nodes))
	for _, argument := range call.Arguments.Nodes {
		arguments = append(arguments, typefacts.Location{Path: path, StartByte: scanner.SkipTrivia(sourceFile.Text(), argument.Pos()), EndByte: argument.End()})
	}
	return typefacts.SourceCall{
		Location:  typefacts.Location{Path: path, StartByte: scanner.SkipTrivia(sourceFile.Text(), node.Pos()), EndByte: node.End()},
		Callee:    typefacts.Location{Path: path, StartByte: scanner.SkipTrivia(sourceFile.Text(), call.Expression.Pos()), EndByte: call.Expression.End()},
		Arguments: arguments,
		Target:    p.idFor(target),
	}, true
}

func bindingNameLocations(path, source string, name *ast.Node) (bool, []typefacts.Location) {
	if name == nil {
		return false, nil
	}
	if ast.IsIdentifier(name) {
		return false, []typefacts.Location{{Path: path, StartByte: scanner.SkipTrivia(source, name.Pos()), EndByte: name.End()}}
	}
	if !ast.IsArrayBindingPattern(name) {
		return false, nil
	}
	elements := name.AsBindingPattern().Elements.Nodes
	locations := make([]typefacts.Location, len(elements))
	for index, element := range elements {
		if !ast.IsBindingElement(element) {
			continue
		}
		bound := element.AsBindingElement().Name()
		if bound != nil && ast.IsIdentifier(bound) {
			locations[index] = typefacts.Location{Path: path, StartByte: scanner.SkipTrivia(source, bound.Pos()), EndByte: bound.End()}
		}
	}
	return true, locations
}

func (p *project) sourceFileFor(location typefacts.Location) (*ast.SourceFile, error) {
	absolutePath, err := filepath.Abs(location.Path)
	if err != nil {
		return nil, fmt.Errorf("resolve source path: %w", err)
	}
	sourceFile := p.program.GetSourceFile(absolutePath)
	if sourceFile == nil {
		return nil, fmt.Errorf("%w: source file %s", typefacts.ErrNotFound, absolutePath)
	}
	if err := validateLocation(location, sourceFile.Text()); err != nil {
		return nil, err
	}
	return sourceFile, nil
}

func validateLocation(location typefacts.Location, source string) error {
	if !utf8.ValidString(source) {
		return errors.New("source is not valid UTF-8")
	}
	if location.StartByte < 0 || location.EndByte < location.StartByte || location.EndByte > len(source) {
		return fmt.Errorf("source byte range [%d,%d) is outside [0,%d)", location.StartByte, location.EndByte, len(source))
	}
	if !utf8Boundary(source, location.StartByte) || !utf8Boundary(source, location.EndByte) {
		return fmt.Errorf("source byte range [%d,%d) does not fall on UTF-8 boundaries", location.StartByte, location.EndByte)
	}
	return nil
}

func utf8Boundary(source string, offset int) bool {
	return offset == 0 || offset == len(source) || utf8.RuneStart(source[offset])
}

func (p *project) Close() error {
	p.mu.Lock()
	defer p.mu.Unlock()
	if p.closed {
		return ErrClosed
	}
	p.closed = true
	if p.release != nil {
		p.release()
	}
	p.program = nil
	p.checker = nil
	clear(p.idsBySymbol)
	clear(p.symbolsByID)
	clear(p.idsByType)
	clear(p.sourceFactsMemo)
	clear(p.durableRefs)
	p.referenceIndex.reset()
	p.filesByName = nil
	p.declarationShapes = nil
	p.exportedIdentities = nil
	return nil
}

// durableSymbolRef is the durable symbol identity: the name span of the
// symbol's first declaration plus the symbol's name. It survives program
// rebuilds while that declaration is unchanged.
type durableSymbolRef struct {
	path      string
	startByte int
	endByte   int
	name      string
}

func durableRefFor(symbol *ast.Symbol) (durableSymbolRef, bool) {
	if len(symbol.Declarations) == 0 {
		return durableSymbolRef{}, false
	}
	node := symbol.Declarations[0]
	sourceFile := ast.GetSourceFileOfNode(node)
	if sourceFile == nil {
		return durableSymbolRef{}, false
	}
	nameNode := node.Name()
	if nameNode == nil {
		nameNode = node
	}
	return durableSymbolRef{
		path:      filepath.Clean(sourceFile.FileName()),
		startByte: scanner.SkipTrivia(sourceFile.Text(), nameNode.Pos()),
		endByte:   nameNode.End(),
		name:      symbol.Name,
	}, true
}

func (ref durableSymbolRef) id() typefacts.SymbolID {
	digest := sha256.Sum256([]byte(fmt.Sprintf("%s\x00%d\x00%d\x00%s", ref.path, ref.startByte, ref.endByte, ref.name)))
	return typefacts.SymbolID("symbol:h:" + hex.EncodeToString(digest[:12]))
}

func (ref durableSymbolRef) exportedID() typefacts.SymbolID {
	digest := sha256.Sum256([]byte(fmt.Sprintf("export\x00%s\x00%s", ref.path, ref.name)))
	return typefacts.SymbolID("symbol:h:" + hex.EncodeToString(digest[:12]))
}

// durableSymbolID reports whether id can outlive the generation that minted
// it. The empty ID is durable: it is a constant, not a handle.
func durableSymbolID(id typefacts.SymbolID) bool {
	return typefacts.DurableSymbolID(id)
}

func (p *project) idFor(symbol *ast.Symbol) typefacts.SymbolID {
	if id, ok := p.idsBySymbol[symbol]; ok {
		return id
	}
	if exported, ok := p.exportedIdentities[symbol]; ok {
		if existing, taken := p.symbolsByID[exported.id]; !taken || existing == symbol {
			p.idsBySymbol[symbol] = exported.id
			p.symbolsByID[exported.id] = symbol
			p.durableRefs[exported.id] = exported.ref
			return exported.id
		}
	}
	if ref, ok := durableRefFor(symbol); ok {
		id := ref.id()
		if existing, taken := p.symbolsByID[id]; !taken || existing == symbol {
			p.idsBySymbol[symbol] = id
			p.symbolsByID[id] = symbol
			p.durableRefs[id] = ref
			return id
		}
	}
	p.nextSymbol++
	id := typefacts.SymbolID(fmt.Sprintf("symbol:%d:%d", p.generation, p.nextSymbol))
	p.idsBySymbol[symbol] = id
	p.symbolsByID[id] = symbol
	return id
}

// symbolFor resolves id in the current generation, lazily re-resolving a
// durable ID minted in an earlier generation through its declaration. A
// failed re-resolution reports not-found, exactly as a stale ID always has.
func (p *project) symbolFor(id typefacts.SymbolID) (*ast.Symbol, bool) {
	if symbol, ok := p.symbolsByID[id]; ok {
		return symbol, true
	}
	ref, ok := p.durableRefs[id]
	if !ok {
		return nil, false
	}
	sourceFile := p.program.GetSourceFile(ref.path)
	if sourceFile == nil {
		if p.filesByName == nil {
			p.filesByName = make(map[string]*ast.SourceFile)
			for _, file := range p.program.SourceFiles() {
				p.filesByName[filepath.Clean(file.FileName())] = file
			}
		}
		sourceFile = p.filesByName[ref.path]
	}
	if sourceFile == nil || ref.startByte >= ref.endByte || ref.endByte > len(sourceFile.Text()) {
		return nil, false
	}
	node := deepestNodeAt(ast.GetNodeAtPosition(sourceFile, ref.startByte, false), ref.startByte)
	if node == nil {
		return nil, false
	}
	symbol := p.checker.GetSymbolAtLocation(node)
	if symbol == nil {
		return nil, false
	}
	if resolved, ok := durableRefFor(symbol); !ok || resolved != ref {
		return nil, false
	}
	if canonical, ok := p.idsBySymbol[symbol]; ok && canonical != id {
		// A shape-equivalent update may deliberately preserve an older
		// canonical ID for this symbol. A historical span-derived ID must
		// not displace that choice if it is queried later.
		return nil, false
	}
	p.idsBySymbol[symbol] = id
	p.symbolsByID[id] = symbol
	return symbol, true
}

func (p *project) canonicalSymbol(symbol *ast.Symbol) *ast.Symbol {
	if symbol.Flags&ast.SymbolFlagsAlias == 0 {
		return symbol
	}
	if original := p.checker.GetAliasedSymbol(symbol); original != nil {
		return original
	}
	return symbol
}

func (p *project) idForType(value *checker.Type) typefacts.TypeID {
	if id, ok := p.idsByType[value]; ok {
		return id
	}
	p.nextType++
	id := typefacts.TypeID(fmt.Sprintf("type:%d:%d", p.generation, p.nextType))
	p.idsByType[value] = id
	return id
}

func declarationKind(node *ast.Node) string {
	switch {
	case ast.IsVariableDeclaration(node):
		return "variable"
	case ast.IsFunctionDeclaration(node):
		return "function"
	case ast.IsClassDeclaration(node):
		return "class"
	default:
		return "declaration"
	}
}
