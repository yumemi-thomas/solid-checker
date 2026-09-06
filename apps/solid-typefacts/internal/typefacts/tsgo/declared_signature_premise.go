package tsgo

import (
	"context"
	"fmt"
	"path/filepath"
	"strconv"
	"strings"

	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/bundled"
	"github.com/microsoft/typescript-go/shim/checker"
	"github.com/microsoft/typescript-go/shim/compiler"
	"github.com/yumemi-thomas/solid-checker/apps/solid-typefacts/internal/typefacts"
)

// The declared-signature premise for the uncensused-form census (ADR 0038).
//
// An unannotated JavaScript parameter is `any` to the checker, so every binary
// operator over it may reach a `valueOf` and every `for…of` over it may reach a
// user iterator, and the form census records the coercion or the iteration
// protocol. The export a consumer calls, though, is declared in a `.d.ts`, and
// that declaration is what the consumer's own compiler holds its arguments to
// — and what the synthesized veto samples from. Classifying the body under
// *that* signature asks the same question the census already asks, under the
// premise every other consumer of the transcript already accepts.
//
// The producer does not re-type the body itself. It builds a **twin** of the
// implementation's file: the same text with one JSDoc comment inserted before
// the declaration — `/** @type {typeof import("<declaration module>").<name>}
// */` — which is exactly how a JavaScript author would state the same fact,
// re-parses that one file into a program that shares every other source file
// with the accepted program, and lets the compiler's own contextual typing
// carry the declared parameter types into the body: through the parameters,
// through the return type into a returned arrow's parameters, through the
// element type of a declared array into a `map` callback. The form census then
// runs over the twin's implementation node with the twin's checker, and every
// location it reports is mapped back to the original bytes by the one
// insertion's length.
//
// What binds the twin to the declaration rather than to the producer's
// spelling of it: after the twin is checked, every parameter's type on the
// twin must print byte-identically to the declared signature's type at that
// position, taken from the accepted program's checker, and a type with a
// named declaration must resolve to the same declaration node. A twin that
// fails either comparison is discarded and the census keeps the strictly more
// refusing reading over the parameters' own types.
//
// What is deliberately outside this premise: a local helper's parameters (a
// call-site argument type is a fact about the caller's twin, not a
// declaration, and stating it is ADR 0038's named follow-up), an
// implementation whose declaration is not in a declaration file (its parameter
// types are already the checker's), a declaration with a rest parameter or a
// different arity from its implementation, and a declaration whose anchor
// already carries a JSDoc tag the compiler would read as a type.

// declaredSignaturePremise is the export's declared call signature as the
// export-value transcript resolved it, handed to the implementation transcript
// so the form census can bind it to the implementation's parameters.
type declaredSignaturePremise struct {
	signature   *checker.Signature
	declaration *ast.Node
	target      *ast.Symbol
}

// premiseTwin is one checked twin: the program, the checker lease, the twin's
// own source-file object, the implementation node inside it, the offset and
// length of the single insertion, and the premises it established.
type premiseTwin struct {
	program        *compiler.Program
	checker        *checker.Checker
	release        func()
	file           *ast.SourceFile
	path           string
	insertAt       int
	delta          int
	implementation *ast.Node
	// original is the accepted program's node for the same implementation:
	// the twin's parameters answer their written/unwritten question through it
	// (parameterSubjectRootsLocked), because the accepted program has already
	// walked that file's assignments once and the twin's cold checker would
	// walk it again per twin.
	original *ast.Node
	premises []typefacts.ParameterPremise
}

// originalLocation maps a location reported over the twin's bytes back to the
// original file's. Every node the census reports lies inside the
// implementation, which lies wholly after the insertion, so the mapping is one
// subtraction; a location in another file, or before the insertion, is
// returned unchanged.
func (t *premiseTwin) originalLocation(location typefacts.Location) typefacts.Location {
	if location.Path != t.path || location.StartByte < t.insertAt+t.delta {
		return location
	}
	location.StartByte -= t.delta
	location.EndByte -= t.delta
	return location
}

