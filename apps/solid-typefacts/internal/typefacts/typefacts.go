// Package typefacts defines the compiler-independent seam through which a
// consumer asks questions about a configured TypeScript project, and the v3
// protocol the producer answers them over.
package typefacts

import (
	"context"
	"errors"
)

var ErrNotFound = errors.New("type fact not found")

// SymbolID is an opaque identity stable for one Project analysis version.
// Implementations may keep an ID resolvable across updates while its
// declaration is unchanged (durable symbol identity); holders must treat
// cross-update resolution as best-effort — it either answers for the same
// declaration or reports ErrNotFound, never a different symbol.
type SymbolID string

// RuntimeSymbolID is a declaration-derived identity used only for equality
// across aliases and reexports. Unlike SymbolID it is not a lookup handle.
type RuntimeSymbolID string

// RuntimeBindingKind is an exact census over a runtime binding's initializer
// and direct writes. The zero value means the census was not demanded or the
// queried span did not identify a runtime binding. Open means at least one
// possible write could not be classified; Mixed means closed writes disagree.
// Neither is proof of a runtime kind.
type RuntimeBindingKind uint8

const (
	RuntimeBindingAbsent RuntimeBindingKind = iota
	RuntimeBindingCallable
	RuntimeBindingNonCallable
	RuntimeBindingMixed
	RuntimeBindingOpen
)

// TypeDescriptor exposes source identity for named types without leaking a
// backend AST. It is available through the optional TypeDescriber capability.
type TypeDescriptor struct {
	Text              string        `cbor:"text,omitempty" json:"text,omitempty"`
	OriginModule      string        `cbor:"originModule,omitempty" json:"originModule,omitempty"`
	AliasDeclarations []Declaration `cbor:"aliasDeclarations,omitempty" json:"aliasDeclarations,omitempty"`
}

type TypeDescriber interface {
	DescribeTypeAt(context.Context, Location) (TypeDescriptor, error)
}

// Callability is the compiler's call-signature classification for a demanded
// expression. It is derived from TypeChecker.GetSignaturesOfType over the
// actual union constituents, never from rendered type text.
//
// CallabilityUntypedCallable is the signature-less function-supertype family:
// a constituent the compiler permits calling even though it exposes no call
// signature to read. lib.es5.d.ts's Function interface declares
// apply/call/bind and no signature of its own, and CallableFunction,
// NewableFunction, an alias or interface reaching them, and an intersection
// containing one all inherit that shape. The compiler resolves such a call
// through its TS 1.0 §4.12 rule (checker.isUntypedFunctionCall: no signatures
// of either kind, not a union, and assignable to the global Function type) and
// gives it anySignature, so `nonCallable` there would claim the call is illegal
// when the compiler allows it, and `unknown` would claim no domain was closed
// when one was. For a single, non-union constituent the value is exact: it
// proves that constituent *is* callable and that no signature, arity, or
// parameter type can be read from it. At a union the promise is weaker — see
// the Aggregation paragraph below.
//
// It never reaches `object`, `{}`, `Record<string, unknown>`, or an interface
// that merely declares a `bind` method: none of those is assignable to
// Function, and the compiler refuses to call them, so they remain
// CallabilityNonCallable. Aggregation places it below CallabilityCallable and
// above CallabilityMixed: constituents that are all callable in either sense
// answer the weaker of the two, and any non-callable constituent beside a
// callable one still answers CallabilityMixed. That promise is per
// constituent, not a claim about the union's own call: Function | (() => void)
// still carries one readable, arity-enforced call signature that tsc itself
// enforces (a wrong argument count is TS2554), while Function | Merged (two
// constituents each individually in this family, such as a merged
// `declare class C {}` and `interface C extends Function {}`) has tsc refuse
// the call outright (TS2349), because the untyped-call rule's fallback
// explicitly excludes unions. Either way a consumer reading
// CallabilityUntypedCallable as "callable, signature unread" only under-checks
// what it could have proven; it never claims the union's call type-checks.
type Callability string

const (
	CallabilityCallable        Callability = "callable"
	CallabilityUntypedCallable Callability = "untypedCallable"
	CallabilityNonCallable     Callability = "nonCallable"
	CallabilityMixed           Callability = "mixed"
	CallabilityUnknown         Callability = "unknown"
)

// Constructability is the compiler's construct-signature classification for a
// demanded expression: the exact counterpart of Callability, asking `new X()`
// where Callability asks `X()`. It is derived from
// TypeChecker.GetSignaturesOfType with SignatureKindConstruct over the actual
// union constituents, never from rendered type text, and the two are answered
// over the same constituent partition of the same type.
//
// A class declaration's *value* type is the case the type system otherwise
// leaves unanswerable: it is `nonCallable` and `constructable`, because a
// construct signature is not a call signature.
//
// The two facts aggregate independently, so a `mixed` verdict on either one
// does not compose with the other into a per-constituent proof: a union of a
// function, a number, and a constructor answers `mixed` twice over and still
// holds a constituent that is neither.
//
// Unlike Callability it is a compact integer rather than a string, and is
// stored inline in the field padding an EntityFact already carries, so a
// retained row pays nothing for it. As a string it cost every row 16 bytes to
// carry an absence — the same call PrimitiveValueDomain made, for the same
// reason. The zero value is the absence of the fact, not a verdict.
type Constructability uint8

