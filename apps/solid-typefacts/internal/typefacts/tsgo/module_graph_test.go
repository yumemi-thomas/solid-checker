package tsgo

import (
	"context"
	"os"
	"path/filepath"
	"runtime"
	"sort"
	"strings"
	"testing"

	"github.com/yumemi-thomas/solid-checker/apps/solid-typefacts/internal/typefacts"
)

// moduleGraphProject writes files into a fresh directory and opens it. The
// directory is realpath-resolved first: macOS hands out a symlinked temporary
// root, and the resolver takes a realpath of its own for node_modules
// resolutions, so an unresolved root would make every external import look
// symlinked and would hide the one case that genuinely is.
func moduleGraphProject(t *testing.T, files map[string]string) (string, typefacts.ModuleGraphProvider) {
	t.Helper()
	dir, err := filepath.EvalSymlinks(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	writeProjectFiles(t, dir, files)
	opened, err := OpenProject(context.Background(), filepath.Join(dir, "tsconfig.json"), nil)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = opened.Close() })
	provider, ok := opened.(typefacts.ModuleGraphProvider)
	if !ok {
		t.Fatal("tsgo project does not provide a module graph")
	}
	return dir, provider
}

func writeProjectFiles(t *testing.T, dir string, files map[string]string) {
	t.Helper()
	for name, contents := range files {
		path := filepath.Join(dir, filepath.FromSlash(name))
		if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
			t.Fatal(err)
		}
		if err := os.WriteFile(path, []byte(contents), 0o644); err != nil {
			t.Fatal(err)
		}
	}
}

func modulePaths(t *testing.T, dir string, inventory typefacts.ModuleInventory) []string {
	t.Helper()
	paths := make([]string, 0, len(inventory.Modules))
	for _, module := range inventory.Modules {
		relative, err := filepath.Rel(dir, module.Path)
		if err != nil || strings.HasPrefix(relative, "..") {
			// Default library files live outside the project and are not part
			// of what these tests assert.
			continue
		}
		paths = append(paths, filepath.ToSlash(relative))
	}
	sort.Strings(paths)
	return paths
}

func importByText(t *testing.T, inventory typefacts.ModuleInventory, text string) typefacts.ModuleImportFact {
	t.Helper()
	for _, fact := range inventory.Imports {
		if fact.Text == text {
			return fact
		}
	}
	t.Fatalf("no import fact for %q; have %+v", text, inventory.Imports)
	return typefacts.ModuleImportFact{}
}

func moduleByPath(t *testing.T, inventory typefacts.ModuleInventory, path string) typefacts.ModuleFact {
	t.Helper()
	for _, module := range inventory.Modules {
		if module.Path == filepath.Clean(path) {
			return module
		}
	}
	t.Fatalf("no module fact for %q", path)
	return typefacts.ModuleFact{}
}

const bundlerConfig = `{
  "compilerOptions": {
    "strict": true,
    "module": "esnext",
    "target": "esnext",
    "moduleResolution": "bundler",
    "allowJs": true
  },
  "include": ["**/*.ts", "**/*.js", "**/*.d.ts"]
}`

