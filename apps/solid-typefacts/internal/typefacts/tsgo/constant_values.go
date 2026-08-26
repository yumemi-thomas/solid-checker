package tsgo

import (
	"errors"
	"math"
	"math/big"
	"path/filepath"
	"strconv"
	"strings"

	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
	"github.com/microsoft/typescript-go/shim/scanner"
	"github.com/yumemi-thomas/solid-ts-facts/internal/typefacts"
)

const constantValueMaxDepth = 64

type constantValueEvaluator struct {
	project  *project
	evidence *semanticEvidence
	seen     map[*ast.Node]struct{}
}

func (p *project) constantValueAtLocked(node *ast.Node, evidence *semanticEvidence) *typefacts.ConstantValue {
	evaluator := constantValueEvaluator{
		project:  p,
		evidence: evidence,
		seen:     make(map[*ast.Node]struct{}),
	}
	value, ok := evaluator.evaluate(node, 0)
	if !ok {
		return nil
	}
	return &value
}

func (e *constantValueEvaluator) evaluate(node *ast.Node, depth int) (typefacts.ConstantValue, bool) {
	if node == nil || depth >= constantValueMaxDepth || !e.hasReliableType(node) {
		return typefacts.ConstantValue{}, false
	}
	if _, cycling := e.seen[node]; cycling {
		return typefacts.ConstantValue{}, false
	}
	e.seen[node] = struct{}{}
	defer delete(e.seen, node)

	switch {
	case ast.IsStringLiteral(node):
		return typefacts.ConstantValue{Kind: typefacts.ConstantValueString, String: node.AsStringLiteral().Text}, true
	case ast.IsNoSubstitutionTemplateLiteral(node):
		return typefacts.ConstantValue{Kind: typefacts.ConstantValueString, String: node.AsNoSubstitutionTemplateLiteral().Text}, true
	case ast.IsNumericLiteral(node):
		value, ok := parseNumericLiteral(node.AsNumericLiteral().Text)
		if !ok {
			return typefacts.ConstantValue{}, false
		}
		return typefacts.ConstantValue{Kind: typefacts.ConstantValueNumber, Number: value}, true
	case ast.IsParenthesizedExpression(node):
		return e.evaluate(node.AsParenthesizedExpression().Expression, depth+1)
	case ast.IsAsExpression(node):
		return e.evaluate(node.AsAsExpression().Expression, depth+1)
	case ast.IsSatisfiesExpression(node):
		return e.evaluate(node.AsSatisfiesExpression().Expression, depth+1)
	case ast.IsNonNullExpression(node):
		return e.evaluate(node.AsNonNullExpression().Expression, depth+1)
	case ast.IsPrefixUnaryExpression(node):
		unary := node.AsPrefixUnaryExpression()
		if unary.Operator != ast.KindPlusToken && unary.Operator != ast.KindMinusToken {
			return typefacts.ConstantValue{}, false
		}
		value, ok := e.evaluate(unary.Operand, depth+1)
		if !ok || value.Kind != typefacts.ConstantValueNumber {
			return typefacts.ConstantValue{}, false
		}
		if unary.Operator == ast.KindMinusToken {
			value.Number = -value.Number
		}
		return value, true
	case ast.IsBinaryExpression(node):
		binary := node.AsBinaryExpression()
		if binary.OperatorToken == nil || binary.OperatorToken.Kind != ast.KindPlusToken {
			return typefacts.ConstantValue{}, false
		}
		left, leftOK := e.evaluate(binary.Left, depth+1)
		right, rightOK := e.evaluate(binary.Right, depth+1)
		if !leftOK || !rightOK || left.Kind != right.Kind {
			return typefacts.ConstantValue{}, false
		}
		switch left.Kind {
		case typefacts.ConstantValueString:
			left.String += right.String
			return left, true
		case typefacts.ConstantValueNumber:
			left.Number += right.Number
			if math.IsNaN(left.Number) {
				return typefacts.ConstantValue{}, false
			}
			return left, true
		default:
			return typefacts.ConstantValue{}, false
		}
	case ast.IsIdentifier(node), ast.IsPropertyAccessExpression(node):
		return e.evaluateResolvedDeclaration(node, depth+1)
	default:
		// Template expressions always contain substitutions and deliberately do
		// not participate, even when those substitutions happen to be constant.
		return typefacts.ConstantValue{}, false
	}
}

func (e *constantValueEvaluator) hasReliableType(node *ast.Node) bool {
	value := e.project.checker.GetTypeAtLocation(node)
	return value != nil && value.Flags()&(checker.TypeFlagsAny|checker.TypeFlagsUnknown|checker.TypeFlagsIncludesError) == 0
}

func (e *constantValueEvaluator) evaluateResolvedDeclaration(node *ast.Node, depth int) (typefacts.ConstantValue, bool) {
	symbol := e.project.checker.GetSymbolAtLocation(node)
	if symbol == nil {
		return typefacts.ConstantValue{}, false
	}
	symbol = e.project.canonicalSymbol(symbol)
	if symbol == nil || symbol.ValueDeclaration == nil {
		return typefacts.ConstantValue{}, false
	}
	declaration := symbol.ValueDeclaration
	var initializer *ast.Node
	switch {
	case ast.IsVariableDeclaration(declaration):
		if !ast.IsVarConst(declaration) {
			return typefacts.ConstantValue{}, false
		}
		initializer = declaration.AsVariableDeclaration().Initializer
	case ast.IsPropertyDeclaration(declaration):
		if !ast.HasSyntacticModifier(declaration, ast.ModifierFlagsReadonly) {
			return typefacts.ConstantValue{}, false
		}
		initializer = declaration.AsPropertyDeclaration().Initializer
	case ast.IsEnumMember(declaration):
		initializer = declaration.AsEnumMember().Initializer
	default:
		return typefacts.ConstantValue{}, false
	}
	if initializer == nil {
		return typefacts.ConstantValue{}, false
	}
	if sourceFile := ast.GetSourceFileOfNode(declaration); sourceFile != nil {
		e.evidence.dependency(typefacts.Location{
			Path:      filepath.Clean(sourceFile.FileName()),
			StartByte: scanner.SkipTrivia(sourceFile.Text(), declaration.Pos()),
			EndByte:   declaration.End(),
		})
	}
	return e.evaluate(initializer, depth)
}

func parseNumericLiteral(text string) (float64, bool) {
	text = strings.ReplaceAll(text, "_", "")
	if strings.HasPrefix(text, "0x") || strings.HasPrefix(text, "0X") ||
		strings.HasPrefix(text, "0b") || strings.HasPrefix(text, "0B") ||
		strings.HasPrefix(text, "0o") || strings.HasPrefix(text, "0O") {
		base := 8
		if text[1] == 'x' || text[1] == 'X' {
			base = 16
		} else if text[1] == 'b' || text[1] == 'B' {
			base = 2
		}
		integer, ok := new(big.Int).SetString(text[2:], base)
		if !ok {
			return 0, false
		}
		value, _ := new(big.Float).SetInt(integer).Float64()
		return value, true
	}
	value, err := strconv.ParseFloat(text, 64)
	return value, (err == nil || errors.Is(err, strconv.ErrRange)) && !math.IsNaN(value)
}