const (
	ConstructabilityAbsent Constructability = iota
	ConstructabilityConstructable
	ConstructabilityNonConstructable
	ConstructabilityMixed
	ConstructabilityUnknown
)

// IsPresent reports whether the fact was answered at all. It is false for a
// span nobody demanded it at; ConstructabilityUnknown is present and means the
// checker closed no domain.
func (c Constructability) IsPresent() bool { return c != ConstructabilityAbsent }

func (c Constructability) String() string {
	switch c {
	case ConstructabilityConstructable:
		return "constructable"
	case ConstructabilityNonConstructable:
		return "nonConstructable"
	case ConstructabilityMixed:
		return "mixed"
	case ConstructabilityUnknown:
		return "unknown"
	default:
		return ""
	}
}

// RuntimeValueDomain summarizes the possible runtime values of a demanded
// expression without exposing a compiler type or relying on rendered type
// text. Unknown means the checker could not provide a closed value domain; in
// that case the three MayBe fields are conservative possibilities rather than
// an exhaustive classification.
//
// The zero value is meaningful: it is the known empty domain produced by
// never. EntityFact uses a pointer so an undemanded fact remains distinct.
type RuntimeValueDomain struct {
	MayBeCallable  bool `cbor:"mayBeCallable,omitempty" json:"mayBeCallable,omitempty"`
	MayBeUndefined bool `cbor:"mayBeUndefined,omitempty" json:"mayBeUndefined,omitempty"`
	MayBeOther     bool `cbor:"mayBeOther,omitempty" json:"mayBeOther,omitempty"`
	Unknown        bool `cbor:"unknown,omitempty" json:"unknown,omitempty"`
}

// PrimitiveValueDomain partitions the possible runtime values of a demanded
// expression by JavaScript primitive kind. A known object/function value sets
// MayBeObject; dynamic, recovery, or unconstrained types set every possibility
// and Unknown. The all-false value is the known empty never domain.
//
// This is deliberately a language fact rather than a serialization verdict.
// Consumers decide which closed subsets their own runtime contracts accept.
type PrimitiveValueDomain struct{ bits uint16 }

const (
	primitiveMayBeString uint16 = 1 << iota
	primitiveMayBeNumber
	primitiveMayBeBoolean
	primitiveMayBeBigInt
	primitiveMayBeSymbol
	primitiveMayBeNull
	primitiveMayBeUndefined
	primitiveMayBeObject
	primitiveNumbersFinite
	primitiveEmpty
)

// NewPrimitiveValueDomain constructs one present, closed domain. The all-false
// input is the known empty never domain, distinct from an undemanded zero value.
func NewPrimitiveValueDomain(stringValue, number, boolean, bigint, symbol, nullValue, undefined, object, numbersFinite bool) PrimitiveValueDomain {
	bits := boolDomainBit(stringValue, primitiveMayBeString) |
		boolDomainBit(number, primitiveMayBeNumber) |
		boolDomainBit(boolean, primitiveMayBeBoolean) |
		boolDomainBit(bigint, primitiveMayBeBigInt) |
		boolDomainBit(symbol, primitiveMayBeSymbol) |
		boolDomainBit(nullValue, primitiveMayBeNull) |
		boolDomainBit(undefined, primitiveMayBeUndefined) |
		boolDomainBit(object, primitiveMayBeObject) |
		boolDomainBit(number && numbersFinite, primitiveNumbersFinite)
	if bits == 0 {
		bits = primitiveEmpty
	}
	return PrimitiveValueDomain{bits: bits}
}

func UnknownPrimitiveValueDomain() PrimitiveValueDomain {
	return PrimitiveValueDomain{bits: ^uint16(0)}
}

func boolDomainBit(value bool, bit uint16) uint16 {
	if value {
		return bit
	}
	return 0
}