func TestModuleInventoryReportsEveryIncludedFile(t *testing.T) {
	dir, provider := moduleGraphProject(t, map[string]string{
		"tsconfig.json": bundlerConfig,
		"index.ts":      "import { helper } from \"./helper\";\nimport type { Shape } from \"./shape\";\nexport const value: Shape = helper();\n",
		"helper.ts":     "import type { Shape } from \"./shape\";\nexport function helper(): Shape { return { tag: \"shape\" }; }\n",
		"shape.d.ts":    "export interface Shape { tag: string }\n",
		"unreached.ts":  "export const unreached = 1;\n",
	})
	inventory, err := provider.ModuleGraph(context.Background(), typefacts.ModuleInventoryDemand{})
	if err != nil {
		t.Fatal(err)
	}
	got := modulePaths(t, dir, inventory)
	want := []string{"helper.ts", "index.ts", "shape.d.ts", "unreached.ts"}
	if strings.Join(got, ",") != strings.Join(want, ",") {
		t.Fatalf("project modules = %v, want %v", got, want)
	}
	if declaration := moduleByPath(t, inventory, filepath.Join(dir, "shape.d.ts")); !declaration.DeclarationFile {
		t.Errorf("shape.d.ts declarationFile = false, want true")
	}
	if implementation := moduleByPath(t, inventory, filepath.Join(dir, "index.ts")); implementation.DeclarationFile {
		t.Errorf("index.ts declarationFile = true, want false")
	}
	if format := moduleByPath(t, inventory, filepath.Join(dir, "index.ts")).Format; format != typefacts.ModuleFormatESM {
		t.Errorf("index.ts format = %q, want %q", format, typefacts.ModuleFormatESM)
	}
	// The inventory is the program's own file list, so the default library
	// files it opened are in it too. That is the point: a closure record built
	// from it names what the analysis read rather than what a scanner guessed.
	if len(inventory.Modules) <= len(want) {
		t.Errorf("inventory holds %d modules, want more than the %d project files", len(inventory.Modules), len(want))
	}
	if len(inventory.Imports) != 0 {
		t.Errorf("imports = %d without an import demand, want 0", len(inventory.Imports))
	}
}

func TestModuleGraphReportsRelativeImportResolution(t *testing.T) {
	dir, provider := moduleGraphProject(t, map[string]string{
		"tsconfig.json":    bundlerConfig,
		"index.ts":         "import { helper } from \"./nested/helper\";\nexport const value = helper();\n",
		"nested/helper.ts": "export function helper() { return 1; }\n",
	})
	inventory, err := provider.ModuleGraph(context.Background(), typefacts.ModuleInventoryDemand{
		Imports:     true,
		ImportPaths: []string{filepath.Join(dir, "index.ts")},
		Packages:    true,
	})
	if err != nil {
		t.Fatal(err)
	}
	if len(inventory.Imports) != 1 {
		t.Fatalf("imports = %d, want 1", len(inventory.Imports))
	}
	fact := inventory.Imports[0]
	if fact.Resolution != typefacts.ModuleResolutionRelative {
		t.Errorf("resolution = %q, want %q", fact.Resolution, typefacts.ModuleResolutionRelative)
	}
	if want := filepath.Join(dir, "nested", "helper.ts"); fact.ResolvedPath != want {
		t.Errorf("resolvedPath = %q, want %q", fact.ResolvedPath, want)
	}
	if fact.Extension != ".ts" {
		t.Errorf("extension = %q, want %q", fact.Extension, ".ts")
	}
	if fact.SymlinkPath != "" {
		t.Errorf("symlinkPath = %q, want empty", fact.SymlinkPath)
	}
	if fact.PathsPattern != "" {
		t.Errorf("pathsPattern = %q, want empty", fact.PathsPattern)
	}
	if fact.ResolverPackage != nil {
		t.Errorf("resolverPackage = %+v, want nil for a relative import", fact.ResolverPackage)
	}
	if fact.Package == nil || fact.Package.ManifestPath == "" {
		// A project with no manifest of its own answers no package identity;
		// this one has none, so absence is the correct answer.
		if fact.Package != nil {
			t.Errorf("package = %+v, want nil", fact.Package)
		}
	}
	source, err := os.ReadFile(filepath.Join(dir, "index.ts"))
	if err != nil {
		t.Fatal(err)
	}
	span := string(source[fact.Specifier.StartByte:fact.Specifier.EndByte])
	if span != `"./nested/helper"` {
		t.Errorf("specifier span = %s, want the quoted literal", span)
	}
}

