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
// A local helper has no declaration to bind, so its premise is the **type of
// each argument slot at the call that reached it, on the caller's twin**
// (handshake protocol 23): the premised census records those types per call
// whose callee is a declaration in the program's own runtime source
// (CallArgumentPremises), the consumer carries the entry for the call it
// followed back as ParameterPremises on the helper's local-declaration demand,
// and the helper is classified on a twin of its own carrying one spelled
// `@param {<type>} <name>` per premised slot, held to the same falsifier — the
// twin's parameter type must print as the demanded text and carry the
// demanded identity — before the transcript echoes the premise. A helper
// reached from two callers with different argument types is two demands and
// two twins; a slot the caller's twin typed `any` is stated nowhere and stays
// `any` on the helper.
//
// What is deliberately outside this premise: an implementation whose
// declaration is not in a declaration file (its parameter types are already
// the checker's), a declaration with a rest parameter or a different arity
// from its implementation, and a declaration whose anchor already carries a
// JSDoc tag the compiler would read as a type.

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
	// arguments is what the premised census recorded at each call to a
	// runtime-source declaration inside the implementation: the type of every
	// informative written argument slot on this twin, in the original file's
	// coordinates. See callArgumentPremisesLocked.
	arguments []typefacts.CallArgumentPremise
}

// slotPremise is one parameter position and the type it must carry on the
// twin: the printed text and the declaration identity. For the root both come
// from the declared signature through the accepted checker; for a helper both
// come from the demand, which carried them from the caller's twin.
type slotPremise struct {
	index    int
	text     string
	identity string
	// spelling is the form the annotation was written with when the printed
	// text names nothing in the twin's own module (ADR 0046). It is echoed
	// back on the established premise so a consumer's byte-for-byte comparison
	// holds; it is never part of the falsifier, which is text and identity.
	spelling string
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
//
// A declaration inside the twin's own file is reported in the *original*
// file's coordinates (twin may be nil for a type of the accepted program), so
// an identity taken on one twin compares equal to the same declaration's
// identity taken on another twin of the same file or on the accepted program.
func typeDeclarationIdentity(value *checker.Type, twin *premiseTwin) string {
	if value == nil {
		return ""
	}
	identity := fmt.Sprintf("flags:%d", value.Flags())
	if symbol := value.Symbol(); symbol != nil &&
		symbol.Flags&(ast.SymbolFlagsClass|ast.SymbolFlagsInterface|ast.SymbolFlagsEnum) != 0 {
		identity += "|symbol:" + symbolDeclarationIdentity(symbol, twin)
	}
	if alias := value.Alias(); alias != nil && alias.Symbol() != nil {
		identity += "|alias:" + symbolDeclarationIdentity(alias.Symbol(), twin)
	}
	return identity
}

// symbolDeclarationIdentity is the file and position of a symbol's first
// declaration, or "" when it has none; a position inside the twin's file is
// mapped back to the original bytes.
func symbolDeclarationIdentity(symbol *ast.Symbol, twin *premiseTwin) string {
	if symbol == nil || len(symbol.Declarations) == 0 || symbol.Declarations[0] == nil {
		return ""
	}
	declaration := symbol.Declarations[0]
	sourceFile := ast.GetSourceFileOfNode(declaration)
	if sourceFile == nil {
		return ""
	}
	position := declaration.Pos()
	if twin != nil && sourceFile == twin.file && position >= twin.insertAt+twin.delta {
		position -= twin.delta
	}
	return fmt.Sprintf("%s:%d", sourceFile.FileName(), position)
}

// premiseCensusKey identifies one premised classification within a
// generation: the implementation's exact span and the annotation the twin
// carried — which names the declaration module and export a root premise
// binds — together with the exact slot premises a helper was demanded under,
// identities included, so two demands that spell alike but bind different
// declarations never share an entry.
type premiseCensusKey struct {
	location   typefacts.Location
	annotation string
	demanded   string
}

// premiseCensusResult is what the memo keeps: the forms classified under the
// premise, the premises established and the argument types recorded at each
// local call, or the refusal.
type premiseCensusResult struct {
	forms     []typefacts.UncensusedInvokingForm
	premises  []typefacts.ParameterPremise
	arguments []typefacts.CallArgumentPremise
	// primitiveCompletion is ADR 0045's fact, and it belongs to the *premised*
	// classification: the same helper censused under two argument premises may
	// return a primitive under one and not the other, so it is computed on the
	// twin beside the forms and never carried over from the accepted program.
	primitiveCompletion bool
	refusal             string
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
		result.forms, result.arguments, result.primitiveCompletion =
			p.uncensusedInvokingFormCensusUnderPremiseLocked(twin)
		result.premises = twin.premises
	} else {
		result.refusal = refusal
	}
	p.rememberPremiseCensus(key, result)
	return result
}

