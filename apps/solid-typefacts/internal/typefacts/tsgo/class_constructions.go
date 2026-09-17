package tsgo

import (
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"

	"github.com/yumemi-thomas/solid-checker/apps/solid-typefacts/internal/typefacts"
)

// classConstructionRefusal names why a class at a demanded location is one this
// producer will not census. The vocabulary is open on the wire — a consumer
// refuses on any nonempty openReasons list — so a reason it has never seen
// still fails closed, which is why these may be added without the consumer
// learning them.
const (
	classRefusalHeritageClause      = "classHeritageClause"
	classRefusalFieldInitializer    = "classFieldInitializer"
	classRefusalStaticBlock         = "classStaticBlock"
	classRefusalComputedMemberName  = "classComputedMemberName"
	classRefusalDecorated           = "classDecorated"
	classRefusalParameterProperty   = "classParameterProperty"
	classRefusalImplicitConstructor = "classImplicitConstructor"
)

// classConstructorAt answers the constructor whose body `new C(…)` runs for a
// class node at an exact demanded location, or the reason this producer will
// not answer for it.
//
// **What a construction actually runs**, and why each part is either covered or
// refused rather than ignored. `new C(…)` evaluates, in order: the heritage
// clause's constructor, every field initializer in declaration order, and the
// constructor body. A census of the constructor body alone would be silent
// about the other two, and silence is the failure this whole census exists to
// prevent — so anything this function cannot see, it refuses on.
//
//   - **A heritage clause** refuses. `class R extends WeakMap` runs `WeakMap`'s
//     constructor, `class J extends WithPromise` runs this artifact's, and
//     `class X extends someExpression()` runs whatever that returned. Three
//     different claims, none of them made here. ADR 0047 drew its `own-class`
//     line at exactly this clause, for the neighbouring reason — what the
//     prototype chain can carry — and this follows it rather than inventing a
//     second line.
//   - **A field initializer** refuses, static or instance. `#keyTriggers = new
//     TriggerCache(WeakMap)` runs at construction as surely as the constructor
//     body does, and it is a *different node* the demand does not name. A bare
//     `#timeoutId;` declares storage and initializes to `undefined`, running
//     nothing, so it is admitted.
//   - **A static block** refuses: it runs at class-definition time, which is
//     module evaluation, not construction — a different question this does not
//     answer.
//   - **A computed member name** refuses. The key expression runs when the class
//     is defined. ADR 0047 refuses it already.
//   - **A decorator** refuses, on the class or any member: a decorator is a call
//     of user code at definition time.
//   - **A parameter property** (`constructor(private readonly x)`) refuses. It
//     assigns a field, and the assignment has no node of its own in the
//     constructor body, so a body census would not see it.
//   - **An implicit constructor** refuses, and this one is a deliberate
//     over-refusal. With no heritage clause an implicit constructor runs
//     nothing at all, which would be the *strongest* possible answer — but
//     "nothing runs" is a positive claim, and there is no node here to census
//     into a transcript that says it. Stating it is a separate decision;
//     refusing is the direction that cannot be wrong.
func classConstructorAt(class *ast.Node) (*ast.Node, string) {
	if class == nil {
		return nil, classRefusalImplicitConstructor
	}
	if decorated(class) {
		return nil, classRefusalDecorated
	}
	heritage, members := classHeritageAndMembers(class)
	if heritage {
		return nil, classRefusalHeritageClause
	}
	var constructor *ast.Node
	for _, member := range members {
		if member == nil {
			continue
		}
		if decorated(member) {
			return nil, classRefusalDecorated
		}
		switch nodeKindName(member) {
		case "ClassStaticBlockDeclaration":
			return nil, classRefusalStaticBlock
		case "PropertyDeclaration":
			if member.Initializer() != nil {
				return nil, classRefusalFieldInitializer
			}
		case "Constructor":
			// An overload signature has no body and is not the implementation;
			// the caller's own Body() check refuses a bodyless match, so the
			// last Constructor member wins here only when it carries one.
			if member.Body() != nil {
				constructor = member
			}
		}
		if name := member.Name(); name != nil && nodeKindName(name) == "ComputedPropertyName" {
			return nil, classRefusalComputedMemberName
		}
	}
	if constructor == nil {
		return nil, classRefusalImplicitConstructor
	}
	if parameters := constructor.Parameters(); parameters != nil {
		for _, parameter := range parameters {
			if parameter == nil {
				continue
			}
			if modifiers := parameter.Modifiers(); modifiers != nil && len(modifiers.Nodes) != 0 {
				// `constructor(private readonly x)` and any other modifier on a
				// parameter: the only ones TypeScript allows there are the
				// accessibility and `readonly` ones that declare a field, plus
				// a decorator. Every one of them is refused.
				return nil, classRefusalParameterProperty
			}
		}
	}
	return constructor, ""
}

