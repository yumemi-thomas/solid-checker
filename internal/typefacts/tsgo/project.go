// Package tsgo adapts the pinned tsgolint/typescript-go integration to the
// compiler-independent typefacts seam. No shim or compiler types escape it.
package tsgo

import (
	"context"
	"errors"
	"fmt"
	"path/filepath"
	"sort"
	"sync"

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
	mu          sync.Mutex
	configPath  string
	fs          *overlayFS
	versions    map[string]uint64
	program     *compiler.Program
	checker     *checker.Checker
	release     func()
	closed      bool
	nextSymbol  uint64
	idsBySymbol map[*ast.Symbol]typefacts.SymbolID
	symbolsByID map[typefacts.SymbolID]*ast.Symbol
	nextType    uint64
	idsByType   map[*checker.Type]typefacts.TypeID
}

// OpenProject loads and binds the TypeScript project at configPath.
func OpenProject(ctx context.Context, configPath string) (typefacts.Project, error) {
	absConfigPath, err := filepath.Abs(configPath)
	if err != nil {
		return nil, fmt.Errorf("resolve tsconfig path: %w", err)
	}
	fs := newOverlayFS(bundled.WrapFS(osvfs.FS()))
	program, typeChecker, release, err := buildProgram(ctx, absConfigPath, fs)
	if err != nil {
		return nil, err
	}

	return &project{
		configPath:  absConfigPath,
		fs:          fs,
		versions:    make(map[string]uint64),
		program:     program,
		checker:     typeChecker,
		release:     release,
		idsBySymbol: make(map[*ast.Symbol]typefacts.SymbolID),
		symbolsByID: make(map[typefacts.SymbolID]*ast.Symbol),
		idsByType:   make(map[*checker.Type]typefacts.TypeID),
	}, nil
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
		Config:                      config,
		SingleThreaded:              core.TSTrue,
		Host:                        host,
		UseSourceOfProjectReference: true,
	})
	if program == nil {
		return nil, nil, nil, errors.New("create TypeScript program")
	}
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

func (p *project) SourceFiles(_ context.Context) ([]typefacts.SourceFile, error) {
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
	p.mu.Lock()
	defer p.mu.Unlock()
	if p.closed {
		return typefacts.AffectedSet{}, ErrClosed
	}

	changedPaths := make([]string, 0, len(changes))
	for _, change := range changes {
		absolutePath, err := filepath.Abs(change.Path)
		if err != nil {
			return typefacts.AffectedSet{}, fmt.Errorf("resolve changed path: %w", err)
		}
		if version, ok := p.versions[absolutePath]; ok && change.Version <= version {
			continue
		}
		p.versions[absolutePath] = change.Version
		if change.Deleted {
			p.fs.delete(absolutePath)
		} else {
			p.fs.set(absolutePath, string(change.Source))
		}
		changedPaths = append(changedPaths, absolutePath)
	}
	if len(changedPaths) == 0 {
		return typefacts.AffectedSet{Files: []string{}}, nil
	}

	oldProgram := p.program
	program, typeChecker, release, err := buildProgram(ctx, p.configPath, p.fs)
	if err != nil {
		return typefacts.AffectedSet{}, err
	}
	if p.release != nil {
		p.release()
	}
	p.program = program
	p.checker = typeChecker
	p.release = release
	clear(p.idsBySymbol)
	clear(p.symbolsByID)
	clear(p.idsByType)
	p.nextSymbol = 0
	p.nextType = 0

	affected := affectedFiles(changedPaths, oldProgram, program)
	sort.Strings(affected)
	return typefacts.AffectedSet{Files: affected}, nil
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

func (p *project) SymbolAt(_ context.Context, location typefacts.Location) (typefacts.SymbolID, error) {
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

func (p *project) ResolveAlias(_ context.Context, id typefacts.SymbolID) (typefacts.SymbolID, error) {
	p.mu.Lock()
	defer p.mu.Unlock()
	if p.closed {
		return "", ErrClosed
	}
	symbol, ok := p.symbolsByID[id]
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

func (p *project) Declarations(_ context.Context, id typefacts.SymbolID) ([]typefacts.Declaration, error) {
	p.mu.Lock()
	defer p.mu.Unlock()
	if p.closed {
		return nil, ErrClosed
	}
	symbol, ok := p.symbolsByID[id]
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

func (p *project) References(_ context.Context, id typefacts.SymbolID) ([]typefacts.Location, error) {
	p.mu.Lock()
	defer p.mu.Unlock()
	if p.closed {
		return nil, ErrClosed
	}
	target, ok := p.symbolsByID[id]
	if !ok {
		return nil, fmt.Errorf("%w: symbol %s", typefacts.ErrNotFound, id)
	}
	target = p.canonicalSymbol(target)

	var references []typefacts.Location
	for _, sourceFile := range p.program.SourceFiles() {
		if sourceFile.IsDeclarationFile {
			continue
		}
		var visit func(*ast.Node) bool
		visit = func(node *ast.Node) bool {
			if ast.IsIdentifier(node) && !ast.IsDeclarationNameOrImportPropertyName(node) {
				symbol := p.checker.GetSymbolAtLocation(node)
				if symbol != nil && p.canonicalSymbol(symbol) == target {
					references = append(references, typefacts.Location{
						Path:      filepath.Clean(sourceFile.FileName()),
						StartByte: scanner.SkipTrivia(sourceFile.Text(), node.Pos()),
						EndByte:   node.End(),
					})
				}
			}
			node.ForEachChild(visit)
			return false
		}
		for _, statement := range sourceFile.Statements.Nodes {
			visit(statement)
		}
	}
	sort.Slice(references, func(i, j int) bool {
		if references[i].Path != references[j].Path {
			return references[i].Path < references[j].Path
		}
		return references[i].StartByte < references[j].StartByte
	})
	return references, nil
}

func (p *project) TypeAt(_ context.Context, location typefacts.Location) (typefacts.TypeID, error) {
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

func (p *project) ResolvedCall(_ context.Context, location typefacts.Location) (typefacts.Call, error) {
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

func (p *project) sourceFileFor(location typefacts.Location) (*ast.SourceFile, error) {
	absolutePath, err := filepath.Abs(location.Path)
	if err != nil {
		return nil, fmt.Errorf("resolve source path: %w", err)
	}
	sourceFile := p.program.GetSourceFile(absolutePath)
	if sourceFile == nil {
		return nil, fmt.Errorf("%w: source file %s", typefacts.ErrNotFound, absolutePath)
	}
	return sourceFile, nil
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
	return nil
}

func (p *project) idFor(symbol *ast.Symbol) typefacts.SymbolID {
	if id, ok := p.idsBySymbol[symbol]; ok {
		return id
	}
	p.nextSymbol++
	id := typefacts.SymbolID(fmt.Sprintf("symbol:%d", p.nextSymbol))
	p.idsBySymbol[symbol] = id
	p.symbolsByID[id] = symbol
	return id
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
	id := typefacts.TypeID(fmt.Sprintf("type:%d", p.nextType))
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