func (p *project) rememberPremiseCensus(key premiseCensusKey, result premiseCensusResult) {
	if p.premiseCensuses == nil {
		p.premiseCensuses = make(map[premiseCensusKey]premiseCensusResult)
	}
	p.premiseCensuses[key] = result
}

// demandedPremiseCensusLocked classifies a local declaration's form census
// under the slot premises a consumer demanded — the argument types the
// caller's premised census recorded at the call that reached it — on a spelled
// twin, memoized per generation like the root's. The result echoes exactly the
// demanded premises when the twin bound every one of them, and refuses
// otherwise; there is no `import()` fallback, because a helper has no
// declaration module to name.
func (p *project) demandedPremiseCensusLocked(
	ctx context.Context,
	implementation *ast.Node,
	demanded []typefacts.ParameterPremise,
) premiseCensusResult {
	annotation, expected, refusal := p.demandedPremiseAnnotationLocked(implementation, demanded)
	if refusal != "" {
		return premiseCensusResult{refusal: refusal}
	}
	var demandedKey strings.Builder
	for _, premise := range demanded {
		fmt.Fprintf(&demandedKey, "%d\x00%s\x00%s\x00", premise.Index, premise.Type, premise.Identity)
	}
	key := premiseCensusKey{
		location: nodeLocation(implementation), annotation: annotation, demanded: demandedKey.String(),
	}
	if cached, ok := p.premiseCensuses[key]; ok {
		return cached
	}
	var result premiseCensusResult
	twin, refusal := p.premiseTwinLocked(ctx, implementation, expected, annotation, nil)
	if twin != nil {
		result.forms, result.arguments, result.primitiveCompletion =
			p.uncensusedInvokingFormCensusUnderPremiseLocked(twin)
		result.premises = twin.premises
	} else {
		result.refusal = refusal
	}
	p.rememberPremiseCensus(key, result)
	return result
}