// decorated reports whether a node carries a decorator among its modifiers. A
// decorator calls user code when the class is *defined*, so a construction
// census that ignored one would be describing a class that never existed.
func decorated(node *ast.Node) bool {
	modifiers := node.Modifiers()
	if modifiers == nil {
		return false
	}
	for _, modifier := range modifiers.Nodes {
		if modifier != nil && nodeKindName(modifier) == "Decorator" {
			return true
		}
	}
	return false
}

// classHeritageAndMembers reads the two halves this gate needs from either
// spelling of a class. `class X {}` and `var X = class {}` are the same
// construction — the second is what every bundler emits for the first — and a
// census that answered for one and not the other would be an accident of
// compilation rather than a statement about the code.
func classHeritageAndMembers(class *ast.Node) (bool, []*ast.Node) {
	// `As…` is an unchecked assertion on the node's data, so the kind has to be
	// tested first; asking a ClassExpression for its ClassDeclaration panics.
	if ast.IsClassDeclaration(class) {
		declaration := class.AsClassDeclaration()
		if declaration == nil {
			return true, nil
		}
		heritage := declaration.HeritageClauses != nil && len(declaration.HeritageClauses.Nodes) != 0
		if declaration.Members == nil {
			return heritage, nil
		}
		return heritage, declaration.Members.Nodes
	}
	if ast.IsClassExpression(class) {
		expression := class.AsClassExpression()
		if expression == nil {
			return true, nil
		}
		heritage := expression.HeritageClauses != nil && len(expression.HeritageClauses.Nodes) != 0
		if expression.Members == nil {
			return heritage, nil
		}
		return heritage, expression.Members.Nodes
	}
	// Not a class at all: the caller has no construction to census, and a
	// heritage clause is the refusing answer.
	return true, nil
}

// exactClassDeclarationAt answers the class node whose span is exactly the
// demanded location, or nil. The walk prunes on containment exactly as
// exactFunctionLikeDeclarationsAt's does.
func exactClassDeclarationAt(
	sourceFile *ast.SourceFile,
	location typefacts.Location,
) *ast.Node {
	var match *ast.Node
	var visit func(*ast.Node)
	visit = func(node *ast.Node) {
		if node == nil || match != nil {
			return
		}
		nodeAt := nodeLocation(node)
		if nodeAt.StartByte == location.StartByte && nodeAt.EndByte == location.EndByte &&
			nodeAt.Path == location.Path &&
			(ast.IsClassDeclaration(node) || ast.IsClassExpression(node)) {
			match = node
			return
		}
		node.ForEachChild(func(child *ast.Node) bool {
			childAt := nodeLocation(child)
			if childAt.StartByte <= location.StartByte && location.EndByte <= childAt.EndByte {
				visit(child)
			}
			return false
		})
	}
	visit(sourceFile.AsNode())
	return match
}

// classConstructSignaturesLocked answers the construct signatures of a value
// whose call side is empty, for ADR 0105's premise, or nil.
//
// It asks the type at the demanded identifier, which in a value position is
// the **constructor** rather than the instance — the distinction the ADR 0099
// comment in `export_value_transcripts.go` records the hard way, where the
// type at a class *declaration's own name* is the instance type and has no
// construct signature at all.
//
// It states nothing for a value that is also callable. Such a value is two
// claims, and selecting the construct side here would be choosing between them
// silently; `classConstructorAt` is what decides whether the construction is
// one this producer can census, and it runs on the declaration this selection
// leads to.
func (p *project) classConstructSignaturesLocked(valueType *checker.Type) []*checker.Signature {
	if valueType == nil {
		return nil
	}
	if len(p.checker.GetSignaturesOfType(valueType, checker.SignatureKindCall)) != 0 {
		return nil
	}
	// The type at a class declaration's **own name** is the *instance* type,
	// which has no construct signature -- the trap ADR 0099's comment in
	// `export_value_transcripts.go` records, found there by the `Box` fixture.
	// Asking the class node itself answers the same instance type, so a demand
	// that lands on the declaring name states nothing and refuses. A demand at
	// any value position -- the binding of `var C = class {…}`, or an
	// `export { C }` specifier -- reaches the constructor and answers.
	return p.checker.GetSignaturesOfType(valueType, checker.SignatureKindConstruct)
}
