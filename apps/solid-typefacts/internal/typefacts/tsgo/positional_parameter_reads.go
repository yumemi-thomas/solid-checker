package tsgo

import (
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
)

// ADR 0069. The opening-prefix proof (ADR 0065) asked which statements are safe
// to walk past. That was never the question. `initialParameterReadsLocked`
// already refuses every implementation that contains a nested callable,
// `arguments` or `eval`, and a parameter with a defaulted or destructured
// declaration. In what is left, nothing outside the body can reach these
// lexical bindings, so the only writer is a direct assignment in this body and
// the question is purely one of order: can any such assignment's store run
// before this read?
//
// It cannot when the read ends at or before the store, and no iteration
// statement encloses both — a loop is the only construct that runs a later
// position before an earlier one, because a call cannot re-enter this
// activation's bindings and forward jumps (`break`, `continue`, `switch`,
// `throw`) never reach backwards.
//
// This is deliberately not a reachability claim. A read inside a branch keeps
// the caller's value *if it runs at all*; `initialParameterReadsLocked` marks
// exactly those rows conditional, and only a zero-lower-bound consumer may
// take one.
//
// positionalOriginalInputCutoffsLocked answers, for each parameter this body
// actually assigns, the last source position at which a read of that binding
// still observes the caller's value. A parameter absent from the result is
// either never assigned here — `unwrittenParameterBindingsLocked` owns that,
// with a stronger premise — or not trackable, and a nil result refuses the
// whole body.
func (p *project) positionalOriginalInputCutoffsLocked(
	body *ast.Node,
	symbols map[int]*ast.Symbol,
	fileChecker *checker.Checker,
) map[int]int {
	tracked := make(map[*ast.Symbol]int, len(symbols))
	for index, symbol := range symbols {
		// An unresolved or shared parameter symbol cannot separate this
		// binding's writes from another's. Refuse the body rather than track
		// the rest of it against an incomplete write set.
		if symbol == nil {
			return nil
		}
		if _, duplicate := tracked[symbol]; duplicate {
			return nil
		}
		tracked[symbol] = index
	}
	cutoffs := make(map[int]int)
	var visit func(*ast.Node)
	visit = func(node *ast.Node) {
		if node == nil {
			return
		}
		// The declaration-name filter is asked after the assignment test for the
		// reason recorded on `isAssignmentTargetIdentifier`: a shorthand
		// destructuring-assignment target is a declaration name by the
		// compiler's reckoning and a write by the language's.
		if ast.IsIdentifier(node) && !ast.IsPartOfTypeNode(node) {
			if assignment := ast.GetAssignmentTarget(node); assignment != nil {
				if symbol := p.assignedBindingSymbol(fileChecker, node); symbol != nil {
					if index, isParameter := tracked[symbol]; isParameter {
						cutoff := positionalStoreCutoff(assignment, body)
						if current, seen := cutoffs[index]; !seen || cutoff < current {
							cutoffs[index] = cutoff
						}
					}
				}
			}
		}
		node.ForEachChild(func(child *ast.Node) bool { visit(child); return false })
	}
	visit(body)
	return cutoffs
}

// positionalStoreCutoff is the last position at which a read still precedes
// this one write.
func positionalStoreCutoff(assignment, body *ast.Node) int {
	// A loop can run this store before a read that is positioned after it, so
	// the only reads this write leaves alone are the ones that precede the
	// whole loop. The outermost enclosing iteration statement is the bound: an
	// inner loop's earlier iterations are already covered by the outer one.
	if loop := outermostIterationStatement(assignment, body); loop != nil {
		return nodeLocation(loop).StartByte
	}
	// A plain `=` evaluates its entire right-hand side before it stores, so a
	// read anywhere inside that expression still observes the caller's value.
	if ast.IsBinaryExpression(assignment) &&
		assignment.AsBinaryExpression().OperatorToken.Kind == ast.KindEqualsToken {
		return nodeLocation(assignment).EndByte
	}
	// A compound assignment, `++`/`--`, and a `for…in`/`for…of` head all read or
	// store within their own extent. Refuse from the write's first byte.
	return nodeLocation(assignment).StartByte
}

func outermostIterationStatement(node, body *ast.Node) *ast.Node {
	var found *ast.Node
	for current := node; current != nil && current != body; current = current.Parent {
		if ast.IsIterationStatement(current, false) {
			found = current
		}
	}
	return found
}