// formsMayClearUnderTypes answers whether any recorded form is of a kind the
// classifier decides from the operand's type — the only kinds a declared
// signature can change. The twin is built for those and for nothing else: an
// accessor form is decided by the member's declaration, which a declared
// parameter type does not turn into runtime bytes.
func formsMayClearUnderTypes(forms []typefacts.UncensusedInvokingForm) bool {
	for _, form := range forms {
		switch form.Kind {
		case typefacts.UncensusedCoercion, typefacts.UncensusedIterationProtocol,
			typefacts.UncensusedAwaitThen:
			return true
		}
	}
	return false
}

// premiseAnnotationAnchor is the statement the JSDoc comment is inserted
// before: the function declaration itself, or the variable statement whose
// single declarator holds the arrow or function expression. Nil for every
// other shape — a method, a property assignment, a declarator list with more
// than one binding — because the compiler reads a `@type` tag on those as
// something else or not at all.
func premiseAnnotationAnchor(implementation *ast.Node) *ast.Node {
	if implementation == nil {
		return nil
	}
	if ast.IsFunctionDeclaration(implementation) {
		return implementation
	}
	if !ast.IsArrowFunction(implementation) && !ast.IsFunctionExpression(implementation) {
		return nil
	}
	declaration := implementation.Parent
	for declaration != nil && ast.IsParenthesizedExpression(declaration) {
		declaration = declaration.Parent
	}
	if declaration == nil || !ast.IsVariableDeclaration(declaration) ||
		identityPreservingUnwrap(declaration.Initializer()) != implementation {
		return nil
	}
	list := declaration.Parent
	if list == nil || nodeKindName(list) != "VariableDeclarationList" ||
		len(list.AsVariableDeclarationList().Declarations.Nodes) != 1 {
		return nil
	}
	statement := list.Parent
	if statement == nil || !ast.IsVariableStatement(statement) {
		return nil
	}
	return statement
}

// jsDocTypeTagKinds are the JSDoc tags the compiler reads as a type of the
// declaration they annotate. An anchor that already carries one is not
// annotated again: the two tags would compete, and which one the checker
// honours is not a premise this file states.
//
// A tag whose type is *optional* — `@param seconds - Time in seconds.`,
// `@return milliseconds - Converted time.`, the shape bundled output keeps
// from the authors' prose — states a type only when it carries a braced type
// expression, and only then blocks the premise. `@template`, `@this` and
// `@overload` change the signature whatever they carry.
var jsDocTypeTagKinds = map[string]bool{
	// true: the tag states a type only through a type expression.
	"JSDocTypeTag": true, "JSDocParameterTag": true, "JSDocReturnTag": true,
	"JSDocTypedefTag": true, "JSDocSatisfiesTag": true, "JSDocCallbackTag": true,
	// false: the tag's presence alone changes the declaration's typing.
	"JSDocTemplateTag": false, "JSDocThisTag": false, "JSDocOverloadTag": false,
}

func carriesJSDocTypeTag(anchor *ast.Node, sourceFile *ast.SourceFile) bool {
	found := false
	for _, comment := range anchor.JSDoc(sourceFile) {
		comment.ForEachChild(func(child *ast.Node) bool {
			if needsExpression, typed := jsDocTypeTagKinds[nodeKindName(child)]; typed {
				if !needsExpression || child.TypeExpression() != nil {
					found = true
				}
			}
			return found
		})
	}
	return found
}

// declarationModuleSpecifier is the module specifier under which a declaration
// file's exports are reachable from an `import()` type: the file's own path
// with the declaration extension replaced by the runtime spelling the compiler
// maps back to it. `.d.ts` → no extension (the compiler tries `.d.ts`),
// `.d.mts` → `.mjs`, `.d.cts` → `.cjs`.
func declarationModuleSpecifier(fileName string) (string, bool) {
	for _, pair := range [][2]string{{".d.mts", ".mjs"}, {".d.cts", ".cjs"}, {".d.ts", ""}} {
		if stem, ok := strings.CutSuffix(fileName, pair[0]); ok {
			return stem + pair[1], true
		}
	}
	return "", false
}

// isJavaScriptSourceFile is the extension test for a file whose parameters the
// checker types as `any` unless a JSDoc says otherwise.
func isJavaScriptSourceFile(sourceFile *ast.SourceFile) bool {
	if sourceFile == nil || sourceFile.IsDeclarationFile {
		return false
	}
	switch strings.ToLower(filepath.Ext(sourceFile.FileName())) {
	case ".js", ".mjs", ".cjs", ".jsx":
		return true
	}
	return false
}

