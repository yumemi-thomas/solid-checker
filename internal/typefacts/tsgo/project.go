// Package tsgo adapts the pinned tsgolint/typescript-go integration to the
// compiler-independent typefacts seam. No shim or compiler types escape it.
package tsgo

import (
	"context"
	"crypto/sha256"
	"encoding/hex"
	"errors"
	"fmt"
	"path"
	"path/filepath"
	"slices"
	"sort"
	"strconv"
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

	"github.com/yumemi-thomas/solid-ts-facts/internal/typefacts"
)

var ErrClosed = errors.New("type facts project is closed")

var _ typefacts.Project = (*project)(nil)

type project struct {
	mu             sync.Mutex
	trace          typefacts.Trace
	configPath     string
	fs             *overlayFS
	versions       map[string]uint64
	program        *compiler.Program
	checker        *checker.Checker
	checkerPool    *singleCheckerPool
	release        func()
	closed         bool
	generation     uint64
	nextSymbol     uint64
	idsBySymbol    map[*ast.Symbol]typefacts.SymbolID
	symbolsByID    map[typefacts.SymbolID]*ast.Symbol
	referenceIndex referenceIndex
	// sourceFactsMemo carries per-file Source* facts across generations. An
	// entry is stored only when every symbol identity it contains is durable,
	// so its facts stay resolvable after an update. Update drops the affected
	// set on the incremental path and clears the memo on full rebuilds.
	sourceFactsMemo map[string]*fileFactsMemo
	// durableRefs maps a durable SymbolID back to its declaration so the
	// symbol can be re-resolved lazily in a later generation. Generation-
	// scoped pointer caches prevent repeat hashing within an analysis; keeping
	// the inverse declaration-to-ID map duplicated every durable identity for
	// the entire session.
	durableRefs map[typefacts.SymbolID]durableSymbolRef
	// filesByName is a generation-scoped index of program files keyed by
	// their cleaned file name. Program.GetSourceFile does not round-trip
	// virtual bundled-lib names (bundled:/… is resolved against the working
	// directory), so durable re-resolution of lib-declared symbols falls
	// back to this index. Nil until the first fallback of a generation.
	filesByName map[string]*ast.SourceFile
	// currentSourceFiles validates checker-owned declaration nodes without a
	// canonical path lookup per resolved call. A stale incremental node is
	// absent and takes the current-declaration remapping path.
	currentSourceFiles map[*ast.SourceFile]struct{}
	// resolved-call caches are generation-scoped and populated only by
	// resolvedCall demands. Signatures and symbols are checker-owned pointers,
	// so every accepted update drops the maps as it installs the new checker.
	resolvedDeclarations map[resolvedDeclarationCacheKey]*typefacts.ResolvedDeclaration
	resolvedParameters   map[resolvedParameterCacheKey]*typefacts.ParameterFact
	callDiagnostics      map[*ast.SourceFile]callDiagnosticIndex
	callDemandScratch    []resolvedCallDemand
	// typeDescriptors interns compiler-identical instantiated and return
	// types. TypeToString is presentation work, not a semantic decision, and
	// is disproportionately allocation-heavy when repeated per call.
	typeDescriptors map[*checker.Type]*typefacts.TypeDescriptor
	// declarationShapes caches diagnostic-free exported contracts for the
	// accepted program generation. Incremental updates need only emit the
	// candidate generation's shape; semantically affected files are evicted
	// and broad rebuilds clear the cache.
	declarationShapes map[string]declarationShape
	// exportedIdentities assigns module-visible target symbols a
	// span-insensitive identity derived from declaring path and symbol name.
	// Module-scope export uniqueness makes this deterministic across process
	// restart while nested/non-exported symbols keep span-based identities.
	exportedIdentities      map[*ast.Symbol]preservedExportIdentity
	exportedIdentitiesByRef map[durableSymbolRef]preservedExportIdentity
	// sourceDigests carries compact source identities across generations.
	// Incremental programs reuse unchanged AST nodes, so unchanged files pay
	// neither a text copy nor another hash. Raw bodies are materialized only
	// for the explicit LifecycleSources operation and are never retained.
	sourceDigests map[*ast.SourceFile][sha256.Size]byte
	// The retained module graph of the accepted program, which the affected
	// walk consumes instead of re-resolving every import edge per update.
	// forwardDeps holds each non-declaration file's cleaned resolved
	// dependencies and reverseDeps the inverted index. A content edit whose
	// resolved edges are unchanged carries the graph directly; anything that
	// can change membership rebuilds it.
	forwardDeps map[string][]string
	reverseDeps map[string]map[string]struct{}
}