// demandedPremiseAnnotationLocked decides whether the local declaration can
// carry the demanded slot premises and spells them: one `@param {<type>}
// <name>` per demanded slot, in position order. Every parameter must be a
// plain identifier — the compiler matches a tag to a pattern parameter by tag
// *order*, and a partial premise set would then bind the wrong slot — and the
// demanded texts must be spellable inside a comment. Answered from the
// accepted program alone, so a refusal costs no twin.
func (p *project) demandedPremiseAnnotationLocked(
	implementation *ast.Node,
	demanded []typefacts.ParameterPremise,
) (string, []slotPremise, string) {
	if len(demanded) == 0 {
		return "", nil, "no premise demanded"
	}
	sourceFile := ast.GetSourceFileOfNode(implementation)
	if !isJavaScriptSourceFile(sourceFile) {
		return "", nil, "implementation is not in a JavaScript file"
	}
	parameters := implementation.Parameters()
	anchor := premiseAnnotationAnchor(implementation)
	if anchor == nil {
		return "", nil, "implementation is neither a function declaration nor the sole initializer of a variable statement"
	}
	if carriesJSDocTypeTag(anchor, sourceFile) {
		return "", nil, "declaration already carries a JSDoc type tag"
	}
	names := make([]string, len(parameters))
	for index, parameter := range parameters {
		declaration := parameter.AsParameterDeclaration()
		if declaration == nil || declaration.DotDotDotToken != nil {
			return "", nil, "implementation has a rest parameter"
		}
		binding := parameter.Name()
		if binding == nil || !ast.IsIdentifier(binding) {
			return "", nil, fmt.Sprintf("parameter %d is not a plain identifier", index)
		}
		names[index] = binding.Text()
	}
	var tags strings.Builder
	tags.WriteString("/**")
	expected := make([]slotPremise, 0, len(demanded))
	previous := -1
	for _, premise := range demanded {
		if premise.Index <= previous || premise.Index >= len(parameters) {
			return "", nil, fmt.Sprintf("premise names parameter %d of %d, out of order or out of range", premise.Index, len(parameters))
		}
		previous = premise.Index
		if premise.Type == "" {
			return "", nil, fmt.Sprintf("premise for parameter %d has no type text", premise.Index)
		}
		// The spelling when the caller's twin could compute one, because the
		// printed text alone names nothing in a JavaScript module; the
		// falsifier below still holds the twin to Type and Identity, so a
		// spelling that resolved to something else refuses.
		written := premise.Type
		if premise.Spelling != "" {
			written = premise.Spelling
		}
		// The comment terminator and a line break, and only those: a spelling
		// is an `import("./pkg/index")` path and is full of slashes.
		if strings.Contains(written, "*/") || strings.ContainsAny(written, "\n\r") {
			return "", nil, fmt.Sprintf("premise for parameter %d cannot be spelled in a comment", premise.Index)
		}
		fmt.Fprintf(&tags, " @param {%s} %s", written, names[premise.Index])
		expected = append(expected, slotPremise{
			index:    premise.Index,
			text:     premise.Type,
			identity: premise.Identity,
			spelling: premise.Spelling,
		})
	}
	tags.WriteString(" */ ")
	return tags.String(), expected, ""
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
// census it already has. Every parameter position is expected to carry the
// declared signature's type at that position; a *spelled* twin's return type
// is checked too (see premiseTwinLocked).
func (p *project) declaredSignatureTwinLocked(
	ctx context.Context,
	implementation *ast.Node,
	premise *declaredSignaturePremise,
	annotation string,
	spelled bool,
) (*premiseTwin, string) {
	declaredParameters := premise.signature.Parameters()
	expected := make([]slotPremise, len(declaredParameters))
	for index := range declaredParameters {
		declared := checker.Checker_getTypeAtPosition(p.checker, premise.signature, index)
		if declared == nil {
			return nil, fmt.Sprintf("parameter %d has no declared type", index)
		}
		expected[index] = slotPremise{
			index:    index,
			text:     p.checker.TypeToString(declared),
			identity: typeDeclarationIdentity(declared, nil),
		}
	}
	var returnCheck *checker.Signature
	if spelled {
		returnCheck = premise.signature
	}
	twin, refusal := p.premiseTwinLocked(ctx, implementation, expected, annotation, returnCheck)
	if twin != nil {
		// A root premise is bound to the declared signature by its text alone
		// (census_root_premises); the identity and the spelling are the
		// producer's own and travel only where a consumer must echo them back.
		for index := range twin.premises {
			twin.premises[index].Identity = ""
			twin.premises[index].Spelling = ""
		}
	}
	return twin, refusal
}

// premiseTwinLocked builds the twin of the implementation's file with
// `annotation` inserted before the anchor, checks it, and holds every expected
// slot to the falsifier: the twin's parameter type at that position must print
// as the expected text and carry the expected declaration identity. With
// `returnCheck`, the twin's return type must also print and resolve as that
// signature's — a spelled root twin's obligation, because the return type is
// what types a returned arrow's parameters contextually, and a spelling that
// named the wrong type there would silently classify those parameters under
// something the declaration did not say. The `import()` twin needs no such
// check — its return type is the declaration's own, by identity — and cannot
// pass one reliably: the compiler answers the function's *inferred* return
// type, which prints structurally where the declaration prints an alias. A
// helper's twin has no declaration to check a return type against.
func (p *project) premiseTwinLocked(
	ctx context.Context,
	implementation *ast.Node,
	expected []slotPremise,
	annotation string,
	returnCheck *checker.Signature,
) (*premiseTwin, string) {
	sourceFile := ast.GetSourceFileOfNode(implementation)
	if sourceFile == nil {
		return nil, "implementation has no source file"
	}
	parameters := implementation.Parameters()
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
	twin.premises = make([]typefacts.ParameterPremise, 0, len(expected))
	for _, slot := range expected {
		if slot.index < 0 || slot.index >= len(twinParameters) {
			twin.release()
			return nil, fmt.Sprintf("premise names parameter %d of %d", slot.index, len(twinParameters))
		}
		name := twinParameters[slot.index].Name()
		if name == nil {
			twin.release()
			return nil, fmt.Sprintf("parameter %d has no binding", slot.index)
		}
		established := twinChecker.GetTypeAtLocation(name)
		if established == nil {
			twin.release()
			return nil, fmt.Sprintf("parameter %d has no type on the twin", slot.index)
		}
		if establishedText := twinChecker.TypeToString(established); establishedText != slot.text {
			twin.release()
			return nil, fmt.Sprintf(
				"parameter %d types differ: twin %q, premised %q", slot.index, establishedText, slot.text,
			)
		}
		if typeDeclarationIdentity(established, twin) != slot.identity {
			twin.release()
			return nil, fmt.Sprintf("parameter %d types name different declarations", slot.index)
		}
		twin.premises = append(twin.premises, typefacts.ParameterPremise{
			Index: slot.index, Type: slot.text, Identity: slot.identity, Spelling: slot.spelling,
		})
	}
	if returnCheck != nil {
		twinSignature := twinChecker.GetSignatureFromDeclaration(twin.implementation)
		declaredReturn := checker.Checker_getReturnTypeOfSignature(p.checker, returnCheck)
		if twinSignature == nil || declaredReturn == nil {
			twin.release()
			return nil, "return type is unavailable on one side"
		}
		establishedReturn := checker.Checker_getReturnTypeOfSignature(twinChecker, twinSignature)
		if establishedReturn == nil ||
			twinChecker.TypeToString(establishedReturn) != p.checker.TypeToString(declaredReturn) ||
			typeDeclarationIdentity(establishedReturn, twin) != typeDeclarationIdentity(declaredReturn, nil) {
			twin.release()
			return nil, "return types differ between the twin and the declaration"
		}
	}
	return twin, ""
}

// uncensusedInvokingFormCensusUnderPremiseLocked runs the form census over the
// twin's implementation with the twin's checker, records the argument types at
// each local call, and reports every location in the original file's bytes.
// The twin is released before returning, and the per-file memo it populated
// is dropped with it so no twin node outlives the census.
func (p *project) uncensusedInvokingFormCensusUnderPremiseLocked(
	twin *premiseTwin,
) ([]typefacts.UncensusedInvokingForm, []typefacts.CallArgumentPremise, bool) {
	p.formTwin = twin
	forms := p.uncensusedInvokingFormCensusLocked(twin.implementation)
	arguments := p.callArgumentPremisesLocked(twin)
	primitive := p.primitiveCompletionLocked(twin.implementation)
	p.formTwin = nil
	delete(p.assignedSymbols, twin.file)
	twin.release()
	for index := range forms {
		forms[index].Location = twin.originalLocation(forms[index].Location)
		if enclosing := forms[index].EnclosingCallable; enclosing != nil {
			mapped := twin.originalLocation(*enclosing)
			forms[index].EnclosingCallable = &mapped
		}
		// A coercion premise names call locations, and a consumer matches them
		// against the call census's rows — which are reported in the original
		// file's bytes. Left in the twin's coordinates they would match
		// nothing, and the premise would be stated and never granted.
		if premise := forms[index].CoercionPremise; premise != nil {
			for call := range premise.Calls {
				premise.Calls[call] = twin.originalLocation(premise.Calls[call])
			}
		}
	}
	return forms, arguments, primitive
}

// callArgumentPremisesLocked records, for every call or construction inside
// the twin's implementation whose callee is an identifier resolving to a
// declaration in the program's own runtime source, the type the twin's checker
// gives each written argument slot — the premise a consumer may hand back for
// that callee's own census. A call carrying a spread states nothing: a slot
// after the spread has no fixed position. A slot whose type is `any` — or
// prints as nothing a comment can carry — is omitted, and an entry with no
// informative slot is not recorded. Must run while p.formTwin is the twin, so
// canonicalSymbol and formIsRuntimeSourceFile answer on the twin's program.
func (p *project) callArgumentPremisesLocked(twin *premiseTwin) []typefacts.CallArgumentPremise {
	body := twin.implementation.Body()
	if body == nil {
		return nil
	}
	var recorded []typefacts.CallArgumentPremise
	var visit func(node *ast.Node) bool
	visit = func(node *ast.Node) bool {
		if node == nil {
			return false
		}
		if ast.IsCallExpression(node) || ast.IsNewExpression(node) {
			if entry, ok := p.callArgumentPremiseLocked(twin, node); ok {
				recorded = append(recorded, entry)
			}
		}
		node.ForEachChild(visit)
		return false
	}
	body.ForEachChild(visit)
	return recorded
}

func (p *project) callArgumentPremiseLocked(twin *premiseTwin, call *ast.Node) (typefacts.CallArgumentPremise, bool) {
	callee := identityPreservingUnwrap(call.Expression())
	if callee == nil || !ast.IsIdentifier(callee) {
		return typefacts.CallArgumentPremise{}, false
	}
	symbol := p.canonicalSymbol(twin.checker.GetSymbolAtLocation(callee))
	if symbol == nil || len(symbol.Declarations) == 0 || symbol.Declarations[0] == nil {
		return typefacts.CallArgumentPremise{}, false
	}
	declarationFile := ast.GetSourceFileOfNode(symbol.Declarations[0])
	if declarationFile == nil || declarationFile.IsDeclarationFile || !p.formIsRuntimeSourceFile(declarationFile) {
		return typefacts.CallArgumentPremise{}, false
	}
	arguments := call.Arguments()
	if len(arguments) == 0 || exactArgumentSlots(call) != len(arguments) {
		return typefacts.CallArgumentPremise{}, false
	}
	entry := typefacts.CallArgumentPremise{Call: twin.originalLocation(nodeLocation(call))}
	for index, argument := range arguments {
		if argument == nil {
			continue
		}
		argumentType := twin.checker.GetTypeAtLocation(argument)
		if argumentType == nil || argumentType.Flags()&checker.TypeFlagsAny != 0 {
			continue
		}
		text := twin.checker.TypeToString(argumentType)
		if text == "" || strings.ContainsAny(text, "\n\r") || strings.Contains(text, "*/") {
			continue
		}
		entry.Arguments = append(entry.Arguments, typefacts.ParameterPremise{
			Index:    index,
			Type:     text,
			Identity: typeDeclarationIdentity(argumentType, twin),
			Spelling: p.spellableTypeReferenceLocked(argumentType),
		})
	}
	entry.Arguments = append(
		entry.Arguments, p.omittedSlotPremisesLocked(call, len(arguments))...,
	)
	if len(entry.Arguments) == 0 {
		return typefacts.CallArgumentPremise{}, false
	}
	return entry, true
}

// omittedSlotPremisesLocked answers the premise for every parameter slot the
// call does not write: at runtime such a parameter receives `undefined`, which
// is a primitive and therefore a *stronger* premise than the `any` the callee's
// own unannotated parameter would otherwise carry (ADR 0049).
//
// It is a semantic fact rather than a type one — the call has fewer arguments
// than the callee has parameters, and the specification fills the rest with
// `undefined` — so it is available where no declaration is. Three slots are
// skipped rather than stated:
//
//   - one whose parameter has an **initializer**, because the value is then
//     the default rather than `undefined`;
//   - a **rest** parameter, and anything that is not a plain identifier
//     binding, because there is no single slot to speak for;
//   - one the callee's own file **writes**, because the parameter would then
//     hold something the premise does not describe by the time the body reads
//     it, and a type of `undefined` would classify that read wrongly.
//
// A skipped slot leaves the list strictly increasing, which is what the
// annotation and the consumer both require.
func (p *project) omittedSlotPremisesLocked(call *ast.Node, written int) []typefacts.ParameterPremise {
	callee := p.formRuntimeCalleeDeclarationLocked(call)
	if callee == nil {
		return nil
	}
	var premises []typefacts.ParameterPremise
	for index, parameter := range callee.Parameters() {
		if index < written {
			continue
		}
		declaration := parameter.AsParameterDeclaration()
		name := parameter.Name()
		if declaration == nil || declaration.DotDotDotToken != nil ||
			parameter.Initializer() != nil || name == nil || !ast.IsIdentifier(name) {
			continue
		}
		symbol := p.canonicalSymbol(p.formChecker().GetSymbolAtLocation(name))
		if symbol == nil || p.symbolIsAssignedLocked(symbol, callee) {
			continue
		}
		premises = append(premises, typefacts.ParameterPremise{
			Index: index, Type: "undefined", Identity: undefinedTypeIdentity(),
		})
	}
	return premises
}

// undefinedTypeIdentity is typeDeclarationIdentity's answer for the intrinsic
// `undefined` type, which has no symbol and no alias and so is its flags
// alone. Built from the constant rather than from a type object because the
// caller's twin has no expression of that type to ask about;
// TestOmittedArgumentSlotsArePremisedAsUndefined pins that the two agree.
func undefinedTypeIdentity() string {
	return fmt.Sprintf("flags:%d", checker.TypeFlagsUndefined)
}

// spellableTypeReferenceLocked answers a form of a type that resolves from a
// module which cannot name it directly — `import("<specifier>").<Name>` — or
// "" when the type has no such form (ADR 0046).
//
// The caller's twin resolves `Axis` because the caller's own premise brought
// the declaration module into scope; the *helper's* twin is a plain JavaScript
// module where the bare name resolves to nothing, so the printed text alone
// cannot carry the premise across. This is the same device the root premise
// already uses for a type its spelling cannot name, applied one hop further.
//
// The alias is preferred over the symbol because that is what the printer
// prefers: a `type EasingFunction = …` prints as its alias, and spelling the
// anonymous signature behind it would name a different thing. Only a type
// declared in a **declaration file** and exported from it under an identifier
// name qualifies; everything else answers "".
func (p *project) spellableTypeReferenceLocked(value *checker.Type) string {
	if value == nil {
		return ""
	}
	symbol := value.Symbol()
	if alias := value.Alias(); alias != nil && alias.Symbol() != nil {
		symbol = alias.Symbol()
	}
	symbol = p.canonicalSymbol(symbol)
	if symbol == nil || len(symbol.Declarations) == 0 || symbol.Declarations[0] == nil {
		return ""
	}
	declarationFile := ast.GetSourceFileOfNode(symbol.Declarations[0])
	if declarationFile == nil || !declarationFile.IsDeclarationFile {
		return ""
	}
	name, ok := p.formDeclaredExportName(declarationFile, symbol)
	if !ok {
		return ""
	}
	specifier, ok := declarationModuleSpecifier(declarationFile.FileName())
	if !ok || strings.Contains(specifier, "*/") || strings.ContainsAny(specifier, "\"\\\n\r") {
		return ""
	}
	return "import(" + strconv.Quote(specifier) + ")." + name
}

// formDeclaredExportName is declaredExportName asked through the form checker,
// so it answers about the twin's program when one is being classified.
func (p *project) formDeclaredExportName(module *ast.SourceFile, target *ast.Symbol) (string, bool) {
	if module == nil || module.Symbol == nil || target == nil {
		return "", false
	}
	for _, exported := range p.formChecker().GetExportsOfModule(module.Symbol) {
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

// calleesWorthPremisingLocked answers whether a premised twin of the
// implementation could change any *callee's* census: whether some declaration
// reachable from its body through calls to runtime-source declarations — the
// callees a consumer's census recurses into — records, over its own parameters'
// types, a form a type can clear. The twin is built for that as it is for a
// type-decided form of the body itself; a body whose reachable helpers record
// no such form has no premise worth carrying, and pays nothing.
//
// The walk is bounded by the consumer's own composition depth and a visited
// set, and each declaration's answer is memoized per generation
// (premiseWorth), so a helper shared by many exports is classified once.
func (p *project) calleesWorthPremisingLocked(implementation *ast.Node) bool {
	visited := map[*ast.Node]bool{implementation: true}
	return p.calleesWorthPremisingWithinLocked(implementation, visited, 0)
}

const maxPremiseWorthDepth = 8

func (p *project) calleesWorthPremisingWithinLocked(node *ast.Node, visited map[*ast.Node]bool, depth int) bool {
	if depth >= maxPremiseWorthDepth {
		return false
	}
	body := node.Body()
	if body == nil {
		return false
	}
	worth := false
	var visit func(child *ast.Node) bool
	visit = func(child *ast.Node) bool {
		if worth || child == nil {
			return worth
		}
		if ast.IsCallExpression(child) || ast.IsNewExpression(child) {
			if callee := p.runtimeCalleeDeclarationLocked(child); callee != nil && !visited[callee] {
				visited[callee] = true
				if p.declarationWorthPremisingLocked(callee, visited, depth+1) {
					worth = true
					return true
				}
			}
		}
		child.ForEachChild(visit)
		return worth
	}
	body.ForEachChild(visit)
	return worth
}

// declarationWorthPremisingLocked is the memoized per-declaration half: the
// declaration's own form census records a type-decided form, or one of its
// reachable callees does.
func (p *project) declarationWorthPremisingLocked(declaration *ast.Node, visited map[*ast.Node]bool, depth int) bool {
	if cached, ok := p.premiseWorth[declaration]; ok {
		return cached
	}
	worth := formsMayClearUnderTypes(p.uncensusedInvokingFormCensusLocked(declaration)) ||
		p.calleesWorthPremisingWithinLocked(declaration, visited, depth)
	if p.premiseWorth == nil {
		p.premiseWorth = make(map[*ast.Node]bool)
	}
	p.premiseWorth[declaration] = worth
	return worth
}

// runtimeCalleeDeclarationLocked resolves a call's identifier callee, on the
// accepted program, to the function-like declaration node a consumer's census
// would recurse into — a function declaration, or the arrow or function
// expression that is the whole initializer of a variable declarator — when
// that declaration sits in a runtime source file. Nil for every other callee.
func (p *project) runtimeCalleeDeclarationLocked(call *ast.Node) *ast.Node {
	callee := identityPreservingUnwrap(call.Expression())
	if callee == nil || !ast.IsIdentifier(callee) {
		return nil
	}
	symbol := p.canonicalSymbol(p.checker.GetSymbolAtLocation(callee))
	if symbol == nil || len(symbol.Declarations) == 0 || symbol.Declarations[0] == nil {
		return nil
	}
	declaration := symbol.Declarations[0]
	sourceFile := ast.GetSourceFileOfNode(declaration)
	if sourceFile == nil || sourceFile.IsDeclarationFile || !p.isCurrentSourceFile(sourceFile) {
		return nil
	}
	switch {
	case ast.IsFunctionDeclaration(declaration):
		return declaration
	case ast.IsVariableDeclaration(declaration):
		initializer := identityPreservingUnwrap(declaration.Initializer())
		if initializer != nil && (ast.IsArrowFunction(initializer) || ast.IsFunctionExpression(initializer)) {
			return initializer
		}
	}
	return nil
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
