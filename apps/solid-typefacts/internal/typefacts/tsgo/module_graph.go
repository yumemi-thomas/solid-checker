package tsgo

import (
	"context"
	"path/filepath"
	"sort"

	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/compiler"
	"github.com/microsoft/typescript-go/shim/core"
	"github.com/microsoft/typescript-go/shim/scanner"

	"github.com/yumemi-thomas/solid-ts-facts/internal/typefacts"
)

var _ typefacts.ModuleGraphProvider = (*project)(nil)

// ModuleGraph reports the module graph of the accepted program exactly as the
// compiler holds it. Nothing here consults the filesystem or reasons about
// path shape: every field is read off Program, ast.SourceFile, or
// module.ResolvedModule, so an absent answer is the compiler's absence and not
// this adapter's.
func (p *project) ModuleGraph(
	ctx context.Context,
	demand typefacts.ModuleInventoryDemand,
) (typefacts.ModuleInventory, error) {
	if err := ctx.Err(); err != nil {
		return typefacts.ModuleInventory{}, err
	}
	p.mu.Lock()
	defer p.mu.Unlock()
	if p.closed {
		return typefacts.ModuleInventory{}, ErrClosed
	}
	program := p.program
	sourceFiles := program.SourceFiles()

	inventory := typefacts.ModuleInventory{
		Modules: make([]typefacts.ModuleFact, 0, len(sourceFiles)),
	}
	for _, sourceFile := range sourceFiles {
		if err := ctx.Err(); err != nil {
			return typefacts.ModuleInventory{}, err
		}
		inventory.Modules = append(inventory.Modules, moduleFact(program, sourceFile))
	}
	sort.Slice(inventory.Modules, func(i, j int) bool {
		return inventory.Modules[i].Path < inventory.Modules[j].Path
	})

	if !demand.Imports {
		return inventory, nil
	}

	answered, unknown := importScope(sourceFiles, demand.ImportPaths)
	inventory.UnknownImportPaths = unknown
	var packages *packageScopeCache
	if demand.Packages {
		packages = newPackageScopeCache(program)
	}
	for _, sourceFile := range answered {
		if err := ctx.Err(); err != nil {
			return typefacts.ModuleInventory{}, err
		}
		inventory.Imports = appendImportFacts(inventory.Imports, program, sourceFile, packages)
	}
	sort.Slice(inventory.Imports, func(i, j int) bool {
		left, right := inventory.Imports[i].Specifier, inventory.Imports[j].Specifier
		if left.Path != right.Path {
			return left.Path < right.Path
		}
		return left.StartByte < right.StartByte
	})
	return inventory, nil
}

// importScope selects the files whose imports are answered. An explicitly
// requested path the program does not hold is reported, never silently
// dropped: a caller cannot otherwise distinguish a file with no imports from a
// file that was never in the program.
func importScope(sourceFiles []*ast.SourceFile, requested []string) ([]*ast.SourceFile, []string) {
	if len(requested) == 0 {
		return sourceFiles, nil
	}
	byPath := make(map[string]*ast.SourceFile, len(sourceFiles))
	for _, sourceFile := range sourceFiles {
		byPath[filepath.Clean(sourceFile.FileName())] = sourceFile
	}
	answered := make([]*ast.SourceFile, 0, len(requested))
	var unknown []string
	seen := make(map[string]struct{}, len(requested))
	for _, path := range requested {
		path = filepath.Clean(path)
		if _, repeated := seen[path]; repeated {
			continue
		}
		seen[path] = struct{}{}
		if sourceFile, ok := byPath[path]; ok {
			answered = append(answered, sourceFile)
			continue
		}
		unknown = append(unknown, path)
	}
	sort.Strings(unknown)
	return answered, unknown
}