// isIdentifierName answers whether an export name can be written as the
// qualifier of an `import()` type.
func isIdentifierName(name string) bool {
	if name == "" {
		return false
	}
	for index, r := range name {
		if r == '_' || r == '$' || (r >= 'a' && r <= 'z') || (r >= 'A' && r <= 'Z') {
			continue
		}
		if index > 0 && r >= '0' && r <= '9' {
			continue
		}
		return false
	}
	return true
}

// declaredExportName finds the name under which the declaration module exports
// the target symbol — by identity through the module's export table, never by
// the symbol's own name, which `export default function clamp` and
// `export { clamp as limit }` both get wrong.
func (p *project) declaredExportName(module *ast.SourceFile, target *ast.Symbol) (string, bool) {
	if module == nil || module.Symbol == nil || target == nil {
		return "", false
	}
	for _, exported := range p.checker.GetExportsOfModule(module.Symbol) {
		if exported == nil || p.canonicalSymbol(exported) != target {
			continue
		}
		if !isIdentifierName(exported.Name) {
			return "", false
		}
		return exported.Name, true
	}
	return "", false
}

// typeDeclarationIdentity names a type beyond its printed text: its flags, the
// file and position of its *named* symbol's first declaration — a class,
// interface or enum — and the same for the alias it was written through. An
// anonymous function or object literal type carries a fresh symbol per
// spelling, so its own declaration says nothing and the printed text compares
// it; but the alias it came through does, and so do the flags. Both matter
// because the compiler prints an *unresolved* type reference by the name it
// was written under: a spelled `@param {EasingFunction}` the twin cannot
// resolve prints exactly like the declaration's `EasingFunction`, and only its
// flags (an error type) and its missing alias tell them apart.
func typeDeclarationIdentity(value *checker.Type) string {
	if value == nil {
		return ""
	}
	identity := fmt.Sprintf("flags:%d", value.Flags())
	if symbol := value.Symbol(); symbol != nil &&
		symbol.Flags&(ast.SymbolFlagsClass|ast.SymbolFlagsInterface|ast.SymbolFlagsEnum) != 0 {
		identity += "|symbol:" + symbolDeclarationIdentity(symbol)
	}
	if alias := value.Alias(); alias != nil && alias.Symbol() != nil {
		identity += "|alias:" + symbolDeclarationIdentity(alias.Symbol())
	}
	return identity
}

// symbolDeclarationIdentity is the file and position of a symbol's first
// declaration, or "" when it has none.
func symbolDeclarationIdentity(symbol *ast.Symbol) string {
	if symbol == nil || len(symbol.Declarations) == 0 || symbol.Declarations[0] == nil {
		return ""
	}
	declaration := symbol.Declarations[0]
	sourceFile := ast.GetSourceFileOfNode(declaration)
	if sourceFile == nil {
		return ""
	}
	return fmt.Sprintf("%s:%d", sourceFile.FileName(), declaration.Pos())
}

// premiseCensusKey identifies one premised classification within a
// generation: the implementation's exact span and the annotation the twin
// carried, which names the declaration module and export the premise binds.
type premiseCensusKey struct {
	location   typefacts.Location
	annotation string
}

// premiseCensusResult is what the memo keeps: the forms classified under the
// premise and the premises established, or the refusal.
type premiseCensusResult struct {
	forms    []typefacts.UncensusedInvokingForm
	premises []typefacts.ParameterPremise
	refusal  string
}

// premisedFormCensusLocked classifies the implementation's form census under
// its declared signature, memoized per generation; see premiseCensuses.
func (p *project) premisedFormCensusLocked(
	ctx context.Context,
	implementation *ast.Node,
	premise *declaredSignaturePremise,
) premiseCensusResult {
	imported, refusal := p.premiseAnnotationLocked(implementation, premise)
	if refusal != "" {
		return premiseCensusResult{refusal: refusal}
	}
	key := premiseCensusKey{location: nodeLocation(implementation), annotation: imported}
	if cached, ok := p.premiseCensuses[key]; ok {
		return cached
	}
	var result premiseCensusResult
	// The spelled annotation first: it adds no module reference, so the
	// compiler splices the twin into the accepted program instead of
	// rebuilding one, and the falsifier below decides whether the spelling
	// re-established the declared types. The `import()` annotation is the
	// fallback for a type the spelling cannot name from the twin's scope.
	var twin *premiseTwin
	if spelled := p.spelledPremiseAnnotationLocked(implementation, premise); spelled != "" {
		twin, _ = p.declaredSignatureTwinLocked(ctx, implementation, premise, spelled, true)
	}
	if twin == nil {
		twin, refusal = p.declaredSignatureTwinLocked(ctx, implementation, premise, imported, false)
	}
	if twin != nil {
		result.forms = p.uncensusedInvokingFormCensusUnderPremiseLocked(twin)
		result.premises = twin.premises
	} else {
		result.refusal = refusal
	}
	if p.premiseCensuses == nil {
		p.premiseCensuses = make(map[premiseCensusKey]premiseCensusResult)
	}
	p.premiseCensuses[key] = result
	return result
}