func (d PrimitiveValueDomain) IsPresent() bool      { return d.bits != 0 }
func (d PrimitiveValueDomain) MayBeString() bool    { return d.bits&primitiveMayBeString != 0 }
func (d PrimitiveValueDomain) MayBeNumber() bool    { return d.bits&primitiveMayBeNumber != 0 }
func (d PrimitiveValueDomain) MayBeBoolean() bool   { return d.bits&primitiveMayBeBoolean != 0 }
func (d PrimitiveValueDomain) MayBeBigInt() bool    { return d.bits&primitiveMayBeBigInt != 0 }
func (d PrimitiveValueDomain) MayBeSymbol() bool    { return d.bits&primitiveMayBeSymbol != 0 }
func (d PrimitiveValueDomain) MayBeNull() bool      { return d.bits&primitiveMayBeNull != 0 }
func (d PrimitiveValueDomain) MayBeUndefined() bool { return d.bits&primitiveMayBeUndefined != 0 }
func (d PrimitiveValueDomain) MayBeObject() bool    { return d.bits&primitiveMayBeObject != 0 }
func (d PrimitiveValueDomain) NumbersAreFinite() bool {
	return d.MayBeNumber() && d.bits&primitiveNumbersFinite != 0
}
func (d PrimitiveValueDomain) Unknown() bool { return d.bits == ^uint16(0) }
func (d PrimitiveValueDomain) Union(other PrimitiveValueDomain) PrimitiveValueDomain {
	if d.Unknown() || other.Unknown() {
		return UnknownPrimitiveValueDomain()
	}
	bits := (d.bits | other.bits) &^ (primitiveEmpty | primitiveNumbersFinite)
	if bits&primitiveMayBeNumber != 0 &&
		(!d.MayBeNumber() || d.NumbersAreFinite()) &&
		(!other.MayBeNumber() || other.NumbersAreFinite()) {
		bits |= primitiveNumbersFinite
	}
	if bits == 0 && (d.IsPresent() || other.IsPresent()) {
		bits = primitiveEmpty
	}
	return PrimitiveValueDomain{bits: bits}
}

type PrimitiveLiteralKind string

const (
	PrimitiveLiteralString  PrimitiveLiteralKind = "string"
	PrimitiveLiteralNumber  PrimitiveLiteralKind = "number"
	PrimitiveLiteralBoolean PrimitiveLiteralKind = "boolean"
)

// PrimitiveLiteralCandidate is one exact, compiler-proven inhabitant of a
// demanded type. The list is a bounded source of valid construction inputs,
// not an exhaustive domain: broad primitive constituents contribute no value.
type PrimitiveLiteralCandidate struct {
	Kind    PrimitiveLiteralKind `cbor:"kind" json:"kind"`
	String  string               `cbor:"string,omitempty" json:"string,omitempty"`
	Number  float64              `cbor:"number,omitempty" json:"number,omitempty"`
	Boolean bool                 `cbor:"boolean,omitempty" json:"boolean,omitempty"`
}

// ConstantValue is a compiler-proven, span-exact primitive value. Kind selects
// the populated payload; an empty string and numeric zero are real values.
type ConstantValue struct {
	Kind   ConstantValueKind `cbor:"kind" json:"kind"`
	String string            `cbor:"string,omitempty" json:"string,omitempty"`
	Number float64           `cbor:"number,omitempty" json:"number,omitempty"`
}

type ConstantValueKind string

const (
	ConstantValueString ConstantValueKind = "string"
	ConstantValueNumber ConstantValueKind = "number"
)

// ArrayShape is the compiler's array/tuple classification for a demanded
// expression, derived from the checker's own isArrayOrTupleType predicate over
// the actual union constituents. Rendered type text never participates, so an
// aliased tuple classifies as its tuple rather than as the alias's name.
//
// ArrayShapeArray means every constituent is a reference to the global Array or
// ReadonlyArray type, or a tuple. It is deliberately narrower than "array-like":
// an interface extending Array, or any other type merely assignable to
// ReadonlyArray<any>, classifies as ArrayShapeNotArray, because its author chose
// that type over an array.
//
// ArrayShapeNotArray is a positive claim — no constituent is an array or tuple —
// and is what makes the negative usable as proof. ArrayShapeMixed and
// ArrayShapeUnknown are both "not proven either way": Mixed for a union that
// genuinely mixes the two, Unknown for any, unknown, never, error, or an
// unresolvable type.
type ArrayShape string

const (
	ArrayShapeArray    ArrayShape = "array"
	ArrayShapeNotArray ArrayShape = "notArray"
	ArrayShapeMixed    ArrayShape = "mixed"
	ArrayShapeUnknown  ArrayShape = "unknown"
)