func moduleFact(program *compiler.Program, sourceFile *ast.SourceFile) typefacts.ModuleFact {
	fact := typefacts.ModuleFact{
		Path:            filepath.Clean(sourceFile.FileName()),
		DeclarationFile: sourceFile.IsDeclarationFile,
		Format:          moduleFormat(program.GetEmitModuleFormatOfFile(sourceFile)),
	}
	// The only declaration-to-implementation pairing TypeScript records is a
	// configured project reference's input and its declaration output. The
	// program holds one of the two, so both mapper directions are asked. A
	// hand-written or published `x.d.ts` beside an `x.js` answers neither,
	// because resolution selected the declaration file and never opened the
	// implementation. Reconstructing that pairing from the two file names is
	// exactly the substitution this fact exists to avoid.
	reference := program.GetProjectReferenceFromSource(sourceFile.Path())
	if reference == nil {
		reference = program.GetProjectReferenceFromOutputDts(sourceFile.Path())
	}
	if reference != nil && reference.Source != "" && reference.OutputDts != "" {
		fact.ProjectReference = &typefacts.ProjectReferenceMapping{
			Source:    filepath.Clean(reference.Source),
			OutputDts: filepath.Clean(reference.OutputDts),
		}
	}
	if targets := program.GetRedirectTargets(sourceFile.Path()); len(targets) != 0 {
		cleaned := make([]string, 0, len(targets))
		for _, target := range targets {
			cleaned = append(cleaned, filepath.Clean(target))
		}
		sort.Strings(cleaned)
		fact.RedirectTargets = cleaned
	}
	return fact
}

// moduleFormat reports only the emit formats that describe a real runtime
// shape. GetEmitModuleFormatOfFile resolves the node16/nodenext family to
// CommonJS or an ES kind through the file's implied node format, so the legacy
// kinds are the only ones that reach the default arm, and they are answered as
// unknown rather than characterized.
func moduleFormat(kind core.ModuleKind) typefacts.ModuleFormat {
	switch {
	case kind == core.ModuleKindCommonJS:
		return typefacts.ModuleFormatCommonJS
	case kind.IsNonNodeESM():
		return typefacts.ModuleFormatESM
	case kind == core.ModuleKindPreserve:
		return typefacts.ModuleFormatPreserve
	default:
		return typefacts.ModuleFormatUnknown
	}
}

func appendImportFacts(
	facts []typefacts.ModuleImportFact,
	program *compiler.Program,
	sourceFile *ast.SourceFile,
	packages *packageScopeCache,
) []typefacts.ModuleImportFact {
	specifiers := sourceFile.Imports()
	if len(specifiers) == 0 {
		return facts
	}
	path := filepath.Clean(sourceFile.FileName())
	text := sourceFile.Text()
	patterns := pathsPatternsOf(program)
	for _, specifier := range specifiers {
		fact := typefacts.ModuleImportFact{
			Specifier: typefacts.Location{
				Path:      path,
				StartByte: scanner.SkipTrivia(text, specifier.Pos()),
				EndByte:   specifier.End(),
			},
			Text:         specifier.Text(),
			Resolution:   typefacts.ModuleResolutionUnresolved,
			PathsPattern: patterns.match(specifier.Text()),
		}
		resolved := program.GetResolvedModuleFromModuleSpecifier(sourceFile, specifier)
		if resolved != nil && resolved.ResolvedFileName != "" {
			fact.ResolvedPath = filepath.Clean(resolved.ResolvedFileName)
			fact.Extension = resolved.Extension
			fact.TSExtension = resolved.ResolvedUsingTsExtension
			if resolved.OriginalPath != "" {
				fact.SymlinkPath = filepath.Clean(resolved.OriginalPath)
			}
			// The program's own redirect for the resolved name. This is the
			// one mechanism that joins a specifier resolved to a declaration
			// file back to an implementation, and it fires only for a
			// configured project reference's output.
			if redirect := program.GetParseFileRedirect(resolved.ResolvedFileName); redirect != "" {
				if cleaned := filepath.Clean(redirect); cleaned != fact.ResolvedPath {
					fact.IncludedPath = cleaned
				}
			}
			switch {
			case resolved.IsExternalLibraryImport:
				fact.Resolution = typefacts.ModuleResolutionNodeModules
			case isRelativeSpecifier(fact.Text):
				fact.Resolution = typefacts.ModuleResolutionRelative
			default:
				fact.Resolution = typefacts.ModuleResolutionNonRelative
			}
			if packages != nil {
				fact.Package = packages.identityFor(fact.ResolvedPath)
				if resolved.PackageId.Name != "" || resolved.PackageId.Version != "" {
					fact.ResolverPackage = &typefacts.ResolverPackageID{
						Name:             resolved.PackageId.Name,
						Subpath:          resolved.PackageId.SubModuleName,
						Version:          resolved.PackageId.Version,
						PeerDependencies: resolved.PackageId.PeerDependencies,
					}
				}
			}
		}
		facts = append(facts, fact)
	}
	return facts
}