// spelledPremiseAnnotationLocked spells the declared signature as one `@param`
// tag per parameter and a `@returns` tag, each type printed by the accepted
// checker. A printed type resolves in the twin only when every name it uses
// is global — a primitive, a literal union, a `lib` interface, a structural
// literal — so this is an attempt, not a premise: declaredSignatureTwinLocked
// compares every parameter's type and the return type on the twin against the
// declared ones and refuses the twin when the spelling resolved to anything
// else. Identifier parameters are matched by name; a pattern parameter takes a
// positional tag, which the compiler matches by tag order, under a name no
// parameter can carry. Empty when a parameter cannot be named at all.
func (p *project) spelledPremiseAnnotationLocked(
	implementation *ast.Node,
	premise *declaredSignaturePremise,
) string {
	var tags strings.Builder
	tags.WriteString("/**")
	for index, parameter := range implementation.Parameters() {
		declared := checker.Checker_getTypeAtPosition(p.checker, premise.signature, index)
		if declared == nil {
			return ""
		}
		text := p.checker.TypeToString(declared)
		if text == "" || strings.ContainsAny(text, "*/\n\r") {
			return ""
		}
		name := fmt.Sprintf("__solidTypefactsParameter%d", index)
		if binding := parameter.Name(); binding != nil && ast.IsIdentifier(binding) {
			name = binding.Text()
		}
		fmt.Fprintf(&tags, " @param {%s} %s", text, name)
	}
	returned := checker.Checker_getReturnTypeOfSignature(p.checker, premise.signature)
	if returned == nil {
		return ""
	}
	text := p.checker.TypeToString(returned)
	if text == "" || strings.ContainsAny(text, "*/\n\r") {
		return ""
	}
	fmt.Fprintf(&tags, " @returns {%s} */ ", text)
	return tags.String()
}

// premiseAnnotationLocked decides whether the implementation can carry a
// declared-signature premise at all and, when it can, spells the JSDoc
// comment. Everything here is answered from the accepted program alone, so a
// refusal costs no twin.
func (p *project) premiseAnnotationLocked(
	implementation *ast.Node,
	premise *declaredSignaturePremise,
) (string, string) {
	if premise == nil || premise.signature == nil || premise.declaration == nil || premise.target == nil {
		return "", "no declared signature"
	}
	sourceFile := ast.GetSourceFileOfNode(implementation)
	if !isJavaScriptSourceFile(sourceFile) {
		return "", "implementation is not in a JavaScript file"
	}
	declarationFile := ast.GetSourceFileOfNode(premise.declaration)
	if declarationFile == nil || !declarationFile.IsDeclarationFile {
		return "", "declared signature is not in a declaration file"
	}
	if premise.signature.HasRestParameter() {
		return "", "declared signature has a rest parameter"
	}
	declaredParameters := premise.signature.Parameters()
	parameters := implementation.Parameters()
	if len(parameters) != len(declaredParameters) {
		return "", fmt.Sprintf(
			"implementation declares %d parameter(s), the signature %d",
			len(parameters), len(declaredParameters),
		)
	}
	for _, parameter := range parameters {
		if declaration := parameter.AsParameterDeclaration(); declaration == nil || declaration.DotDotDotToken != nil {
			return "", "implementation has a rest parameter"
		}
	}
	declaredTypes := make([]*checker.Type, len(declaredParameters))
	informative := false
	for index := range declaredParameters {
		declaredTypes[index] = checker.Checker_getTypeAtPosition(p.checker, premise.signature, index)
		if declaredTypes[index] != nil && declaredTypes[index].Flags()&checker.TypeFlagsAny == 0 {
			informative = true
		}
	}
	if !informative {
		return "", "every declared parameter type is any"
	}
	anchor := premiseAnnotationAnchor(implementation)
	if anchor == nil {
		return "", "implementation is neither a function declaration nor the sole initializer of a variable statement"
	}
	if carriesJSDocTypeTag(anchor, sourceFile) {
		return "", "declaration already carries a JSDoc type tag"
	}
	name, ok := p.declaredExportName(declarationFile, premise.target)
	if !ok {
		return "", "declaration module exports the signature under no identifier name"
	}
	specifier, ok := declarationModuleSpecifier(declarationFile.FileName())
	if !ok || strings.Contains(specifier, "*/") || strings.ContainsAny(specifier, "\"\\\n\r") {
		return "", "declaration file path cannot be spelled in an import type"
	}
	return "/** @type {typeof import(" + strconv.Quote(specifier) + ")." + name + "} */ ", ""
}