// TupleShape describes the tuple type at a demanded span: how many fixed
// element slots it has, whether a rest or variadic tail follows them, and
// whether the first slot holds a callable value.
//
// It is emitted when the type at the exact demanded span resolves to a tuple:
// itself a tuple, or a union whose every value-carrying constituent is one. It is
// never emitted for the global Array/ReadonlyArray types, which have a number
// index signature instead of fixed slots. Absence means "not proven a tuple",
// never "not a tuple".
//
// For a union the fields are the constituents' meet — the slots they all have,
// callable only if all are, and the largest argument requirement among them — so
// what it reports holds whichever constituent the value turns out to be. Nullish
// constituents carry no structure and are skipped, so an optional tuple still
// describes the tuple it is when present; a consumer that also needs the value to
// be present should read RuntimeValueDomain.
//
// This is the structural detail ArrayShape deliberately collapses. A consumer
// asking whether a value satisfies an interface with numbered members, such as
// a two-slot bound-handler pair, needs the slot count and the first slot's
// callability; a consumer asking only "is this iterable as an array" does not.
type TupleShape struct {
	// FixedLength counts initial required-or-optional slots, matching the
	// compiler's own fixedLength.
	FixedLength int `cbor:"fixedLength,omitempty" json:"fixedLength,omitempty"`
	// ExactLength is the tuple's exact runtime element count when every
	// constituent has the same required-only shape. Read it only when
	// ExactLengthKnown is true; optional, rest, variadic, and unequal union
	// shapes deliberately leave it unknown.
	ExactLength      int  `cbor:"exactLength,omitempty" json:"exactLength,omitempty"`
	ExactLengthKnown bool `cbor:"exactLengthKnown,omitempty" json:"exactLengthKnown,omitempty"`
	// HasRest reports a rest or variadic tail after the fixed slots.
	HasRest bool `cbor:"hasRest,omitempty" json:"hasRest,omitempty"`
	// ElementZero is the callability of the first slot's type, or
	// CallabilityUnknown when there is no fixed first slot.
	ElementZero Callability `cbor:"elementZero,omitempty" json:"elementZero,omitempty"`
	// ElementZeroMinimumParameters is the fewest arguments any call signature of
	// the first slot's type requires. A caller can pass more than a function
	// accepts only if the function accepts them, so this is what decides whether
	// the slot can be invoked with a given argument count. Zero when the slot is
	// absent or not callable.
	ElementZeroMinimumParameters int `cbor:"elementZeroMinimumParameters,omitempty" json:"elementZeroMinimumParameters,omitempty"`
}

// ReferenceSpace summarizes the semantic meaning of all compiler-resolved
// references to an imported or aliased symbol.
type ReferenceSpace string

const (
	ReferenceSpaceValue   ReferenceSpace = "value"
	ReferenceSpaceType    ReferenceSpace = "type"
	ReferenceSpaceBoth    ReferenceSpace = "both"
	ReferenceSpaceNeither ReferenceSpace = "neither"
)

// SemanticEntityLookup is the demand-shaped compiler-fact interface used by
// protocol producers and integration tests.
type SemanticEntityLookup interface {
	SemanticEntities(context.Context, []EntityDemand) ([]EntityFact, error)
}

// SourceCall describes one parsed call expression without exposing backend AST
// nodes. Target is alias-resolved for the current project generation.
type SourceCall struct {
	Location  Location
	Callee    Location
	Arguments []Location
	Target    SymbolID
}

// CallDiscoverer is an optional bulk syntax capability. Implementations return
// calls in source order with parser-derived callee and argument boundaries.
type CallDiscoverer interface {
	SourceCalls(context.Context, string) ([]SourceCall, error)
}

// SourceBinding describes a variable initialized directly by a resolved call.
// Names contains one entry for a direct identifier, or one entry per top-level
// array binding slot; omitted or nested slots have zero-value locations.
type SourceBinding struct {
	Array       bool
	Names       []Location
	Initializer SourceCall
}

// BindingDiscoverer is an optional bulk syntax capability for call-initialized
// variable declarations.
type BindingDiscoverer interface {
	SourceBindings(context.Context, string) ([]SourceBinding, error)
}

// SourceFunction describes a named block-bodied function without exposing its
// backend AST node. Parameters retain their complete declaration ranges.
type SourceFunction struct {
	Name       Location
	Body       Location
	Parameters []Location
	Exported   bool
	Async      bool
	Arrow      bool
}

// FunctionDiscoverer is an optional bulk syntax capability for named function
// declarations and direct identifier-bound arrow functions.
type FunctionDiscoverer interface {
	SourceFunctions(context.Context, string) ([]SourceFunction, error)
}

// AsyncFunctionFact describes a function-like expression or declaration using
// parser and checker facts. Target links a local identifier alias to the
// summarized function symbol. CallsAfterAwait contains call expressions whose
// execution is dominated by await on every reachable AST control-flow path;
// calls inside nested functions are excluded.
type AsyncFunctionFact struct {
	Expression      Location
	Symbol          SymbolID
	Target          SymbolID
	CanReturnAsync  bool
	CallsAfterAwait []Location
}

// AsyncFunctionDiscoverer is an optional semantic async/control-flow
// capability. It keeps backend AST details behind the Type Facts seam.
type AsyncFunctionDiscoverer interface {
	SourceAsyncFunctions(context.Context, string) ([]AsyncFunctionFact, error)
}

// AsyncFunctionLookup is the demand-shaped async/control-flow capability the
// retained analysis uses. Implementations return only the function and
// local-alias facts relevant at the requested locations.
type AsyncFunctionLookup interface {
	AsyncFunctionsAt(context.Context, []Location) ([]AsyncFunctionFact, error)
}

