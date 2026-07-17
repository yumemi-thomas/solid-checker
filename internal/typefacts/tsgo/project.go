// Package tsgo adapts the pinned tsgolint/typescript-go integration to the
// compiler-independent typefacts seam. No shim or compiler types escape it.
package tsgo

import (
	"context"
	"errors"
	"fmt"
	"path/filepath"
	"sort"
	"strings"
	"sync"
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
	mu          sync.Mutex
	configPath  string
	fs          *overlayFS
	versions    map[string]uint64
	program     *compiler.Program
	checker     *checker.Checker
	release     func()
	closed      bool
	generation  uint64
	nextSymbol  uint64
	idsBySymbol map[*ast.Symbol]typefacts.SymbolID
	symbolsByID map[typefacts.SymbolID]*ast.Symbol
	nextType    uint64
	idsByType   map[*checker.Type]typefacts.TypeID
	references  map[*ast.Symbol][]typefacts.Location
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
		generation:  1,
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
	if err := ctx.Err(); err != nil {
		return typefacts.AffectedSet{}, err
	}
	p.mu.Lock()
	defer p.mu.Unlock()
	if p.closed {
		return typefacts.AffectedSet{}, ErrClosed
	}

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

	oldProgram := p.program
	var program *compiler.Program
	var typeChecker *checker.Checker
	var release func()
	var err error
	if incremental && incrementalPath != "" {
		program, typeChecker, release, err = updateProgram(ctx, oldProgram, p.configPath, candidateFS, incrementalPath)
	} else {
		program, typeChecker, release, err = buildProgram(ctx, p.configPath, candidateFS)
	}
	if err != nil {
		return typefacts.AffectedSet{}, err
	}
	if p.release != nil {
		p.release()
	}
	p.program = program
	p.checker = typeChecker
	p.release = release
	p.fs = candidateFS
	p.versions = candidateVersions
	p.generation++
	clear(p.idsBySymbol)
	clear(p.symbolsByID)
	clear(p.idsByType)
	p.references = nil
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

func (p *project) Declarations(ctx context.Context, id typefacts.SymbolID) ([]typefacts.Declaration, error) {
	if err := ctx.Err(); err != nil {
		return nil, err
	}
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

func (p *project) References(ctx context.Context, id typefacts.SymbolID) ([]typefacts.Location, error) {
	if err := ctx.Err(); err != nil {
		return nil, err
	}
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

	if p.references == nil {
		p.references = p.buildReferenceIndex()
	}
	return append([]typefacts.Location(nil), p.references[target]...), nil
}

// buildReferenceIndex resolves every non-declaration identifier once for the
// current program generation. Update invalidates the index when it installs a
// new TypeScript program.
func (p *project) buildReferenceIndex() map[*ast.Symbol][]typefacts.Location {
	references := make(map[*ast.Symbol][]typefacts.Location)
	for _, sourceFile := range p.program.SourceFiles() {
		if sourceFile.IsDeclarationFile {
			continue
		}
		var visit func(*ast.Node) bool
		visit = func(node *ast.Node) bool {
			if ast.IsIdentifier(node) && !ast.IsDeclarationNameOrImportPropertyName(node) {
				symbol := p.checker.GetSymbolAtLocation(node)
				if symbol != nil {
					symbol = p.canonicalSymbol(symbol)
					references[symbol] = append(references[symbol], typefacts.Location{
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
	for symbol, locations := range references {
		sort.Slice(locations, func(i, j int) bool {
			if locations[i].Path != locations[j].Path {
				return locations[i].Path < locations[j].Path
			}
			return locations[i].StartByte < locations[j].StartByte
		})
		references[symbol] = locations
	}
	return references
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

func (p *project) SourceCalls(ctx context.Context, path string) ([]typefacts.SourceCall, error) {
	if err := ctx.Err(); err != nil {
		return nil, err
	}
	p.mu.Lock()
	defer p.mu.Unlock()
	if p.closed {
		return nil, ErrClosed
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
	return functions, nil
}

func (p *project) SourceAsyncFunctions(ctx context.Context, path string) ([]typefacts.AsyncFunctionFact, error) {
	if err := ctx.Err(); err != nil {
		return nil, err
	}
	p.mu.Lock()
	defer p.mu.Unlock()
	if p.closed {
		return nil, ErrClosed
	}
	sourceFile, err := p.sourceFileFor(typefacts.Location{Path: path})
	if err != nil {
		return nil, err
	}
	facts := make([]typefacts.AsyncFunctionFact, 0)
	var visit func(*ast.Node) bool
	visit = func(node *ast.Node) bool {
		if ast.IsArrowFunction(node) || ast.IsFunctionExpression(node) || ast.IsFunctionDeclaration(node) {
			body, symbol := asyncFunctionBodyAndSymbol(p, node)
			if body != nil {
				fact := typefacts.AsyncFunctionFact{
					Expression: typefacts.Location{Path: path, StartByte: scanner.SkipTrivia(sourceFile.Text(), node.Pos()), EndByte: node.End()},
					Symbol:     symbol, CanReturnAsync: ast.HasSyntacticModifier(node, ast.ModifierFlagsAsync),
				}
				if !fact.CanReturnAsync {
					functionType := p.checker.GetTypeAtLocation(node)
					for _, signature := range p.checker.GetSignaturesOfType(functionType, checker.SignatureKindCall) {
						if asyncReturnType(p.checker, p.checker.GetReturnTypeOfSignature(signature)) {
							fact.CanReturnAsync = true
							break
						}
					}
				}
				state := asyncFlowState{reachable: true}
				scanAsyncFlow(p, path, sourceFile, body, body, &state, &fact.CallsAfterAwait)
				facts = append(facts, fact)
			}
		} else if ast.IsVariableDeclaration(node) {
			declaration := node.AsVariableDeclaration()
			if ast.IsIdentifier(declaration.Name()) && declaration.Initializer != nil && ast.IsIdentifier(declaration.Initializer) {
				alias := p.checker.GetSymbolAtLocation(declaration.Name())
				target := p.checker.GetSymbolAtLocation(declaration.Initializer)
				if alias != nil && target != nil {
					facts = append(facts, typefacts.AsyncFunctionFact{
						Expression: typefacts.Location{Path: path, StartByte: scanner.SkipTrivia(sourceFile.Text(), declaration.Initializer.Pos()), EndByte: declaration.Initializer.End()},
						Symbol:     p.idFor(p.canonicalSymbol(alias)),
						Target:     p.idFor(p.canonicalSymbol(target)),
					})
				}
			}
		}
		node.ForEachChild(visit)
		return false
	}
	for _, statement := range sourceFile.Statements.Nodes {
		visit(statement)
	}
	return facts, nil
}

func asyncFunctionBodyAndSymbol(p *project, node *ast.Node) (*ast.Node, typefacts.SymbolID) {
	var body, name *ast.Node
	switch {
	case ast.IsArrowFunction(node):
		body = node.AsArrowFunction().Body
		if parent := node.Parent; parent != nil && ast.IsVariableDeclaration(parent) {
			name = parent.AsVariableDeclaration().Name()
		}
	case ast.IsFunctionExpression(node):
		body = node.AsFunctionExpression().Body
	case ast.IsFunctionDeclaration(node):
		body = node.AsFunctionDeclaration().Body
		name = node.AsFunctionDeclaration().Name()
	}
	if name != nil {
		if symbol := p.checker.GetSymbolAtLocation(name); symbol != nil {
			return body, p.idFor(p.canonicalSymbol(symbol))
		}
	}
	return body, ""
}

func asyncReturnType(typeChecker *checker.Checker, returnType *checker.Type) bool {
	if returnType == nil {
		return false
	}
	if awaited := checker.Checker_getAwaitedType(typeChecker, returnType); awaited != nil && !checker.Checker_isTypeIdenticalTo(typeChecker, returnType, awaited) {
		return true
	}
	if symbol := checker.Type_symbol(returnType); symbol != nil {
		return symbol.Name == "AsyncIterable" || symbol.Name == "AsyncIterator"
	}
	return false
}

type asyncFlowState struct {
	awaited   bool
	reachable bool
}

func scanAsyncFlow(p *project, path string, sourceFile *ast.SourceFile, root, node *ast.Node, state *asyncFlowState, calls *[]typefacts.Location) {
	if node == nil || !state.reachable {
		return
	}
	if node != root && (ast.IsArrowFunction(node) || ast.IsFunctionExpression(node) || ast.IsFunctionDeclaration(node)) {
		return
	}
	if ast.IsBlock(node) {
		for _, statement := range node.AsBlock().Statements.Nodes {
			scanAsyncFlow(p, path, sourceFile, root, statement, state, calls)
			if !state.reachable {
				break
			}
		}
		return
	}
	if ast.IsIfStatement(node) {
		statement := node.AsIfStatement()
		scanAsyncFlow(p, path, sourceFile, root, statement.Expression, state, calls)
		thenState, elseState := *state, *state
		scanAsyncFlow(p, path, sourceFile, root, statement.ThenStatement, &thenState, calls)
		if statement.ElseStatement != nil {
			scanAsyncFlow(p, path, sourceFile, root, statement.ElseStatement, &elseState, calls)
		}
		*state = mergeAsyncBranches(thenState, elseState)
		return
	}
	if ast.IsConditionalExpression(node) {
		expression := node.AsConditionalExpression()
		scanAsyncFlow(p, path, sourceFile, root, expression.Condition, state, calls)
		trueState, falseState := *state, *state
		scanAsyncFlow(p, path, sourceFile, root, expression.WhenTrue, &trueState, calls)
		scanAsyncFlow(p, path, sourceFile, root, expression.WhenFalse, &falseState, calls)
		*state = mergeAsyncBranches(trueState, falseState)
		return
	}
	if ast.IsTryStatement(node) {
		statement := node.AsTryStatement()
		tryState, catchState := *state, *state
		scanAsyncFlow(p, path, sourceFile, root, statement.TryBlock, &tryState, calls)
		if statement.CatchClause != nil {
			scanAsyncFlow(p, path, sourceFile, root, statement.CatchClause.AsCatchClause().Block, &catchState, calls)
		} else {
			catchState = tryState
		}
		*state = mergeAsyncBranches(tryState, catchState)
		if statement.FinallyBlock != nil {
			scanAsyncFlow(p, path, sourceFile, root, statement.FinallyBlock, state, calls)
		}
		return
	}
	if ast.IsIterationStatement(node, true) {
		entry := *state
		loopState := entry
		node.ForEachChild(func(child *ast.Node) bool {
			scanAsyncFlow(p, path, sourceFile, root, child, &loopState, calls)
			return false
		})
		state.awaited, state.reachable = entry.awaited, entry.reachable
		return
	}
	if ast.IsAwaitExpression(node) {
		node.ForEachChild(func(child *ast.Node) bool {
			scanAsyncFlow(p, path, sourceFile, root, child, state, calls)
			return false
		})
		state.awaited = true
		return
	}
	if ast.IsCallExpression(node) && state.awaited {
		if call, ok := p.sourceCallFact(path, sourceFile, node); ok {
			*calls = append(*calls, call.Callee)
		}
	}
	if ast.IsReturnStatement(node) || ast.IsThrowStatement(node) {
		node.ForEachChild(func(child *ast.Node) bool {
			scanAsyncFlow(p, path, sourceFile, root, child, state, calls)
			return false
		})
		state.reachable = false
		return
	}
	node.ForEachChild(func(child *ast.Node) bool {
		scanAsyncFlow(p, path, sourceFile, root, child, state, calls)
		return false
	})
}

func mergeAsyncBranches(left, right asyncFlowState) asyncFlowState {
	if !left.reachable {
		return right
	}
	if !right.reachable {
		return left
	}
	return asyncFlowState{reachable: true, awaited: left.awaited && right.awaited}
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
	return nil
}

func (p *project) idFor(symbol *ast.Symbol) typefacts.SymbolID {
	if id, ok := p.idsBySymbol[symbol]; ok {
		return id
	}
	p.nextSymbol++
	id := typefacts.SymbolID(fmt.Sprintf("symbol:%d:%d", p.generation, p.nextSymbol))
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
