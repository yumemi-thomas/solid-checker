// Package ast re-exports the slice of typescript-go's internal ast
// package that this repository uses — nothing more. Declarations are
// copied from oxc-project/tsgolint's generated shims (MIT); the module
// path claims the typescript-go prefix so the internal imports resolve.
// Regenerate by hand when a compiler bump moves an identifier: the
// compiler reports alias breaks, and the go:linkname signatures below
// must be re-verified against the target revision by eye.
package ast

import ast "github.com/microsoft/typescript-go/internal/ast"
import _ "unsafe"

type Diagnostic = ast.Diagnostic

//go:linkname GetNodeAtPosition github.com/microsoft/typescript-go/internal/ast.GetNodeAtPosition
func GetNodeAtPosition(file *ast.SourceFile, position int, includeJSDoc bool) *ast.Node

//go:linkname GetSourceFileOfNode github.com/microsoft/typescript-go/internal/ast.GetSourceFileOfNode
func GetSourceFileOfNode(node *ast.Node) *ast.SourceFile

//go:linkname GetAssignmentTarget github.com/microsoft/typescript-go/internal/ast.GetAssignmentTarget
func GetAssignmentTarget(node *ast.Node) *ast.Node

//go:linkname HasSyntacticModifier github.com/microsoft/typescript-go/internal/ast.HasSyntacticModifier
func HasSyntacticModifier(node *ast.Node, flags ast.ModifierFlags) bool

//go:linkname IsExpression github.com/microsoft/typescript-go/internal/ast.IsExpression
func IsExpression(node *ast.Node) bool

//go:linkname IsAsExpression github.com/microsoft/typescript-go/internal/ast.IsAsExpression
func IsAsExpression(node *ast.Node) bool

//go:linkname IsArrayBindingPattern github.com/microsoft/typescript-go/internal/ast.IsArrayBindingPattern
func IsArrayBindingPattern(node *ast.Node) bool

//go:linkname IsArrayLiteralExpression github.com/microsoft/typescript-go/internal/ast.IsArrayLiteralExpression
func IsArrayLiteralExpression(node *ast.Node) bool

//go:linkname IsObjectBindingPattern github.com/microsoft/typescript-go/internal/ast.IsObjectBindingPattern
func IsObjectBindingPattern(node *ast.Node) bool

//go:linkname IsArrowFunction github.com/microsoft/typescript-go/internal/ast.IsArrowFunction
func IsArrowFunction(node *ast.Node) bool

//go:linkname IsAwaitExpression github.com/microsoft/typescript-go/internal/ast.IsAwaitExpression
func IsAwaitExpression(node *ast.Node) bool

//go:linkname IsBinaryExpression github.com/microsoft/typescript-go/internal/ast.IsBinaryExpression
func IsBinaryExpression(node *ast.Node) bool

//go:linkname IsEnumMember github.com/microsoft/typescript-go/internal/ast.IsEnumMember
func IsEnumMember(node *ast.Node) bool

//go:linkname IsBindingElement github.com/microsoft/typescript-go/internal/ast.IsBindingElement
func IsBindingElement(node *ast.Node) bool

//go:linkname IsBlock github.com/microsoft/typescript-go/internal/ast.IsBlock
func IsBlock(node *ast.Node) bool

//go:linkname IsBreakStatement github.com/microsoft/typescript-go/internal/ast.IsBreakStatement
func IsBreakStatement(node *ast.Node) bool

//go:linkname IsCallExpression github.com/microsoft/typescript-go/internal/ast.IsCallExpression
func IsCallExpression(node *ast.Node) bool

//go:linkname IsClassDeclaration github.com/microsoft/typescript-go/internal/ast.IsClassDeclaration
func IsClassDeclaration(node *ast.Node) bool

//go:linkname IsClassExpression github.com/microsoft/typescript-go/internal/ast.IsClassExpression
func IsClassExpression(node *ast.Node) bool

//go:linkname IsConditionalExpression github.com/microsoft/typescript-go/internal/ast.IsConditionalExpression
func IsConditionalExpression(node *ast.Node) bool

//go:linkname IsEnumDeclaration github.com/microsoft/typescript-go/internal/ast.IsEnumDeclaration
func IsEnumDeclaration(node *ast.Node) bool

//go:linkname IsDeclarationNameOrImportPropertyName github.com/microsoft/typescript-go/internal/ast.IsDeclarationNameOrImportPropertyName
func IsDeclarationNameOrImportPropertyName(name *ast.Node) bool