// Location identifies a UTF-8 byte range in original source.
type Location struct {
	Path      string `cbor:"path" json:"path"`
	StartByte int    `cbor:"startByte" json:"startByte"`
	EndByte   int    `cbor:"endByte" json:"endByte"`
}

// Declaration is the source-only description of a symbol declaration.
type Declaration struct {
	Name     string   `cbor:"name" json:"name"`
	Kind     string   `cbor:"kind" json:"kind"`
	Location Location `cbor:"location" json:"location"`
}

// ResolvedCallValidity distinguishes a compiler-selected signature from the
// recovery signatures TypeScript creates while reporting failed resolution.
type ResolvedCallValidity string

const (
	ResolvedCallValid      ResolvedCallValidity = "valid"
	ResolvedCallRecovery   ResolvedCallValidity = "recovery"
	ResolvedCallUnresolved ResolvedCallValidity = "unresolved"
)

// CallKind distinguishes ordinary invocation from construction.
type CallKind string

const (
	CallKindUnknown   CallKind = "unknown"
	CallKindCall      CallKind = "call"
	CallKindConstruct CallKind = "construct"
)

// ResolvedDeclaration identifies the declaration selected by overload
// resolution. Symbol and each owner symbol are compiler-resolved identities;
// names and QualifiedName are display metadata.
type ResolvedDeclaration struct {
	Symbol          SymbolID           `cbor:"symbol,omitempty" json:"symbol,omitempty"`
	Name            string             `cbor:"name,omitempty" json:"name,omitempty"`
	Kind            string             `cbor:"kind" json:"kind"`
	Location        Location           `cbor:"location" json:"location"`
	Owners          []DeclarationOwner `cbor:"owners,omitempty" json:"owners,omitempty"`
	QualifiedName   string             `cbor:"qualifiedName,omitempty" json:"qualifiedName,omitempty"`
	OriginModule    string             `cbor:"originModule,omitempty" json:"originModule,omitempty"`
	SourceFile      string             `cbor:"sourceFile,omitempty" json:"sourceFile,omitempty"`
	StandardLibrary bool               `cbor:"standardLibrary,omitempty" json:"standardLibrary,omitempty"`
}

// DeclarationOwner is one compiler declaration containing a selected
// signature declaration, ordered outermost to innermost.
type DeclarationOwner struct {
	Symbol   SymbolID `cbor:"symbol,omitempty" json:"symbol,omitempty"`
	Name     string   `cbor:"name,omitempty" json:"name,omitempty"`
	Kind     string   `cbor:"kind" json:"kind"`
	Location Location `cbor:"location" json:"location"`
}

// ModuleFormat is the compiler's emit module format for one included file, as
// GetEmitModuleFormatOfFile computes it: the file's implied node format when
// the configured module kind defers to it, and the configured module kind
// otherwise. It is the runtime-meaningful half of `module`, not the option's
// text.
//
// Only the three formats that describe a real runtime shape have a value.
// Anything else — including the legacy AMD, UMD, and System kinds, and a
// program with no module emit at all — is ModuleFormatUnknown, which is a
// refusal to characterize rather than a claim about the file.
type ModuleFormat string

const (
	ModuleFormatUnknown  ModuleFormat = ""
	ModuleFormatCommonJS ModuleFormat = "commonjs"
	ModuleFormatESM      ModuleFormat = "esm"
	ModuleFormatPreserve ModuleFormat = "preserve"
)

// ProjectReferenceMapping is the compiler's own record pairing one input file
// with the declaration file emitted from it. It exists only where a configured
// `references` entry covers the file, and it is the **only** declaration-to-
// implementation pairing TypeScript maintains.
//
// It is emphatically not available for the shape almost every published
// package has: a hand-written or shipped `channel.d.ts` beside a
// `channel.js`. Resolution selects the declaration file, never opens the
// implementation, and records nothing joining them — so the two are unrelated
// modules that happen to share a name on disk, and no field here will ever say
// otherwise. Recovering that edge by matching file names is the substitution
// this fact exists to avoid. See
// docs/adr/0018-v1-attested-resolved-module-graph.md.
//
// Both fields are always populated; which one equals the module's own path
// says whether the program holds the input or the output.
type ProjectReferenceMapping struct {
	Source    string `cbor:"source" json:"source"`
	OutputDts string `cbor:"outputDts" json:"outputDts"`
}