// A declaration file beside a runtime file is the shape almost every published
// package has, and it is the case the protocol cannot close. The compiler
// resolves the specifier to the declaration file, includes the runtime file as
// an unrelated root, and records nothing joining the two. This test pins that
// the fact reports the split honestly rather than papering over it.
func TestDeclarationSiblingHasNoCompilerRecordedRuntimePairing(t *testing.T) {
	dir, provider := moduleGraphProject(t, map[string]string{
		"tsconfig.json": bundlerConfig,
		"index.ts":      "import { channelFor } from \"./channel.js\";\nexport const channel = channelFor();\n",
		"channel.js":    "export function channelFor() { return { live: true }; }\n",
		"channel.d.ts":  "export declare function channelFor(): { live: boolean };\n",
	})
	inventory, err := provider.ModuleGraph(context.Background(), typefacts.ModuleInventoryDemand{Imports: true})
	if err != nil {
		t.Fatal(err)
	}
	fact := importByText(t, inventory, "./channel.js")
	declaration := filepath.Join(dir, "channel.d.ts")
	if fact.ResolvedPath != declaration {
		t.Fatalf("resolvedPath = %q, want the declaration file %q", fact.ResolvedPath, declaration)
	}
	if fact.Extension != ".d.ts" {
		t.Errorf("extension = %q, want %q", fact.Extension, ".d.ts")
	}
	// Both files are in the program, as unrelated roots.
	runtimeModule := moduleByPath(t, inventory, filepath.Join(dir, "channel.js"))
	declarationModule := moduleByPath(t, inventory, declaration)
	if !declarationModule.DeclarationFile || runtimeModule.DeclarationFile {
		t.Fatalf("declaration bits wrong: %+v %+v", declarationModule, runtimeModule)
	}
	// And nothing pairs them. These are the compiler's own records; all three
	// are empty because no configured project reference produced this .d.ts,
	// and TypeScript has no other pairing mechanism to consult.
	if declarationModule.ProjectReference != nil {
		t.Errorf(
			"declaration projectReference = %+v, want nil: the compiler records no input for a hand-written .d.ts",
			declarationModule.ProjectReference,
		)
	}
	if len(declarationModule.RedirectTargets) != 0 {
		t.Errorf("redirectTargets = %v, want none", declarationModule.RedirectTargets)
	}
	if fact.IncludedPath != "" {
		t.Errorf("includedPath = %q, want empty: nothing redirects a shipped .d.ts to the .js beside it", fact.IncludedPath)
	}
}

// The one declaration-to-input pairing TypeScript does record is a configured
// project reference's declaration output.
func TestProjectReferenceDeclarationOutputCarriesItsInput(t *testing.T) {
	dir, provider := moduleGraphProject(t, map[string]string{
		"tsconfig.json": `{
  "compilerOptions": { "strict": true, "module": "esnext", "target": "esnext", "moduleResolution": "bundler" },
  "include": ["app/**/*.ts"],
  "references": [{ "path": "./lib" }]
}`,
		"lib/tsconfig.json": `{
  "compilerOptions": {
    "strict": true, "module": "esnext", "target": "esnext", "moduleResolution": "bundler",
    "composite": true, "declaration": true, "outDir": "./dist", "rootDir": "./src"
  },
  "include": ["src/**/*.ts"]
}`,
		"lib/src/channel.ts":    "export function channelFor() { return { live: true }; }\n",
		"lib/dist/channel.d.ts": "export declare function channelFor(): { live: boolean };\n",
		"lib/dist/channel.js":   "export function channelFor() { return { live: true }; }\n",
		"app/index.ts":          "import { channelFor } from \"../lib/dist/channel.js\";\nexport const channel = channelFor();\n",
	})
	inventory, err := provider.ModuleGraph(context.Background(), typefacts.ModuleInventoryDemand{Imports: true})
	if err != nil {
		t.Fatal(err)
	}
	fact := importByText(t, inventory, "../lib/dist/channel.js")
	if fact.Resolution != typefacts.ModuleResolutionRelative {
		t.Fatalf("resolution = %q, want relative", fact.Resolution)
	}
	source := filepath.Join(dir, "lib", "src", "channel.ts")
	declaration := filepath.Join(dir, "lib", "dist", "channel.d.ts")
	// The resolver still selects the declaration output; the program redirects
	// it to the input it was emitted from. That redirect is the whole of what
	// the compiler knows about declaration-to-implementation pairing.
	if fact.ResolvedPath != declaration {
		t.Fatalf("resolvedPath = %q, want the declaration output %q", fact.ResolvedPath, declaration)
	}
	if fact.IncludedPath != source {
		t.Fatalf("includedPath = %q, want the project-reference input %q", fact.IncludedPath, source)
	}
	// The inventory holds the input, and names the output it corresponds to.
	module := moduleByPath(t, inventory, source)
	if module.ProjectReference == nil {
		t.Fatalf("%q carries no project-reference mapping", source)
	}
	if module.ProjectReference.Source != source || module.ProjectReference.OutputDts != declaration {
		t.Errorf("projectReference = %+v, want %s <-> %s", module.ProjectReference, source, declaration)
	}
	for _, other := range inventory.Modules {
		if other.Path == declaration {
			t.Errorf("the redirected declaration output %q is in the inventory as its own module", declaration)
		}
	}
}