// OpenProject loads and binds the TypeScript project at configPath.
// OpenProject opens one configured project. trace may be nil, which disables
// backend tracing.
func OpenProject(ctx context.Context, configPath string, trace typefacts.Trace) (typefacts.Project, error) {
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
		trace:           trace,
		configPath:      absConfigPath,
		fs:              fs,
		versions:        make(map[string]uint64),
		program:         program,
		checker:         typeChecker,
		checkerPool:     program.GetCheckerPool().(*singleCheckerPool),
		release:         release,
		generation:      1,
		idsBySymbol:     make(map[*ast.Symbol]typefacts.SymbolID),
		symbolsByID:     make(map[typefacts.SymbolID]*ast.Symbol),
		sourceFactsMemo: make(map[string]*fileFactsMemo),
		durableRefs:     make(map[typefacts.SymbolID]durableSymbolRef),
	}
	opened.exportedIdentities = collectExportedIdentities(program, typeChecker)
	opened.exportedIdentitiesByRef = indexExportedIdentitiesByRef(opened.exportedIdentities)
	opened.rebuildImportGraph(program)
	return opened, nil
}

// TypeScript paths always use forward slashes, including on Windows. The
// underlying OS filesystem accepts them, while TypeScript-Go's path and VFS
// helpers do not consistently treat backslashes as directory separators.
func normalizeTypeScriptPath(path string) string {
	path = strings.ReplaceAll(path, `\`, "/")
	if strings.HasPrefix(path, "//?/UNC/") {
		return "//" + strings.TrimPrefix(path, "//?/UNC/")
	}
	return strings.TrimPrefix(path, "//?/")
}

func typeScriptPathDir(fileName string) string {
	return path.Dir(normalizeTypeScriptPath(fileName))
}

// singleCheckerPool serves this adapter's one retained checker (ADR 0004).
// UpdateProgram inherits it through program options, so incremental updates
// construct one checker instead of the default pool's four. The pool and the
// project mutex are one decision: the non-exclusive file-affine lease below is
// safe only because every entry point is already serialized.
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

func (p *singleCheckerPool) drop() {
	p.once = sync.Once{}
	p.checker = nil
	p.lock = nil
}

func (p *project) ensureCheckerLocked(ctx context.Context) error {
	if p.checker != nil {
		return nil
	}
	typeChecker, release := p.program.GetTypeChecker(ctx)
	if typeChecker == nil {
		if release != nil {
			release()
		}
		return errors.New("create TypeScript checker")
	}
	p.checker = typeChecker
	p.release = release
	return nil
}

// ReleaseAnalysisState drops checker-expanded query state once its immutable
// facts have transferred into the closure. Pointer-keyed identity maps must go
// with the checker: retaining them both pins checker arenas and prevents a
// recreated equivalent symbol from reclaiming its durable ID. Durable
// declaration refs, retained source facts, and compact reference
// contributions are pointer-free and remain the rehydration boundary.
func (p *project) ReleaseAnalysisState() {
	p.mu.Lock()
	defer p.mu.Unlock()
	if p.closed || p.checker == nil {
		return
	}
	if p.release != nil {
		p.release()
		p.release = nil
	}
	p.checker = nil
	p.checkerPool.drop()
	p.idsBySymbol = make(map[*ast.Symbol]typefacts.SymbolID)
	p.symbolsByID = make(map[typefacts.SymbolID]*ast.Symbol)
	p.exportedIdentities = nil
	p.filesByName = nil
	p.currentSourceFiles = nil
	p.resolvedDeclarations = nil
	p.resolvedParameters = nil
	p.callDiagnostics = nil
	p.callDemandScratch = nil
	p.typeDescriptors = nil
	p.nextSymbol = 0
	p.referenceIndex.releaseAnalysisState()
}

func buildProgram(ctx context.Context, configPath string, fs vfs.FS) (*compiler.Program, *checker.Checker, func(), error) {
	cwd := typeScriptPathDir(configPath)
	host := compiler.NewCompilerHost(cwd, fs, bundled.LibPath(), nil, nil)
	config, diagnostics := tsoptions.GetParsedCommandLineOfConfigFile(configPath, &core.CompilerOptions{}, nil, host, nil)
	if len(diagnostics) != 0 {
		return nil, nil, nil, fmt.Errorf("parse tsconfig: %s", formatDiagnostics(diagnostics))
	}
	if config == nil {
		return nil, nil, nil, errors.New("parse tsconfig: no configuration returned")
	}
	if len(config.Errors) != 0 {
		return nil, nil, nil, fmt.Errorf("parse tsconfig: %d configuration error(s)", len(config.Errors))
	}

	program := compiler.NewProgram(compiler.ProgramOptions{
		Config: config,
		// Parse and bind in parallel, but keep exactly one checker (ADR 0004).
		// SingleThreaded would also yield one checker, but it serializes parse
		// and bind — the phases that do scale — so the custom pool is what
		// keeps both properties.
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
	host := compiler.NewCompilerHost(typeScriptPathDir(configPath), fs, bundled.LibPath(), nil, nil)
	program, _, _ := oldProgram.UpdateProgram(oldFile.Path(), host, nil)
	if program == nil {
		return nil, nil, nil, errors.New("update TypeScript program")
	}
	return finishProgram(ctx, program)
}

func formatDiagnostics(diagnostics []*ast.Diagnostic) string {
	messages := make([]string, len(diagnostics))
	for index, diagnostic := range diagnostics {
		messages[index] = fmt.Sprintf("TS%d: %s", diagnostic.Code(), diagnostic.String())
	}
	return strings.Join(messages, "; ")
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
	programFiles := p.program.SourceFiles()
	files := make([]typefacts.SourceFile, 0, len(programFiles))
	for _, sourceFile := range programFiles {
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

// SourceDigests is the semantic table's compact source inventory. Rust owns
// the retained source rows; Go keeps only this AST-node memo so incremental
// analyses do not re-hash unchanged text.
func (p *project) SourceDigests(ctx context.Context) ([]typefacts.SourceDigest, error) {
	if err := ctx.Err(); err != nil {
		return nil, err
	}
	p.mu.Lock()
	defer p.mu.Unlock()
	if p.closed {
		return nil, ErrClosed
	}
	programFiles := p.program.SourceFiles()
	digests := make([]typefacts.SourceDigest, 0, len(programFiles))
	byFile := make(map[*ast.SourceFile][sha256.Size]byte, len(programFiles))
	for _, sourceFile := range programFiles {
		if sourceFile.IsDeclarationFile {
			continue
		}
		digest, cached := p.sourceDigests[sourceFile]
		if !cached {
			digest = sha256.Sum256([]byte(sourceFile.Text()))
		}
		byFile[sourceFile] = digest
		digests = append(digests, typefacts.SourceDigest{
			Path:   filepath.Clean(sourceFile.FileName()),
			SHA256: digest,
		})
	}
	p.sourceDigests = byFile
	sort.Slice(digests, func(i, j int) bool { return digests[i].Path < digests[j].Path })
	return digests, nil
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
	if err := p.ensureCheckerLocked(ctx); err != nil {
		return typefacts.AffectedSet{}, err
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
	noImporters := incremental && incrementalPath != "" && len(p.reverseDeps[incrementalPath]) == 0
	stageStarted = time.Now()
	if incremental && incrementalPath != "" && !noImporters {
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
	leafCutoff := false
	var newShape declarationShape
	var newShapeOK bool
	var currentExports map[*ast.Symbol]preservedExportIdentity
	currentExportsKnown := false
	stageStarted = time.Now()
	if noImporters {
		sourceFile := program.GetSourceFile(incrementalPath)
		if sourceFile != nil && ast.IsExternalModule(sourceFile) && !hasGlobalOrModuleAugmentation(sourceFile) {
			if exports, ok := exportedDurableSymbols(typeChecker, sourceFile); ok {
				currentExports = declarationExportIdentities(declarationShape{exports: exports})
				currentExportsKnown = true
				semanticCutoff = true
				leafCutoff = true
			}
		}
	}
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
			currentExportsKnown = true
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
	p.checkerPool = program.GetCheckerPool().(*singleCheckerPool)
	p.release = release
	p.fs = candidateFS
	p.versions = candidateVersions
	p.generation++
	if incremental && incrementalPath != "" && currentExportsKnown {
		if p.exportedIdentities == nil {
			p.exportedIdentities = make(map[*ast.Symbol]preservedExportIdentity)
		}
		for ref := range p.exportedIdentitiesByRef {
			if ref.path == incrementalPath {
				delete(p.exportedIdentitiesByRef, ref)
			}
		}
		for symbol, identity := range currentExports {
			p.exportedIdentities[symbol] = identity
			p.exportedIdentitiesByRef[identity.ref] = identity
		}
	} else {
		p.exportedIdentities = collectExportedIdentities(program, typeChecker)
		p.exportedIdentitiesByRef = indexExportedIdentitiesByRef(p.exportedIdentities)
	}
	clear(p.idsBySymbol)
	clear(p.symbolsByID)
	for symbol, preserved := range currentExports {
		p.idsBySymbol[symbol] = preserved.id
		p.symbolsByID[preserved.id] = symbol
		p.durableRefs[preserved.id] = preserved.ref
	}
	p.filesByName = nil
	p.currentSourceFiles = nil
	p.resolvedDeclarations = nil
	p.resolvedParameters = nil
	p.callDiagnostics = nil
	p.callDemandScratch = nil
	p.typeDescriptors = nil
	p.nextSymbol = 0

	stageStarted = time.Now()
	// The retained graph must advance on every accepted update — even ones
	// whose affected set is decided without a walk — or a later generation
	// walks stale edges. oldReverse keeps the pre-update edges alive for the
	// rebuild path, where the union of both generations' graphs preserves the
	// prior behaviour: an importer whose edge only the old program knew (a
	// deleted dependency, a redirected resolution) still lands in the set.
	oldReverse := p.reverseDeps
	graphPatched := false
	if incremental && incrementalPath != "" {
		graphPatched = p.patchImportGraph(program, incrementalPath)
	}
	if !graphPatched {
		p.rebuildImportGraph(program)
	}
	var affected []string
	switch {
	case semanticCutoff:
		// A diagnostic-free declaration emit proves that the module's
		// exported TypeScript shape is unchanged, and every external export
		// slot was paired bijectively with its prior canonical ID. Retained
		// importer facts can therefore keep those IDs even when declaration
		// spans inside the edited module moved.
		affected = append([]string(nil), changedPaths...)
	case globalScopeChange(changedPaths, oldProgram, program):
		// A change inside the shared global scope can be referenced from
		// anywhere with no import edge to follow. Fail closed, exactly as
		// the multi-file and delete paths do.
		affected = everySourcePath(oldProgram, program)
	case graphPatched:
		// A content edit changes only the edited file's own outgoing edges,
		// and a walk rooted at that file never traverses them, so the
		// patched graph alone already equals the two-generation union.
		affected = affectedFromReverseDeps(changedPaths, p.reverseDeps)
	default:
		affected = affectedFromReverseDeps(changedPaths, oldReverse, p.reverseDeps)
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
		p.sweepDurableIdentities(program)
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
	if p.trace != nil {
		p.trace.Metrics("update",
			typefacts.Nanos("totalNs", time.Since(updateStarted)),
			typefacts.Nanos("overlayNs", overlayDuration),
			typefacts.Nanos("oldShapeNs", oldShapeDuration),
			typefacts.Flag("oldShapeCached", oldShapeCached),
			typefacts.Nanos("programNs", programDuration),
			typefacts.Nanos("newShapeNs", newShapeDuration),
			typefacts.Flag("leafCutoff", leafCutoff),
			typefacts.Nanos("affectedNs", affectedDuration),
			typefacts.Nanos("invalidationNs", invalidationDuration))
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
		TargetSourceFiles: []*ast.SourceFile{sourceFile},
		EmitOnly:          compiler.EmitOnlyForcedDts,
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

func indexExportedIdentitiesByRef(
	identities map[*ast.Symbol]preservedExportIdentity,
) map[durableSymbolRef]preservedExportIdentity {
	byRef := make(map[durableSymbolRef]preservedExportIdentity, len(identities))
	for _, identity := range identities {
		byRef[identity.ref] = identity
	}
	return byRef
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

// globalScopeChange reports whether any changed file participates in the shared
// global scope rather than being an external module. Script-kind files (no
// imports and no exports) and global declaration files are referenced without
// any import edge, so the reverse-dependency walk in affectedFiles cannot find
// their referencing files.
func globalScopeChange(changedPaths []string, programs ...*compiler.Program) bool {
	for _, path := range changedPaths {
		path = filepath.Clean(path)
		for _, program := range programs {
			sourceFile := program.GetSourceFile(path)
			if sourceFile == nil {
				continue
			}
			if !ast.IsExternalModule(sourceFile) {
				return true
			}
		}
	}
	return false
}

func everySourcePath(programs ...*compiler.Program) []string {
	paths := make(map[string]struct{})
	for _, program := range programs {
		for _, sourceFile := range program.SourceFiles() {
			paths[filepath.Clean(sourceFile.FileName())] = struct{}{}
		}
	}
	result := make([]string, 0, len(paths))
	for path := range paths {
		result = append(result, path)
	}
	return result
}

// moduleEdges resolves one file's import specifiers to cleaned dependency
// paths. Duplicate specifiers yield duplicate entries, which every consumer
// tolerates.
func moduleEdges(program *compiler.Program, sourceFile *ast.SourceFile) []string {
	imports := sourceFile.Imports()
	if len(imports) == 0 {
		return nil
	}
	edges := make([]string, 0, len(imports))
	for _, specifier := range imports {
		resolved := program.GetResolvedModuleFromModuleSpecifier(sourceFile, specifier)
		if resolved == nil {
			continue
		}
		edges = append(edges, filepath.Clean(resolved.ResolvedFileName))
	}
	return edges
}

func (p *project) addImportEdges(importer string, dependencies []string) {
	for _, dependency := range dependencies {
		importers := p.reverseDeps[dependency]
		if importers == nil {
			importers = make(map[string]struct{})
			p.reverseDeps[dependency] = importers
		}
		importers[importer] = struct{}{}
	}
}

// rebuildImportGraph resolves every non-declaration file's imports and
// replaces the retained graph. This is the O(project) path; ordinary edits go
// through patchImportGraph instead.
func (p *project) rebuildImportGraph(program *compiler.Program) {
	sourceFiles := program.SourceFiles()
	p.forwardDeps = make(map[string][]string, len(sourceFiles))
	p.reverseDeps = make(map[string]map[string]struct{}, len(sourceFiles))
	for _, sourceFile := range sourceFiles {
		if sourceFile.IsDeclarationFile {
			continue
		}
		importer := filepath.Clean(sourceFile.FileName())
		edges := moduleEdges(program, sourceFile)
		if len(edges) != 0 {
			p.forwardDeps[importer] = edges
		}
		p.addImportEdges(importer, edges)
	}
}

// patchImportGraph proves the common content-only edit from the edited file's
// own resolved edges. Equal old/new edges imply unchanged program membership:
// with config and filesystem membership fixed, only a changed import can pull
// a file in or let one leave. Edge changes take the conservative full rebuild.
func (p *project) patchImportGraph(program *compiler.Program, path string) bool {
	path = filepath.Clean(path)
	edited := program.GetSourceFile(path)
	if edited == nil || edited.IsDeclarationFile {
		return false
	}
	edges := moduleEdges(program, edited)
	return slices.Equal(edges, p.forwardDeps[path])
}

// affectedFromReverseDeps walks the reverse-dependency indexes from the
// changed paths to a fixed point. Retention rests on the premise that a
// changed declaring file puts every referencing file into the affected set,
// which import edges establish only for external modules; callers guard the
// shared-global-scope case before walking.
func affectedFromReverseDeps(changedPaths []string, graphs ...map[string]map[string]struct{}) []string {
	affected := make(map[string]struct{}, len(changedPaths))
	queue := make([]string, 0, len(changedPaths))
	for _, path := range changedPaths {
		path = filepath.Clean(path)
		affected[path] = struct{}{}
		queue = append(queue, path)
	}
	for len(queue) != 0 {
		dependency := queue[0]
		queue = queue[1:]
		for _, graph := range graphs {
			for importer := range graph[dependency] {
				if _, seen := affected[importer]; seen {
					continue
				}
				affected[importer] = struct{}{}
				queue = append(queue, importer)
			}
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
	if err := p.ensureCheckerLocked(ctx); err != nil {
		return "", err
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
	if err := p.ensureCheckerLocked(ctx); err != nil {
		return "", err
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
	if err := p.ensureCheckerLocked(ctx); err != nil {
		return nil, err
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

func (p *project) DescribeTypeAt(ctx context.Context, location typefacts.Location) (typefacts.TypeDescriptor, error) {
	if err := ctx.Err(); err != nil {
		return typefacts.TypeDescriptor{}, err
	}
	p.mu.Lock()
	defer p.mu.Unlock()
	if p.closed {
		return typefacts.TypeDescriptor{}, ErrClosed
	}
	if err := p.ensureCheckerLocked(ctx); err != nil {
		return typefacts.TypeDescriptor{}, err
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
	return *p.typeDescriptorFor(value), nil
}

func declarationModule(symbol *ast.Symbol) string {
	for _, declaration := range symbol.Declarations {
		for node := declaration; node != nil; node = node.Parent {
			if !ast.IsModuleDeclaration(node) {
				continue
			}
			name := node.Name()
			if name == nil {
				continue
			}
			return name.Text()
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
	if err := p.ensureCheckerLocked(ctx); err != nil {
		return nil, err
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
	if err := p.ensureCheckerLocked(ctx); err != nil {
		return nil, err
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
	clear(p.sourceFactsMemo)
	clear(p.durableRefs)
	p.referenceIndex.reset()
	p.filesByName = nil
	p.currentSourceFiles = nil
	p.resolvedDeclarations = nil
	p.resolvedParameters = nil
	p.callDiagnostics = nil
	p.callDemandScratch = nil
	p.typeDescriptors = nil
	p.declarationShapes = nil
	p.exportedIdentities = nil
	p.exportedIdentitiesByRef = nil
	return nil
}

// sweepDurableIdentities drops durable-identity bookkeeping for files the
// program no longer contains. This is the one eviction that cannot change
// behavior: an absent file's declarations cannot re-resolve regardless, and a
// file that re-enters the program re-mints identical entries through the
// location scans that precede any by-id query. It runs on the full-rebuild
// update path — deletes and config changes are where files leave wholesale.
// Identities whose declaring file stays in the program are never dropped,
// even when the file is affected: recomputing an edited file's facts resolves
// enqueued symbols by ID, and evicting those entries would silently cost
// their declarations. So churn within living files still accretes (one entry
// per distinct declaration span ever seen), which is the accepted bound.
func (p *project) sweepDurableIdentities(program *compiler.Program) {
	sourceFiles := program.SourceFiles()
	present := make(map[string]struct{}, len(sourceFiles))
	for _, sourceFile := range sourceFiles {
		present[filepath.Clean(sourceFile.FileName())] = struct{}{}
	}
	for id, ref := range p.durableRefs {
		if _, ok := present[ref.path]; !ok {
			delete(p.durableRefs, id)
		}
	}
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
	return durableRefForDeclaration(symbol, symbol.Declarations[0])
}

func durableRuntimeRefFor(symbol *ast.Symbol) (durableSymbolRef, bool) {
	if symbol == nil || symbol.ValueDeclaration == nil {
		return durableSymbolRef{}, false
	}
	return durableRefForDeclaration(symbol, symbol.ValueDeclaration)
}

func durableRefForDeclaration(symbol *ast.Symbol, node *ast.Node) (durableSymbolRef, bool) {
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

// hashedSymbolID renders a digest prefix as the wire ID in one allocation.
func hashedSymbolID(digest [sha256.Size]byte) typefacts.SymbolID {
	var id [9 + 24]byte
	copy(id[:], "symbol:h:")
	hex.Encode(id[9:], digest[:12])
	return typefacts.SymbolID(id[:])
}

func (ref durableSymbolRef) id() typefacts.SymbolID {
	// The digest input is byte-identical to the historical
	// fmt.Sprintf("%s\x00%d\x00%d\x00%s", ...) — durable IDs persist across
	// sessions, so the rendering may never drift.
	input := make([]byte, 0, len(ref.path)+len(ref.name)+32)
	input = append(input, ref.path...)
	input = append(input, 0)
	input = strconv.AppendInt(input, int64(ref.startByte), 10)
	input = append(input, 0)
	input = strconv.AppendInt(input, int64(ref.endByte), 10)
	input = append(input, 0)
	input = append(input, ref.name...)
	return hashedSymbolID(sha256.Sum256(input))
}

func (ref durableSymbolRef) exportedID() typefacts.SymbolID {
	// Byte-identical to the historical fmt.Sprintf("export\x00%s\x00%s", ...).
	input := make([]byte, 0, len(ref.path)+len(ref.name)+16)
	input = append(input, "export\x00"...)
	input = append(input, ref.path...)
	input = append(input, 0)
	input = append(input, ref.name...)
	return hashedSymbolID(sha256.Sum256(input))
}

func (ref durableSymbolRef) runtimeID() typefacts.RuntimeSymbolID {
	path := ref.path
	if resolved, err := filepath.EvalSymlinks(path); err == nil {
		path = filepath.Clean(resolved)
	}
	input := make([]byte, 0, len(path)+len(ref.name)+32)
	input = append(input, path...)
	input = append(input, 0)
	input = strconv.AppendInt(input, int64(ref.startByte), 10)
	input = append(input, 0)
	input = strconv.AppendInt(input, int64(ref.endByte), 10)
	input = append(input, 0)
	input = append(input, ref.name...)
	id := string(hashedSymbolID(sha256.Sum256(input)))
	return typefacts.RuntimeSymbolID("runtime:h:" + strings.TrimPrefix(id, "symbol:h:"))
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
		// Checker rebuilds can expose an equivalent declaration through a
		// different symbol pointer from the one GetExportsOfModule returned.
		// Pointer-only lookup made identity depend on whether an intermediate
		// generation happened to populate retained source facts. The durable
		// declaration ref is the cross-generation proof and therefore the
		// authoritative secondary key.
		if exported, ok := p.exportedIdentitiesByRef[ref]; ok {
			if existing, taken := p.symbolsByID[exported.id]; !taken || existing == symbol {
				p.idsBySymbol[symbol] = exported.id
				p.symbolsByID[exported.id] = symbol
				p.durableRefs[exported.id] = ref
				return exported.id
			}
		}
		// Pointer-keyed maps avoid repeat hashing within a generation. Across
		// checker recreation only changed/recomputed symbols reach this path,
		// so hashing those sparse refs is cheaper than retaining a complete
		// inverse map alongside durableRefs.
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
	// GetAliasedSymbol normally resolves the complete chain. The bounded loop
	// also handles compiler versions that expose one hop without allocating a
	// visited map on every identifier in the retained reference scan.
	for range 64 {
		if symbol == nil || symbol.Flags&ast.SymbolFlagsAlias == 0 {
			break
		}
		original := p.checker.GetAliasedSymbol(symbol)
		if original == nil || original == symbol {
			break
		}
		symbol = original
	}
	return symbol
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