// declaredSignatureTwinLocked builds and checks the premise twin for one
// implementation whose annotation premiseAnnotationLocked already admitted,
// or refuses with the reason. A refusal is not an error: the caller keeps the
// census it already has.
func (p *project) declaredSignatureTwinLocked(
	ctx context.Context,
	implementation *ast.Node,
	premise *declaredSignaturePremise,
	annotation string,
	spelled bool,
) (*premiseTwin, string) {
	sourceFile := ast.GetSourceFileOfNode(implementation)
	if sourceFile == nil {
		return nil, "implementation has no source file"
	}
	declaredParameters := premise.signature.Parameters()
	parameters := implementation.Parameters()
	declaredTypes := make([]*checker.Type, len(declaredParameters))
	for index := range declaredParameters {
		declaredTypes[index] = checker.Checker_getTypeAtPosition(p.checker, premise.signature, index)
	}
	insertAt := nodeLocation(premiseAnnotationAnchor(implementation)).StartByte
	text := sourceFile.Text()
	if insertAt < 0 || insertAt > len(text) {
		return nil, "annotation anchor is outside the file"
	}
	twinText := text[:insertAt] + annotation + text[insertAt:]

	if err := ctx.Err(); err != nil {
		return nil, err.Error()
	}
	fs := p.fs.clone()
	fs.set(sourceFile.FileName(), twinText)
	host := &premiseHost{
		CompilerHost: compiler.NewCompilerHost(typeScriptPathDir(p.configPath), fs, bundled.LibPath(), nil, nil),
		accepted:     p.program,
		twin:         string(sourceFile.Path()),
	}
	program, _, _ := p.program.UpdateProgram(sourceFile.Path(), host, nil)
	if program == nil {
		return nil, "twin program could not be built"
	}
	// The file object *in the returned program*, not the one UpdateProgram
	// hands back: when the edit cannot be spliced into the old program (an
	// added `import()` type is a new module reference) the compiler rebuilds
	// the program and parses the file a second time, and the first parse is a
	// tree no binder ever visited.
	twinFile := program.GetSourceFileByPath(sourceFile.Path())
	if twinFile == nil {
		return nil, "twin program lost the implementation's file"
	}
	program.BindSourceFiles()
	twinChecker, release := program.GetTypeChecker(ctx)
	if twinChecker == nil {
		if release != nil {
			release()
		}
		return nil, "twin program has no checker"
	}
	twin := &premiseTwin{
		program:  program,
		checker:  twinChecker,
		release:  release,
		file:     twinFile,
		path:     filepath.Clean(sourceFile.FileName()),
		insertAt: insertAt,
		delta:    len(annotation),
		original: implementation,
	}
	original := nodeLocation(implementation)
	matches := exactFunctionLikeDeclarationsAt(twinFile, typefacts.Location{
		Path:      original.Path,
		StartByte: original.StartByte + twin.delta,
		EndByte:   original.EndByte + twin.delta,
	})
	if len(matches) != 1 || matches[0].Kind != implementation.Kind {
		twin.release()
		return nil, "twin declaration is not exactly the implementation"
	}
	twin.implementation = matches[0]
	twinParameters := twin.implementation.Parameters()
	if len(twinParameters) != len(parameters) {
		twin.release()
		return nil, "twin declaration has a different arity"
	}
	twin.premises = make([]typefacts.ParameterPremise, len(parameters))
	for index, parameter := range twinParameters {
		name := parameter.Name()
		if name == nil {
			twin.release()
			return nil, fmt.Sprintf("parameter %d has no binding", index)
		}
		established := twinChecker.GetTypeAtLocation(name)
		declared := declaredTypes[index]
		if established == nil || declared == nil {
			twin.release()
			return nil, fmt.Sprintf("parameter %d has no type on one side", index)
		}
		declaredText := p.checker.TypeToString(declared)
		if establishedText := twinChecker.TypeToString(established); establishedText != declaredText {
			twin.release()
			return nil, fmt.Sprintf(
				"parameter %d types differ: twin %q, declared %q", index, establishedText, declaredText,
			)
		}
		if typeDeclarationIdentity(established) != typeDeclarationIdentity(declared) {
			twin.release()
			return nil, fmt.Sprintf("parameter %d types name different declarations", index)
		}
		twin.premises[index] = typefacts.ParameterPremise{Index: index, Type: declaredText}
	}
	// The return type too, for a *spelled* twin: it is what types a returned
	// arrow's parameters contextually, and a spelling that named the wrong
	// type there would silently classify those parameters under something the
	// declaration did not say. The `import()` twin needs no such check — its
	// return type is the declaration's own, by identity — and cannot pass one
	// reliably: the compiler answers the function's *inferred* return type,
	// which prints structurally where the declaration prints an alias.
	if spelled {
		twinSignature := twinChecker.GetSignatureFromDeclaration(twin.implementation)
		declaredReturn := checker.Checker_getReturnTypeOfSignature(p.checker, premise.signature)
		if twinSignature == nil || declaredReturn == nil {
			twin.release()
			return nil, "return type is unavailable on one side"
		}
		establishedReturn := checker.Checker_getReturnTypeOfSignature(twinChecker, twinSignature)
		if establishedReturn == nil ||
			twinChecker.TypeToString(establishedReturn) != p.checker.TypeToString(declaredReturn) ||
			typeDeclarationIdentity(establishedReturn) != typeDeclarationIdentity(declaredReturn) {
			twin.release()
			return nil, "return types differ between the twin and the declaration"
		}
	}
	return twin, ""
}