func TestPathsAliasedImportIsDistinguishableFromTheInstalledPackage(t *testing.T) {
	dir, provider := moduleGraphProject(t, map[string]string{
		"tsconfig.json": `{
  "compilerOptions": {
    "strict": true, "module": "esnext", "target": "esnext", "moduleResolution": "bundler",
    "paths": { "reactive-package": ["./src/local-impl.ts"], "shared/*": ["./src/shared/*"] }
  },
  "include": ["src/**/*.ts"]
}`,
		"src/local-impl.ts":  "export function createReactive() { return 1; }\n",
		"src/shared/util.ts": "export const util = 1;\n",
		"src/index.ts": "import { createReactive } from \"reactive-package\";\n" +
			"import { util } from \"shared/util\";\n" +
			"export const value = createReactive() + util;\n",
		"node_modules/reactive-package/package.json": `{"name":"reactive-package","version":"4.2.0","main":"index.js","types":"index.d.ts"}`,
		"node_modules/reactive-package/index.d.ts":   "export declare function createReactive(): number;\n",
		"node_modules/reactive-package/index.js":     "export function createReactive() { return 2; }\n",
	})
	inventory, err := provider.ModuleGraph(context.Background(), typefacts.ModuleInventoryDemand{
		Imports:  true,
		Packages: true,
	})
	if err != nil {
		t.Fatal(err)
	}
	aliased := importByText(t, inventory, "reactive-package")
	if want := filepath.Join(dir, "src", "local-impl.ts"); aliased.ResolvedPath != want {
		t.Fatalf("resolvedPath = %q, want %q", aliased.ResolvedPath, want)
	}
	// The decisive pair: a bare specifier that a `paths` key matched and that
	// did not land in node_modules is not the installed package of that name,
	// however identical the specifier text is.
	if aliased.Resolution != typefacts.ModuleResolutionNonRelative {
		t.Errorf("resolution = %q, want %q", aliased.Resolution, typefacts.ModuleResolutionNonRelative)
	}
	if aliased.PathsPattern != "reactive-package" {
		t.Errorf("pathsPattern = %q, want %q", aliased.PathsPattern, "reactive-package")
	}
	if aliased.ResolverPackage != nil {
		t.Errorf("resolverPackage = %+v, want nil: no manifest was consulted", aliased.ResolverPackage)
	}
	if aliased.Package != nil && aliased.Package.Name == "reactive-package" {
		t.Errorf("package = %+v, want anything but the installed package it shadows", aliased.Package)
	}
	star := importByText(t, inventory, "shared/util")
	if star.PathsPattern != "shared/*" {
		t.Errorf("star pathsPattern = %q, want %q", star.PathsPattern, "shared/*")
	}
}