// isRelativeSpecifier mirrors tspath.IsExternalModuleNameRelative: a specifier
// whose first term is "." or "..", or a rooted disk path, is never searched for
// in node_modules and never matched against `paths`. Only the two-character
// prefixes and the bare "."/".." forms are relative; "./" and ".\" both count,
// because TypeScript accepts either separator in a specifier.
func isRelativeSpecifier(text string) bool {
	switch {
	case text == "." || text == "..":
		return true
	case len(text) >= 2 && text[0] == '.' && (text[1] == '/' || text[1] == '\\'):
		return true
	case len(text) >= 3 && text[0] == '.' && text[1] == '.' && (text[2] == '/' || text[2] == '\\'):
		return true
	case len(text) >= 1 && (text[0] == '/' || text[0] == '\\'):
		return true
	case len(text) >= 3 && text[1] == ':' && (text[2] == '/' || text[2] == '\\'):
		return true
	default:
		return false
	}
}

// pathsPatterns holds the compiler's parsed `paths` keys for one program.
type pathsPatterns struct {
	eligible bool
	patterns []core.Pattern
}

func pathsPatternsOf(program *compiler.Program) pathsPatterns {
	options := program.Options()
	if options == nil || options.Paths.Size() == 0 {
		return pathsPatterns{}
	}
	parsed := pathsPatterns{eligible: true}
	for key := range options.Paths.Keys() {
		if pattern := core.TryParsePattern(key); pattern.IsValid() {
			parsed.patterns = append(parsed.patterns, pattern)
		}
	}
	return parsed
}

// match selects the configured `paths` key for a specifier under TypeScript's
// own rules: `paths` is consulted only for a non-relative specifier, an exact
// key wins outright, and among star patterns the longest literal prefix wins.
// The predicate and the star classification are the compiler's own
// core.Pattern; only the selection loop, which mirrors core.FindBestPatternMatch
// over an un-shimmable generic, is written out here.
func (p pathsPatterns) match(text string) string {
	if !p.eligible || isRelativeSpecifier(text) {
		return ""
	}
	best := ""
	longestPrefix := -1
	for index := range p.patterns {
		pattern := p.patterns[index]
		if pattern.StarIndex != -1 && pattern.StarIndex <= longestPrefix {
			continue
		}
		if !pattern.Matches(text) {
			continue
		}
		if pattern.StarIndex == -1 {
			return pattern.Text
		}
		best = pattern.Text
		longestPrefix = pattern.StarIndex
	}
	return best
}

// packageScopeCache answers "which package.json owns this file" through the
// compiler's own package-scope lookup, memoized per directory for one graph
// answer. The compiler caches manifest reads itself; this only avoids repeating
// the ancestor walk once per import row.
type packageScopeCache struct {
	program      *compiler.Program
	byDirectory  map[string]*typefacts.PackageIdentity
	byPackageDir map[string]*typefacts.PackageIdentity
}

func newPackageScopeCache(program *compiler.Program) *packageScopeCache {
	return &packageScopeCache{
		program:      program,
		byDirectory:  make(map[string]*typefacts.PackageIdentity),
		byPackageDir: make(map[string]*typefacts.PackageIdentity),
	}
}

func (c *packageScopeCache) identityFor(resolvedPath string) *typefacts.PackageIdentity {
	directory := typeScriptPathDir(resolvedPath)
	if cached, ok := c.byDirectory[directory]; ok {
		return cached
	}
	identity := c.lookup(directory)
	c.byDirectory[directory] = identity
	return identity
}

func (c *packageScopeCache) lookup(directory string) *typefacts.PackageIdentity {
	packageDirectory := c.program.GetNearestAncestorDirectoryWithPackageJson(directory)
	if packageDirectory == "" {
		return nil
	}
	packageDirectory = normalizeTypeScriptPath(packageDirectory)
	if cached, ok := c.byPackageDir[packageDirectory]; ok {
		return cached
	}
	manifestPath := packageDirectory + "/package.json"
	identity := &typefacts.PackageIdentity{ManifestPath: filepath.Clean(manifestPath)}
	if contents := c.program.GetPackageJsonInfo(manifestPath).GetContents(); contents != nil {
		if contents.Name.Valid {
			identity.Name = contents.Name.Value
		}
		if contents.Version.Valid {
			identity.Version = contents.Version.Value
		}
	}
	c.byPackageDir[packageDirectory] = identity
	return identity
}
