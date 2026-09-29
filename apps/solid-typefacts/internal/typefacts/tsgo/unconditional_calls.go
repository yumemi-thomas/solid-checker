package tsgo

import (
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/yumemi-thomas/solid-checker/apps/solid-typefacts/internal/typefacts"
)

// callRunsOnEveryCompletion answers ImplementationCall.Unconditional (ADR 0152,
// handshake protocol 71): the call runs exactly once on every normal
// completion of its flow owner, the innermost callable containing it or the
// implementation itself.
//
// The walk goes up from the call to the owner's body and admits each step only
// when it is a position its parent evaluates exactly once, unconditionally,
// every time the parent itself is evaluated. It is a whitelist on purpose: a
// syntactic form this list does not name -- a future one included -- makes
// the answer false, which claims nothing. Every block on the way also has its
// earlier statements checked for a `return`, `break` or `continue` that could
// leave the owner before the call. The optimistic `Reach` the body walk has
// always stated keeps an `if` arm reachable; this fact is the lower bound that
// reading does not give.
//
// ADR 0166: a condition literalTruthinessLocked decides -- a literal, a
// never-written const, or a host constant -- leaves one arm of an `if` or `?:`
// dead. The live arm is then evaluated whenever its parent is, and an early
// exit written only in the dead arm is no path out.
func (p *project) callRunsOnEveryCompletionLocked(call *ast.Node, owner *ast.Node, reach typefacts.Reachability) bool {
	if call == nil || owner == nil || reach != typefacts.Reachable {
		return false
	}
	if implementationCompletionForm(owner) != typefacts.CompletionPlain {
		return false
	}
	if ast.IsCallExpression(call) && chainHasOptional(call) {
		return false
	}
	body := owner.Body()
	if body == nil {
		return false
	}
	child := call
	for parent := call.Parent; parent != nil; child, parent = parent, parent.Parent {
		if parent == owner {
			return child == body
		}
		if isCallableDeclaration(parent) {
			return false
		}
		if !p.evaluatesChildUnconditionallyLocked(parent, child) {
			return false
		}
	}
	return false
}

// evaluatesChildUnconditionally is the whitelist: whether evaluating `parent`
// evaluates `child` exactly once, whatever the values involved.
func (p *project) evaluatesChildUnconditionallyLocked(parent, child *ast.Node) bool {
	switch {
	case ast.IsBlock(parent):
		for _, statement := range parent.AsBlock().Statements.Nodes {
			if statement == child {
				return true
			}
			if p.mayExitEarlyLocked(statement) {
				return false
			}
		}
		return false
	case ast.IsIfStatement(parent):
		statement := parent.AsIfStatement()
		if statement.Expression == child {
			return true
		}
		truthy, known := p.literalTruthinessLocked(statement.Expression, 0)
		return known && ((truthy && statement.ThenStatement == child) ||
			(!truthy && statement.ElseStatement == child))
	case ast.IsConditionalExpression(parent):
		conditional := parent.AsConditionalExpression()
		if conditional.Condition == child {
			return true
		}
		truthy, known := p.literalTruthinessLocked(conditional.Condition, 0)
		return known && ((truthy && conditional.WhenTrue == child) ||
			(!truthy && conditional.WhenFalse == child))
	case ast.IsBinaryExpression(parent):
		binary := parent.AsBinaryExpression()
		operator := binary.OperatorToken.Kind
		if operator == ast.KindAmpersandAmpersandToken || operator == ast.KindBarBarToken ||
			operator == ast.KindQuestionQuestionToken || ast.IsLogicalOrCoalescingAssignmentOperator(operator) {
			return binary.Left == child
		}
		return true
	case ast.IsCallExpression(parent) || ast.IsNewExpression(parent):
		return !chainHasOptional(parent)
	case ast.IsPropertyAccessExpression(parent) || nodeKindName(parent) == "ElementAccessExpression":
		return !chainHasOptional(parent)
	case ast.IsReturnStatement(parent), ast.IsParenthesizedExpression(parent), ast.IsNonNullExpression(parent):
		return true
	}
	switch nodeKindName(parent) {
	case "ExpressionStatement", "ThrowStatement", "VariableStatement", "VariableDeclarationList",
		"AsExpression", "SatisfiesExpression", "TypeAssertionExpression",
		"PrefixUnaryExpression", "PostfixUnaryExpression", "TypeOfExpression", "VoidExpression",
		"TemplateExpression", "TemplateSpan", "ArrayLiteralExpression", "ObjectLiteralExpression",
		"PropertyAssignment", "SpreadElement", "SpreadAssignment", "ComputedPropertyName":
		return true
	case "VariableDeclaration":
		return parent.Initializer() == child
	}
	return false
}

// chainHasOptional reports whether a call or member access, or any link of the
// chain it continues, is an optional one: `a?.b.c(x)` evaluates `x` only when
// `a` is not nullish.
func chainHasOptional(node *ast.Node) bool {
	for node != nil {
		switch {
		case ast.IsCallExpression(node), ast.IsPropertyAccessExpression(node),
			nodeKindName(node) == "ElementAccessExpression":
			if node.QuestionDotToken() != nil {
				return true
			}
			node = node.Expression()
		case ast.IsNewExpression(node), ast.IsParenthesizedExpression(node), ast.IsNonNullExpression(node):
			node = node.Expression()
		default:
			return false
		}
	}
	return false
}