func TestNodeModulesImportCarriesOwningPackageIdentity(t *testing.T) {
	dir, provider := moduleGraphProject(t, map[string]string{
		"tsconfig.json": `{
  "compilerOptions": { "strict": true, "module": "esnext", "target": "esnext", "moduleResolution": "bundler" },
  "include": ["src/**/*.ts"]
}`,
		"package.json": `{"name":"host-project","version":"0.1.0"}`,
		"src/index.ts": "import { createReactive } from \"reactive-package\";\n" +
			"import { deep } from \"reactive-package/deep\";\n" +
			"export const value = createReactive() + deep;\n",
		"node_modules/reactive-package/package.json":     `{"name":"reactive-package","version":"4.2.0","exports":{".":"./index.js","./deep":"./nested/deep.js"}}`,
		"node_modules/reactive-package/index.d.ts":       "export declare function createReactive(): number;\n",
		"node_modules/reactive-package/index.js":         "export function createReactive() { return 2; }\n",
		"node_modules/reactive-package/nested/deep.d.ts": "export declare const deep: number;\n",
		"node_modules/reactive-package/nested/deep.js":   "export const deep = 3;\n",
	})
	inventory, err := provider.ModuleGraph(context.Background(), typefacts.ModuleInventoryDemand{
		Imports:  true,
		Packages: true,
	})
	if err != nil {
		t.Fatal(err)
	}
	root := importByText(t, inventory, "reactive-package")
	if root.Resolution != typefacts.ModuleResolutionNodeModules {
		t.Fatalf("resolution = %q, want %q", root.Resolution, typefacts.ModuleResolutionNodeModules)
	}
	if root.Package == nil {
		t.Fatal("package = nil, want the owning manifest")
	}
	wantManifest := filepath.Join(dir, "node_modules", "reactive-package", "package.json")
	if root.Package.ManifestPath != wantManifest {
		t.Errorf("manifestPath = %q, want %q", root.Package.ManifestPath, wantManifest)
	}
	if root.Package.Name != "reactive-package" || root.Package.Version != "4.2.0" {
		t.Errorf("package identity = %+v, want reactive-package@4.2.0", root.Package)
	}
	if root.ResolverPackage == nil || root.ResolverPackage.Name != "reactive-package" || root.ResolverPackage.Version != "4.2.0" {
		t.Errorf("resolverPackage = %+v, want reactive-package@4.2.0", root.ResolverPackage)
	}
	deep := importByText(t, inventory, "reactive-package/deep")
	if deep.Package == nil || deep.Package.ManifestPath != wantManifest {
		t.Errorf("deep subpath package = %+v, want the same owning manifest %q", deep.Package, wantManifest)
	}
	// SubModuleName is the path of the file the resolver landed on within the
	// package, not the `exports` key that selected it.
	if deep.ResolverPackage == nil || deep.ResolverPackage.Subpath != "nested/deep.d.ts" {
		t.Errorf("deep resolverPackage = %+v, want subpath %q", deep.ResolverPackage, "nested/deep.d.ts")
	}
}