// ModuleFact is one file the TypeScript program actually resolved and
// included. The inventory of these facts is the program's own file list, so a
// consumer that must record which bytes an analysis read has an attestation
// rather than a reconstruction of it.
type ModuleFact struct {
	// Path is the cleaned absolute path the program holds the file under.
	// For a module reached through a symlink this is the realpath, matching
	// ModuleImportFact.ResolvedPath.
	Path string `cbor:"path" json:"path"`
	// DeclarationFile is the compiler's own IsDeclarationFile bit.
	DeclarationFile bool `cbor:"declarationFile,omitempty" json:"declarationFile,omitempty"`
	// Format is the emit module format; see ModuleFormat.
	Format ModuleFormat `cbor:"format,omitempty" json:"format,omitempty"`
	// ProjectReference is the compiler's input-to-declaration-output pairing
	// for this file, and nil whenever no configured project reference covers
	// it — which is almost always. See ProjectReferenceMapping.
	ProjectReference *ProjectReferenceMapping `cbor:"projectReference,omitempty" json:"projectReference,omitempty"`
	// RedirectTargets are the other paths the program resolved to this same
	// file because they are the same name@version installed in more than one
	// place (Program.GetRedirectTargets). It is the compiler's own duplicate-
	// install record, never a path similarity.
	RedirectTargets []string `cbor:"redirectTargets,omitempty" json:"redirectTargets,omitempty"`
}

// ModuleResolution names what the compiler's resolver recorded about the shape
// of one resolution. Every value is read off module.ResolvedModule; none is
// inferred from a path.
type ModuleResolution string

const (
	// ModuleResolutionUnresolved means the program holds no resolution for
	// this specifier. It is the only value with an empty ResolvedPath.
	ModuleResolutionUnresolved ModuleResolution = "unresolved"
	// ModuleResolutionRelative is a specifier the resolver treated as
	// relative or rooted, so no package lookup participated.
	ModuleResolutionRelative ModuleResolution = "relative"
	// ModuleResolutionNodeModules is IsExternalLibraryImport: the resolver
	// landed inside a node_modules tree.
	ModuleResolutionNodeModules ModuleResolution = "nodeModules"
	// ModuleResolutionNonRelative is a bare specifier that resolved outside
	// every node_modules tree. A tsconfig `paths` or `baseUrl` mapping, a
	// package self-name, a project-reference redirect, and an ambient module
	// declaration all land here, and ResolvedModule does not record which, so
	// this value never claims one. ModuleImportFact.PathsPattern answers the
	// `paths` half on its own terms.
	ModuleResolutionNonRelative ModuleResolution = "nonRelative"
)

// PackageIdentity is the owning package of a resolved file: the nearest
// enclosing package.json found by the compiler's own package-scope lookup, and
// that manifest's own name and version.
//
// Name and Version are empty when the manifest declares none, which is a fact
// about the manifest and not a lookup failure — ManifestPath is populated in
// that case too. The package directory is the manifest path's directory and is
// not repeated here.
type PackageIdentity struct {
	ManifestPath string `cbor:"manifestPath" json:"manifestPath"`
	Name         string `cbor:"name,omitempty" json:"name,omitempty"`
	Version      string `cbor:"version,omitempty" json:"version,omitempty"`
}

// ResolverPackageID is the package identity the *resolver* recorded while
// resolving one specifier (module.ResolvedModule.PackageId). It is a different
// fact from PackageIdentity and the two can disagree: this one names the
// package whose manifest the resolution consulted, which for a subpath export
// or a nested workspace install is not always the nearest manifest above the
// file that was selected. A consumer comparing a contract against a package
// must say which of the two it means.
type ResolverPackageID struct {
	Name string `cbor:"name,omitempty" json:"name,omitempty"`
	// Subpath is PackageId.SubModuleName: the path of the selected file
	// relative to the package directory, as the resolver recorded it — the
	// file it landed on, not the `exports` key that led there. It is empty
	// when the package root's own entry was selected.
	Subpath          string `cbor:"subpath,omitempty" json:"subpath,omitempty"`
	Version          string `cbor:"version,omitempty" json:"version,omitempty"`
	PeerDependencies string `cbor:"peerDependencies,omitempty" json:"peerDependencies,omitempty"`
}