//go:linkname IsExternalModule github.com/microsoft/typescript-go/internal/ast.IsExternalModule
func IsExternalModule(file *ast.SourceFile) bool

//go:linkname IsExternalModuleAugmentation github.com/microsoft/typescript-go/internal/ast.IsExternalModuleAugmentation
func IsExternalModuleAugmentation(node *ast.Node) bool

//go:linkname IsFunctionDeclaration github.com/microsoft/typescript-go/internal/ast.IsFunctionDeclaration
func IsFunctionDeclaration(node *ast.Node) bool

//go:linkname IsFunctionExpression github.com/microsoft/typescript-go/internal/ast.IsFunctionExpression
func IsFunctionExpression(node *ast.Node) bool

//go:linkname IsGlobalScopeAugmentation github.com/microsoft/typescript-go/internal/ast.IsGlobalScopeAugmentation
func IsGlobalScopeAugmentation(node *ast.Node) bool

//go:linkname IsIdentifier github.com/microsoft/typescript-go/internal/ast.IsIdentifier
func IsIdentifier(node *ast.Node) bool

//go:linkname IsNoSubstitutionTemplateLiteral github.com/microsoft/typescript-go/internal/ast.IsNoSubstitutionTemplateLiteral
func IsNoSubstitutionTemplateLiteral(node *ast.Node) bool

//go:linkname IsNonNullExpression github.com/microsoft/typescript-go/internal/ast.IsNonNullExpression
func IsNonNullExpression(node *ast.Node) bool

//go:linkname IsNumericLiteral github.com/microsoft/typescript-go/internal/ast.IsNumericLiteral
func IsNumericLiteral(node *ast.Node) bool

//go:linkname IsParenthesizedExpression github.com/microsoft/typescript-go/internal/ast.IsParenthesizedExpression
func IsParenthesizedExpression(node *ast.Node) bool

//go:linkname IsParameterDeclaration github.com/microsoft/typescript-go/internal/ast.IsParameterDeclaration
func IsParameterDeclaration(node *ast.Node) bool

//go:linkname IsPostfixUnaryExpression github.com/microsoft/typescript-go/internal/ast.IsPostfixUnaryExpression
func IsPostfixUnaryExpression(node *ast.Node) bool

//go:linkname IsPrefixUnaryExpression github.com/microsoft/typescript-go/internal/ast.IsPrefixUnaryExpression
func IsPrefixUnaryExpression(node *ast.Node) bool

//go:linkname IsPropertyAccessExpression github.com/microsoft/typescript-go/internal/ast.IsPropertyAccessExpression
func IsPropertyAccessExpression(node *ast.Node) bool

//go:linkname IsObjectLiteralExpression github.com/microsoft/typescript-go/internal/ast.IsObjectLiteralExpression
func IsObjectLiteralExpression(node *ast.Node) bool

//go:linkname IsPropertyDeclaration github.com/microsoft/typescript-go/internal/ast.IsPropertyDeclaration
func IsPropertyDeclaration(node *ast.Node) bool

//go:linkname IsSatisfiesExpression github.com/microsoft/typescript-go/internal/ast.IsSatisfiesExpression
func IsSatisfiesExpression(node *ast.Node) bool

//go:linkname IsStringLiteral github.com/microsoft/typescript-go/internal/ast.IsStringLiteral
func IsStringLiteral(node *ast.Node) bool

//go:linkname IsVarConst github.com/microsoft/typescript-go/internal/ast.IsVarConst
func IsVarConst(node *ast.Node) bool

//go:linkname IsIfStatement github.com/microsoft/typescript-go/internal/ast.IsIfStatement
func IsIfStatement(node *ast.Node) bool

//go:linkname IsImportClause github.com/microsoft/typescript-go/internal/ast.IsImportClause
func IsImportClause(node *ast.Node) bool

//go:linkname IsImportDeclaration github.com/microsoft/typescript-go/internal/ast.IsImportDeclaration
func IsImportDeclaration(node *ast.Node) bool

//go:linkname IsImportSpecifier github.com/microsoft/typescript-go/internal/ast.IsImportSpecifier
func IsImportSpecifier(node *ast.Node) bool

//go:linkname IsIterationStatement github.com/microsoft/typescript-go/internal/ast.IsIterationStatement
func IsIterationStatement(node *ast.Node, lookInLabeledStatements bool) bool