// pnpm installs a package once in a content-addressed store and links it into
// each dependent's node_modules. TypeScript resolves through the link and then
// takes the realpath, so the two paths differ and both are reported.
func TestSymlinkedPackageReportsBothPaths(t *testing.T) {
	if runtime.GOOS == "windows" {
		t.Skip("symlink creation needs elevation on Windows")
	}
	dir, err := filepath.EvalSymlinks(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	writeProjectFiles(t, dir, map[string]string{
		"tsconfig.json": `{
  "compilerOptions": { "strict": true, "module": "esnext", "target": "esnext", "moduleResolution": "bundler" },
  "include": ["src/**/*.ts"]
}`,
		"src/index.ts": "import { createReactive } from \"reactive-package\";\nexport const value = createReactive();\n",
		"node_modules/.store/reactive-package@4.2.0/node_modules/reactive-package/package.json": `{"name":"reactive-package","version":"4.2.0","main":"index.js","types":"index.d.ts"}`,
		"node_modules/.store/reactive-package@4.2.0/node_modules/reactive-package/index.d.ts":   "export declare function createReactive(): number;\n",
		"node_modules/.store/reactive-package@4.2.0/node_modules/reactive-package/index.js":     "export function createReactive() { return 2; }\n",
	})
	link := filepath.Join(dir, "node_modules", "reactive-package")
	target := filepath.Join(dir, "node_modules", ".store", "reactive-package@4.2.0", "node_modules", "reactive-package")
	if err := os.Symlink(target, link); err != nil {
		t.Fatal(err)
	}
	opened, err := OpenProject(context.Background(), filepath.Join(dir, "tsconfig.json"), nil)
	if err != nil {
		t.Fatal(err)
	}
	defer opened.Close()
	provider, ok := opened.(typefacts.ModuleGraphProvider)
	if !ok {
		t.Fatal("tsgo project does not provide a module graph")
	}
	inventory, err := provider.ModuleGraph(context.Background(), typefacts.ModuleInventoryDemand{
		Imports:  true,
		Packages: true,
	})
	if err != nil {
		t.Fatal(err)
	}
	fact := importByText(t, inventory, "reactive-package")
	if fact.Resolution != typefacts.ModuleResolutionNodeModules {
		t.Fatalf("resolution = %q, want %q", fact.Resolution, typefacts.ModuleResolutionNodeModules)
	}
	if want := filepath.Join(target, "index.d.ts"); fact.ResolvedPath != want {
		t.Errorf("resolvedPath = %q, want the realpath %q", fact.ResolvedPath, want)
	}
	if want := filepath.Join(link, "index.d.ts"); fact.SymlinkPath != want {
		t.Errorf("symlinkPath = %q, want the as-resolved link path %q", fact.SymlinkPath, want)
	}
	// The owning manifest is looked up from the realpath, so it is the store
	// copy's manifest rather than the link's.
	if fact.Package == nil || fact.Package.ManifestPath != filepath.Join(target, "package.json") {
		t.Errorf("package = %+v, want the store manifest", fact.Package)
	}
	// The inventory names the realpath too, so a closure record built from it
	// identifies one copy of the package rather than one link into it.
	moduleByPath(t, inventory, filepath.Join(target, "index.d.ts"))
}

func TestUnknownImportPathsAreReportedRatherThanDropped(t *testing.T) {
	dir, provider := moduleGraphProject(t, map[string]string{
		"tsconfig.json": bundlerConfig,
		"index.ts":      "export const value = 1;\n",
	})
	missing := filepath.Join(dir, "absent.ts")
	inventory, err := provider.ModuleGraph(context.Background(), typefacts.ModuleInventoryDemand{
		Imports:     true,
		ImportPaths: []string{filepath.Join(dir, "index.ts"), missing, missing},
	})
	if err != nil {
		t.Fatal(err)
	}
	if len(inventory.Imports) != 0 {
		t.Errorf("imports = %+v, want none", inventory.Imports)
	}
	if len(inventory.UnknownImportPaths) != 1 || inventory.UnknownImportPaths[0] != missing {
		t.Errorf("unknownImportPaths = %v, want [%s]", inventory.UnknownImportPaths, missing)
	}
}

func TestUnresolvedSpecifierIsReportedAsUnresolved(t *testing.T) {
	_, provider := moduleGraphProject(t, map[string]string{
		"tsconfig.json": bundlerConfig,
		"index.ts":      "// @ts-expect-error missing package\nimport { nothing } from \"never-installed\";\nexport const value = nothing;\n",
	})
	inventory, err := provider.ModuleGraph(context.Background(), typefacts.ModuleInventoryDemand{Imports: true})
	if err != nil {
		t.Fatal(err)
	}
	fact := importByText(t, inventory, "never-installed")
	if fact.Resolution != typefacts.ModuleResolutionUnresolved {
		t.Errorf("resolution = %q, want %q", fact.Resolution, typefacts.ModuleResolutionUnresolved)
	}
	if fact.ResolvedPath != "" {
		t.Errorf("resolvedPath = %q, want empty", fact.ResolvedPath)
	}
}