// ModuleImportFact is the compiler's own answer for one module specifier: the
// file the program included for it, and what the resolver recorded on the way.
//
// One fact is produced per specifier occurrence in SourceFile.Imports — import
// declarations, export-from declarations, `import(...)` types, and require
// calls alike — so a consumer joins these rows to its own syntax facts by
// exact span rather than by matching specifier text.
type ModuleImportFact struct {
	// Specifier is the string-literal span, with Path naming the importing
	// file.
	Specifier Location `cbor:"specifier" json:"specifier"`
	// Text is the specifier as written, after string-literal unescaping.
	Text string `cbor:"text" json:"text"`
	// Resolution names what the resolver recorded; see ModuleResolution.
	Resolution ModuleResolution `cbor:"resolution" json:"resolution"`
	// ResolvedPath is ResolvedModule.ResolvedFileName, cleaned: the file the
	// resolver selected. When resolution walked a symlink it is the realpath.
	ResolvedPath string `cbor:"resolvedPath,omitempty" json:"resolvedPath,omitempty"`
	// IncludedPath is the file the program actually parses in place of
	// ResolvedPath (Program.GetParseFileRedirect), populated only when the two
	// differ. It is the compiler's own redirect record and the only mechanism
	// by which a specifier that resolved to a declaration file is joined to an
	// implementation: a configured project reference's declaration output is
	// redirected to the input it was emitted from, and a symlinked equivalent
	// of the same. Nothing redirects an ordinary shipped `.d.ts` to the `.js`
	// beside it, so an empty IncludedPath is the usual and honest answer.
	IncludedPath string `cbor:"includedPath,omitempty" json:"includedPath,omitempty"`
	// SymlinkPath is ResolvedModule.OriginalPath, cleaned: the path the
	// resolver had reached before taking its realpath. TypeScript populates it
	// only when the two differ and only for a non-relative resolution under
	// node_modules with preserveSymlinks off — which is exactly the pnpm and
	// workspace-link shape — so an empty SymlinkPath means the resolver saw no
	// divergence, not that none was looked for.
	SymlinkPath string `cbor:"symlinkPath,omitempty" json:"symlinkPath,omitempty"`
	// Extension is ResolvedModule.Extension, the extension the resolver
	// selected (".ts", ".d.ts", ".js", ".json", …). It is how a consumer sees
	// that a specifier landed on a declaration file.
	Extension string `cbor:"extension,omitempty" json:"extension,omitempty"`
	// TSExtension is ResolvedModule.ResolvedUsingTsExtension: the specifier
	// named a TypeScript extension outright rather than having one substituted.
	TSExtension bool `cbor:"tsExtension,omitempty" json:"tsExtension,omitempty"`
	// PathsPattern is the configured `paths` key the compiler's own pattern
	// matcher selects for Text, under the compiler's own eligibility rule
	// (`paths` is non-empty and the specifier is not relative) and its own
	// longest-prefix tie-break.
	//
	// It says the mapping *matched the specifier*, which is a fact about the
	// configuration and the text. It does not say the resolution came through
	// the mapping: TypeScript tries `paths` first and falls through to
	// ordinary resolution when the mapped candidate does not exist, and
	// ResolvedModule records no trace of which happened. Read together with
	// Resolution it is nonetheless decisive for the case it exists to serve —
	// a bare specifier that a `paths` key matches and that did *not* land in
	// node_modules is not the installed package of that name.
	PathsPattern string `cbor:"pathsPattern,omitempty" json:"pathsPattern,omitempty"`
	// Package is the owning package of ResolvedPath. It is populated only when
	// the request asked for package identities.
	Package *PackageIdentity `cbor:"package,omitempty" json:"package,omitempty"`
	// ResolverPackage is the identity the resolver itself recorded. It is
	// populated only when the request asked for package identities and only
	// when the resolver read a manifest during this resolution.
	ResolverPackage *ResolverPackageID `cbor:"resolverPackage,omitempty" json:"resolverPackage,omitempty"`
}

// ModuleInventoryDemand selects how much of the resolved module graph one
// answer carries. The module inventory itself is unconditional: it is the
// operation's reason to exist.
type ModuleInventoryDemand struct {
	// Imports adds resolved import provenance. With no ImportPaths it covers
	// every file in the program.
	Imports bool `cbor:"imports,omitempty" json:"imports,omitempty"`
	// ImportPaths scopes Imports to these importing files.
	ImportPaths []string `cbor:"importPaths,omitempty" json:"importPaths,omitempty"`
	// Packages adds Package and ResolverPackage to every import fact.
	Packages bool `cbor:"packages,omitempty" json:"packages,omitempty"`
}

// ModuleInventory is one generation's resolved module graph.
type ModuleInventory struct {
	// Modules is every file the program included, ordered by path.
	Modules []ModuleFact `cbor:"modules,omitempty" json:"modules,omitempty"`
	// Imports are the requested files' specifier facts, ordered by importing
	// path and then by specifier start byte.
	Imports []ModuleImportFact `cbor:"imports,omitempty" json:"imports,omitempty"`
	// UnknownImportPaths are requested import paths the program does not hold,
	// ordered by path. They are reported rather than dropped so a consumer can
	// tell "this file imports nothing" from "this file was never analyzed".
	UnknownImportPaths []string `cbor:"unknownImportPaths,omitempty" json:"unknownImportPaths,omitempty"`
}

// ModuleGraphProvider answers for the resolved module graph of one configured
// project. It is a compiler-resolution capability with no compiler-independent
// approximation: a backend that cannot answer must say so rather than return a
// partial graph, because an incomplete inventory presented as complete is the
// exact defect an attested closure exists to remove.
type ModuleGraphProvider interface {
	ModuleGraph(context.Context, ModuleInventoryDemand) (ModuleInventory, error)
}