// uncensusedInvokingFormCensusUnderPremiseLocked runs the form census over the
// twin's implementation with the twin's checker and reports every location in
// the original file's bytes. The twin is released before returning, and the
// per-file memo it populated is dropped with it so no twin node outlives the
// census.
func (p *project) uncensusedInvokingFormCensusUnderPremiseLocked(
	twin *premiseTwin,
) []typefacts.UncensusedInvokingForm {
	p.formTwin = twin
	forms := p.uncensusedInvokingFormCensusLocked(twin.implementation)
	p.formTwin = nil
	delete(p.assignedSymbols, twin.file)
	twin.release()
	for index := range forms {
		forms[index].Location = twin.originalLocation(forms[index].Location)
		if enclosing := forms[index].EnclosingCallable; enclosing != nil {
			mapped := twin.originalLocation(*enclosing)
			forms[index].EnclosingCallable = &mapped
		}
	}
	return forms
}

// premiseHost is the compiler host the twin program is built with. The
// inserted `import()` type is a module reference the original file did not
// have, so the compiler cannot splice the twin into the accepted program and
// rebuilds one from scratch; left to itself that rebuild would parse every
// file of the program again, and a corpus census pays that once per premised
// export. This host answers every path but the twin's with the accepted
// program's own, already parsed and bound, source-file object — exactly the
// sharing the compiler's own incremental update performs — and parses only the
// twin. A file whose parse options differ from the accepted program's is
// parsed afresh, so an option the rebuild changes is never answered with a
// tree parsed under another.
type premiseHost struct {
	compiler.CompilerHost
	accepted *compiler.Program
	// The twin's path as a string: the compiler's path type is not shimmed,
	// and it is a string kind.
	twin string
}

func (h *premiseHost) GetSourceFile(options ast.SourceFileParseOptions) *ast.SourceFile {
	if string(options.Path) != h.twin {
		if file := h.accepted.GetSourceFileByPath(options.Path); file != nil && file.ParseOptions() == options {
			return file
		}
	}
	return h.CompilerHost.GetSourceFile(options)
}