//go:linkname IsLogicalOrCoalescingAssignmentOperator github.com/microsoft/typescript-go/internal/ast.IsLogicalOrCoalescingAssignmentOperator
func IsLogicalOrCoalescingAssignmentOperator(kind ast.Kind) bool

//go:linkname IsMethodDeclaration github.com/microsoft/typescript-go/internal/ast.IsMethodDeclaration
func IsMethodDeclaration(node *ast.Node) bool

//go:linkname IsModuleDeclaration github.com/microsoft/typescript-go/internal/ast.IsModuleDeclaration
func IsModuleDeclaration(node *ast.Node) bool

//go:linkname IsNamespaceImport github.com/microsoft/typescript-go/internal/ast.IsNamespaceImport
func IsNamespaceImport(node *ast.Node) bool

//go:linkname IsNewExpression github.com/microsoft/typescript-go/internal/ast.IsNewExpression
func IsNewExpression(node *ast.Node) bool

//go:linkname IsPartOfTypeNode github.com/microsoft/typescript-go/internal/ast.IsPartOfTypeNode
func IsPartOfTypeNode(node *ast.Node) bool

//go:linkname IsPartOfTypeOnlyImportOrExportDeclaration github.com/microsoft/typescript-go/internal/ast.IsPartOfTypeOnlyImportOrExportDeclaration
func IsPartOfTypeOnlyImportOrExportDeclaration(node *ast.Node) bool

//go:linkname IsQualifiedName github.com/microsoft/typescript-go/internal/ast.IsQualifiedName
func IsQualifiedName(node *ast.Node) bool

//go:linkname IsReturnStatement github.com/microsoft/typescript-go/internal/ast.IsReturnStatement
func IsReturnStatement(node *ast.Node) bool

//go:linkname IsSpreadElement github.com/microsoft/typescript-go/internal/ast.IsSpreadElement
func IsSpreadElement(node *ast.Node) bool

//go:linkname IsSwitchStatement github.com/microsoft/typescript-go/internal/ast.IsSwitchStatement
func IsSwitchStatement(node *ast.Node) bool

//go:linkname IsThrowStatement github.com/microsoft/typescript-go/internal/ast.IsThrowStatement
func IsThrowStatement(node *ast.Node) bool

//go:linkname IsTryStatement github.com/microsoft/typescript-go/internal/ast.IsTryStatement
func IsTryStatement(node *ast.Node) bool

//go:linkname IsTypeReferenceNode github.com/microsoft/typescript-go/internal/ast.IsTypeReferenceNode
func IsTypeReferenceNode(node *ast.Node) bool

//go:linkname IsVariableDeclaration github.com/microsoft/typescript-go/internal/ast.IsVariableDeclaration
func IsVariableDeclaration(node *ast.Node) bool

//go:linkname IsVariableStatement github.com/microsoft/typescript-go/internal/ast.IsVariableStatement
func IsVariableStatement(node *ast.Node) bool

const KindAmpersandAmpersandToken = ast.KindAmpersandAmpersandToken
const KindBarBarToken = ast.KindBarBarToken
const KindCommaToken = ast.KindCommaToken
const KindEqualsToken = ast.KindEqualsToken
const KindFalseKeyword = ast.KindFalseKeyword
const KindNullKeyword = ast.KindNullKeyword
const KindDefaultClause = ast.KindDefaultClause
const KindJsxClosingElement = ast.KindJsxClosingElement
const KindJsxOpeningElement = ast.KindJsxOpeningElement
const KindJsxSelfClosingElement = ast.KindJsxSelfClosingElement
const KindMinusToken = ast.KindMinusToken
const KindPlusToken = ast.KindPlusToken
const KindPropertySignature = ast.KindPropertySignature
const KindQuestionQuestionToken = ast.KindQuestionQuestionToken
const KindTrueKeyword = ast.KindTrueKeyword
const KindUndefinedKeyword = ast.KindUndefinedKeyword
const InternalSymbolNamePrefix = ast.InternalSymbolNamePrefix
const ModifierFlagsAsync = ast.ModifierFlagsAsync
const ModifierFlagsExport = ast.ModifierFlagsExport
const ModifierFlagsReadonly = ast.ModifierFlagsReadonly

type Node = ast.Node
type SourceFile = ast.SourceFile
type Symbol = ast.Symbol
type Kind = ast.Kind

const SymbolFlagsAlias = ast.SymbolFlagsAlias
const SymbolFlagsOptional = ast.SymbolFlagsOptional
const SymbolFlagsValue = ast.SymbolFlagsValue