// ArgumentMappingStatus says whether TypeScript exposes one exact formal
// parameter for a supplied argument.
type ArgumentMappingStatus string

const (
	ArgumentMappingResolved   ArgumentMappingStatus = "resolved"
	ArgumentMappingUnresolved ArgumentMappingStatus = "unresolved"
)

// ArgumentMappingReason explains why a supplied argument has no exact formal
// parameter mapping.
type ArgumentMappingReason string

const (
	ArgumentMappingCallUnresolved       ArgumentMappingReason = "callUnresolved"
	ArgumentMappingRecoverySignature    ArgumentMappingReason = "recoverySignature"
	ArgumentMappingCompositeSignature   ArgumentMappingReason = "compositeSignature"
	ArgumentMappingSpreadArgument       ArgumentMappingReason = "spreadArgument"
	ArgumentMappingParameterUnavailable ArgumentMappingReason = "parameterUnavailable"
)

// ParameterFact describes the selected signature's formal parameter after
// generic substitution at one argument position.
type ParameterFact struct {
	Index          int
	Symbol         SymbolID
	Declaration    *Declaration
	Rest           bool
	Optional       bool
	Callability    Callability
	TypeDescriptor *TypeDescriptor
	ObjectShape    *ObjectConstructionShape
}

// ConstructionWitness is a bounded, side-effect-free JavaScript candidate
// derived from a selected declaration parameter and its compiler constraints.
// It becomes a proven inhabitant only when a completed synthetic call resolves
// validly. Unknown means the producer found no candidate; it is never
// permission to guess one.
type ConstructionWitness string

const (
	ConstructionWitnessUnknown     ConstructionWitness = "unknown"
	ConstructionWitnessEmptyArray  ConstructionWitness = "emptyArray"
	ConstructionWitnessEmptyObject ConstructionWitness = "emptyObject"
)

// ObjectConstructionProperty is one required property of a selected call
// parameter. Name comes from the compiler symbol table, not rendered type text.
type ObjectConstructionProperty struct {
	Name    string
	Witness ConstructionWitness
}

// ObjectConstructionShape is the finite set of required properties common to
// every inhabited constituent of a selected call parameter's object type.
type ObjectConstructionShape struct {
	RequiredProperties []ObjectConstructionProperty
}

// ArgumentMapping relates one supplied argument to its exact formal parameter,
// or carries an explicit reason that no exact mapping exists.
type ArgumentMapping struct {
	ArgumentIndex int
	Status        ArgumentMappingStatus
	Unresolved    ArgumentMappingReason
	Parameter     *ParameterFact
}

// CallTargetSet is a finite set of exact callable declarations for one call.
// Exhaustive is an explicit compiler proof that Candidates cover every call
// signature of the callee's apparent type: every union constituent was a
// closed concrete callable and every one of its signatures named one exact
// implementation declaration. A set without that proof bit must never be
// treated as the complete runtime dispatch set. Candidates are deduplicated
// and ordered deterministically by declaration location, then symbol.
type CallTargetSet struct {
	Exhaustive bool                  `cbor:"exhaustive,omitempty" json:"exhaustive,omitempty"`
	Candidates []ResolvedDeclaration `cbor:"candidates,omitempty" json:"candidates,omitempty"`
}

// Call describes the target and instantiated return type of a demanded call.
// The return type is carried as text only: the opaque per-generation identity
// that used to accompany it had no consumer, and because it embedded the
// generation number it made every entity row holding a resolved call compare as
// changed on every generation, inflating each delta.
type Call struct {
	Target         SymbolID
	ReturnTypeText string
	Validity       ResolvedCallValidity
	Kind           CallKind
	Declaration    *ResolvedDeclaration
	// Targets carries the exact candidate declarations of a composite
	// (union) callee when the compiler proves the set exhaustive. It is
	// complementary to Declaration, which stays nil for composite callees
	// because no single signature was selected.
	Targets   *CallTargetSet
	Arguments []ArgumentMapping
}

// FileChange is one monotonically-versioned editor overlay change.
type FileChange struct {
	Path    string
	Version uint64
	Source  []byte
	Deleted bool
}

// AffectedSet lists normalized source paths invalidated by an update.
type AffectedSet struct {
	Files []string
}

// SourceFile is an original project source and its normalized path. This bulk
// view lets compiler adapters analyze project inputs without exposing TS ASTs.
type SourceFile struct {
	Path   string
	Source []byte
}

// Project provides type facts for one configured TypeScript project.
type Project interface {
	SourceFiles(context.Context) ([]SourceFile, error)
	Update(context.Context, []FileChange) (AffectedSet, error)
	SymbolAt(context.Context, Location) (SymbolID, error)
	ResolveAlias(context.Context, SymbolID) (SymbolID, error)
	Declarations(context.Context, SymbolID) ([]Declaration, error)
	References(context.Context, SymbolID) ([]Location, error)
	Close() error
}
