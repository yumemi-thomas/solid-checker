// Package ast re-exports the slice of TypeScript's tsc internal ast
// package that this repository uses — nothing more. Declarations are
// copied from oxc-project/tsgolint's generated shims (MIT); the module
// path claims the TypeScript/tsc prefix so the internal imports resolve.
// Regenerate by hand when a compiler bump moves an identifier: the
// compiler reports alias breaks, and the go:linkname signatures below
// must be re-verified against the target revision by eye.
package ast

import ast "github.com/microsoft/TypeScript/tsc/internal/ast"
import _ "unsafe"

type Diagnostic = ast.Diagnostic
type SourceFileParseOptions = ast.SourceFileParseOptions

//go:linkname GetNodeAtPosition github.com/microsoft/TypeScript/tsc/internal/ast.GetNodeAtPosition
func GetNodeAtPosition(file *ast.SourceFile, position int, includeJSDoc bool) *ast.Node

//go:linkname GetSourceFileOfNode github.com/microsoft/TypeScript/tsc/internal/ast.GetSourceFileOfNode
func GetSourceFileOfNode(node *ast.Node) *ast.SourceFile

//go:linkname GetAssignmentTarget github.com/microsoft/TypeScript/tsc/internal/ast.GetAssignmentTarget
func GetAssignmentTarget(node *ast.Node) *ast.Node

//go:linkname HasSyntacticModifier github.com/microsoft/TypeScript/tsc/internal/ast.HasSyntacticModifier
func HasSyntacticModifier(node *ast.Node, flags ast.ModifierFlags) bool

//go:linkname IsExpression github.com/microsoft/TypeScript/tsc/internal/ast.IsExpression
func IsExpression(node *ast.Node) bool

//go:linkname IsOptionalChain github.com/microsoft/TypeScript/tsc/internal/ast.IsOptionalChain
func IsOptionalChain(node *ast.Node) bool

//go:linkname IsAsExpression github.com/microsoft/TypeScript/tsc/internal/ast.IsAsExpression
func IsAsExpression(node *ast.Node) bool

//go:linkname IsAssertionExpression github.com/microsoft/TypeScript/tsc/internal/ast.IsAssertionExpression
func IsAssertionExpression(node *ast.Node) bool

//go:linkname IsArrayBindingPattern github.com/microsoft/TypeScript/tsc/internal/ast.IsArrayBindingPattern
func IsArrayBindingPattern(node *ast.Node) bool

//go:linkname IsLabeledStatement github.com/microsoft/TypeScript/tsc/internal/ast.IsLabeledStatement
func IsLabeledStatement(node *ast.Node) bool

//go:linkname IsPropertyAssignment github.com/microsoft/TypeScript/tsc/internal/ast.IsPropertyAssignment
func IsPropertyAssignment(node *ast.Node) bool

//go:linkname IsArrayLiteralExpression github.com/microsoft/TypeScript/tsc/internal/ast.IsArrayLiteralExpression
func IsArrayLiteralExpression(node *ast.Node) bool

//go:linkname IsObjectBindingPattern github.com/microsoft/TypeScript/tsc/internal/ast.IsObjectBindingPattern
func IsObjectBindingPattern(node *ast.Node) bool

//go:linkname IsArrowFunction github.com/microsoft/TypeScript/tsc/internal/ast.IsArrowFunction
func IsArrowFunction(node *ast.Node) bool

//go:linkname IsAwaitExpression github.com/microsoft/TypeScript/tsc/internal/ast.IsAwaitExpression
func IsAwaitExpression(node *ast.Node) bool

//go:linkname IsBinaryExpression github.com/microsoft/TypeScript/tsc/internal/ast.IsBinaryExpression
func IsBinaryExpression(node *ast.Node) bool

//go:linkname IsEnumMember github.com/microsoft/TypeScript/tsc/internal/ast.IsEnumMember
func IsEnumMember(node *ast.Node) bool

//go:linkname IsBindingElement github.com/microsoft/TypeScript/tsc/internal/ast.IsBindingElement
func IsBindingElement(node *ast.Node) bool

//go:linkname IsBlock github.com/microsoft/TypeScript/tsc/internal/ast.IsBlock
func IsBlock(node *ast.Node) bool

//go:linkname IsBreakStatement github.com/microsoft/TypeScript/tsc/internal/ast.IsBreakStatement
func IsBreakStatement(node *ast.Node) bool

//go:linkname IsCallExpression github.com/microsoft/TypeScript/tsc/internal/ast.IsCallExpression
func IsCallExpression(node *ast.Node) bool

//go:linkname IsClassDeclaration github.com/microsoft/TypeScript/tsc/internal/ast.IsClassDeclaration
func IsClassDeclaration(node *ast.Node) bool

//go:linkname IsClassExpression github.com/microsoft/TypeScript/tsc/internal/ast.IsClassExpression
func IsClassExpression(node *ast.Node) bool

// IsClassStaticBlockDeclaration dereferences its argument; callers guard nil.
//
//go:linkname IsClassStaticBlockDeclaration github.com/microsoft/TypeScript/tsc/internal/ast.IsClassStaticBlockDeclaration
func IsClassStaticBlockDeclaration(node *ast.Node) bool

//go:linkname IsConditionalExpression github.com/microsoft/TypeScript/tsc/internal/ast.IsConditionalExpression
func IsConditionalExpression(node *ast.Node) bool

//go:linkname IsEnumDeclaration github.com/microsoft/TypeScript/tsc/internal/ast.IsEnumDeclaration
func IsEnumDeclaration(node *ast.Node) bool

//go:linkname IsDeclarationNameOrImportPropertyName github.com/microsoft/TypeScript/tsc/internal/ast.IsDeclarationNameOrImportPropertyName
func IsDeclarationNameOrImportPropertyName(name *ast.Node) bool

//go:linkname IsExternalModule github.com/microsoft/TypeScript/tsc/internal/ast.IsExternalModule
func IsExternalModule(file *ast.SourceFile) bool

//go:linkname IsExternalModuleAugmentation github.com/microsoft/TypeScript/tsc/internal/ast.IsExternalModuleAugmentation
func IsExternalModuleAugmentation(node *ast.Node) bool

//go:linkname IsFunctionDeclaration github.com/microsoft/TypeScript/tsc/internal/ast.IsFunctionDeclaration
func IsFunctionDeclaration(node *ast.Node) bool

//go:linkname IsFunctionExpression github.com/microsoft/TypeScript/tsc/internal/ast.IsFunctionExpression
func IsFunctionExpression(node *ast.Node) bool

// IsFunctionLikeDeclaration is the compiler's own closed enumeration of the
// function-like kinds that own a body: function declarations and expressions,
// arrows, methods, constructors, and get/set accessors. Taking it from the
// compiler rather than restating it means a kind added by a future revision is
// covered here without an edit.
//
//go:linkname IsFunctionLikeDeclaration github.com/microsoft/TypeScript/tsc/internal/ast.IsFunctionLikeDeclaration
func IsFunctionLikeDeclaration(node *ast.Node) bool

//go:linkname IsGlobalScopeAugmentation github.com/microsoft/TypeScript/tsc/internal/ast.IsGlobalScopeAugmentation
func IsGlobalScopeAugmentation(node *ast.Node) bool

//go:linkname IsIdentifier github.com/microsoft/TypeScript/tsc/internal/ast.IsIdentifier
func IsIdentifier(node *ast.Node) bool

//go:linkname IsNoSubstitutionTemplateLiteral github.com/microsoft/TypeScript/tsc/internal/ast.IsNoSubstitutionTemplateLiteral
func IsNoSubstitutionTemplateLiteral(node *ast.Node) bool

//go:linkname IsNonNullExpression github.com/microsoft/TypeScript/tsc/internal/ast.IsNonNullExpression
func IsNonNullExpression(node *ast.Node) bool

//go:linkname IsNumericLiteral github.com/microsoft/TypeScript/tsc/internal/ast.IsNumericLiteral
func IsNumericLiteral(node *ast.Node) bool

//go:linkname IsParenthesizedExpression github.com/microsoft/TypeScript/tsc/internal/ast.IsParenthesizedExpression
func IsParenthesizedExpression(node *ast.Node) bool

//go:linkname IsParameterDeclaration github.com/microsoft/TypeScript/tsc/internal/ast.IsParameterDeclaration
func IsParameterDeclaration(node *ast.Node) bool

//go:linkname IsPostfixUnaryExpression github.com/microsoft/TypeScript/tsc/internal/ast.IsPostfixUnaryExpression
func IsPostfixUnaryExpression(node *ast.Node) bool

//go:linkname IsPrefixUnaryExpression github.com/microsoft/TypeScript/tsc/internal/ast.IsPrefixUnaryExpression
func IsPrefixUnaryExpression(node *ast.Node) bool

//go:linkname IsPropertyAccessExpression github.com/microsoft/TypeScript/tsc/internal/ast.IsPropertyAccessExpression
func IsPropertyAccessExpression(node *ast.Node) bool

//go:linkname IsElementAccessExpression github.com/microsoft/TypeScript/tsc/internal/ast.IsElementAccessExpression
func IsElementAccessExpression(node *ast.Node) bool

//go:linkname IsDeleteExpression github.com/microsoft/TypeScript/tsc/internal/ast.IsDeleteExpression
func IsDeleteExpression(node *ast.Node) bool

//go:linkname IsObjectLiteralExpression github.com/microsoft/TypeScript/tsc/internal/ast.IsObjectLiteralExpression
func IsObjectLiteralExpression(node *ast.Node) bool

//go:linkname IsPropertyDeclaration github.com/microsoft/TypeScript/tsc/internal/ast.IsPropertyDeclaration
func IsPropertyDeclaration(node *ast.Node) bool

//go:linkname IsSatisfiesExpression github.com/microsoft/TypeScript/tsc/internal/ast.IsSatisfiesExpression
func IsSatisfiesExpression(node *ast.Node) bool

//go:linkname IsStringLiteral github.com/microsoft/TypeScript/tsc/internal/ast.IsStringLiteral
func IsStringLiteral(node *ast.Node) bool

//go:linkname IsVarConst github.com/microsoft/TypeScript/tsc/internal/ast.IsVarConst
func IsVarConst(node *ast.Node) bool

//go:linkname IsIfStatement github.com/microsoft/TypeScript/tsc/internal/ast.IsIfStatement
func IsIfStatement(node *ast.Node) bool

//go:linkname IsImportClause github.com/microsoft/TypeScript/tsc/internal/ast.IsImportClause
func IsImportClause(node *ast.Node) bool

//go:linkname IsImportDeclaration github.com/microsoft/TypeScript/tsc/internal/ast.IsImportDeclaration
func IsImportDeclaration(node *ast.Node) bool

//go:linkname IsImportSpecifier github.com/microsoft/TypeScript/tsc/internal/ast.IsImportSpecifier
func IsImportSpecifier(node *ast.Node) bool

//go:linkname IsIterationStatement github.com/microsoft/TypeScript/tsc/internal/ast.IsIterationStatement
func IsIterationStatement(node *ast.Node, lookInLabeledStatements bool) bool

//go:linkname IsLogicalOrCoalescingAssignmentOperator github.com/microsoft/TypeScript/tsc/internal/ast.IsLogicalOrCoalescingAssignmentOperator
func IsLogicalOrCoalescingAssignmentOperator(kind ast.Kind) bool

//go:linkname IsMethodDeclaration github.com/microsoft/TypeScript/tsc/internal/ast.IsMethodDeclaration
func IsMethodDeclaration(node *ast.Node) bool

//go:linkname IsModuleDeclaration github.com/microsoft/TypeScript/tsc/internal/ast.IsModuleDeclaration
func IsModuleDeclaration(node *ast.Node) bool

//go:linkname IsNamespaceImport github.com/microsoft/TypeScript/tsc/internal/ast.IsNamespaceImport
func IsNamespaceImport(node *ast.Node) bool

//go:linkname IsNewExpression github.com/microsoft/TypeScript/tsc/internal/ast.IsNewExpression
func IsNewExpression(node *ast.Node) bool

//go:linkname IsPartOfTypeNode github.com/microsoft/TypeScript/tsc/internal/ast.IsPartOfTypeNode
func IsPartOfTypeNode(node *ast.Node) bool

//go:linkname IsPartOfTypeOnlyImportOrExportDeclaration github.com/microsoft/TypeScript/tsc/internal/ast.IsPartOfTypeOnlyImportOrExportDeclaration
func IsPartOfTypeOnlyImportOrExportDeclaration(node *ast.Node) bool

//go:linkname IsQualifiedName github.com/microsoft/TypeScript/tsc/internal/ast.IsQualifiedName
func IsQualifiedName(node *ast.Node) bool

//go:linkname IsReturnStatement github.com/microsoft/TypeScript/tsc/internal/ast.IsReturnStatement
func IsReturnStatement(node *ast.Node) bool

//go:linkname IsShorthandPropertyAssignment github.com/microsoft/TypeScript/tsc/internal/ast.IsShorthandPropertyAssignment
func IsShorthandPropertyAssignment(node *ast.Node) bool

//go:linkname IsSpreadElement github.com/microsoft/TypeScript/tsc/internal/ast.IsSpreadElement
func IsSpreadElement(node *ast.Node) bool

//go:linkname IsSwitchStatement github.com/microsoft/TypeScript/tsc/internal/ast.IsSwitchStatement
func IsSwitchStatement(node *ast.Node) bool

//go:linkname IsThrowStatement github.com/microsoft/TypeScript/tsc/internal/ast.IsThrowStatement
func IsThrowStatement(node *ast.Node) bool

//go:linkname IsTryStatement github.com/microsoft/TypeScript/tsc/internal/ast.IsTryStatement
func IsTryStatement(node *ast.Node) bool

//go:linkname IsTypeReferenceNode github.com/microsoft/TypeScript/tsc/internal/ast.IsTypeReferenceNode
func IsTypeReferenceNode(node *ast.Node) bool

//go:linkname IsVariableDeclaration github.com/microsoft/TypeScript/tsc/internal/ast.IsVariableDeclaration
func IsVariableDeclaration(node *ast.Node) bool

//go:linkname IsVariableStatement github.com/microsoft/TypeScript/tsc/internal/ast.IsVariableStatement
func IsVariableStatement(node *ast.Node) bool

const KindAmpersandAmpersandToken = ast.KindAmpersandAmpersandToken
const KindBarBarToken = ast.KindBarBarToken
const KindCommaToken = ast.KindCommaToken
const KindEqualsToken = ast.KindEqualsToken
const KindExclamationToken = ast.KindExclamationToken
const KindFalseKeyword = ast.KindFalseKeyword
const KindNullKeyword = ast.KindNullKeyword
const KindDefaultClause = ast.KindDefaultClause
const KindJsxClosingElement = ast.KindJsxClosingElement
const KindJsxOpeningElement = ast.KindJsxOpeningElement
const KindJsxSelfClosingElement = ast.KindJsxSelfClosingElement
const KindMinusToken = ast.KindMinusToken
const KindPlusToken = ast.KindPlusToken
const KindPropertySignature = ast.KindPropertySignature

// KindFirstNode is the compiler's own boundary between tokens — trivia,
// literals, punctuation, identifiers, and keywords — and the node kinds above
// them. Everything strictly below it is a token, which is the only way to say
// "this is not a syntactic construct" without restating a 160-entry list that
// a compiler bump could silently extend.
const KindFirstNode = ast.KindFirstNode

// NodeFlagsUsing is set on the declaration list of both `using x = …` and
// `await using x = …`; the latter additionally sets NodeFlagsConst, which is
// why testing this one bit covers both spellings.
const NodeFlagsUsing = ast.NodeFlagsUsing

// NodeFlagsConst marks a `const` declaration list (ADR 0153 asks it of a
// context binding).
const NodeFlagsConst = ast.NodeFlagsConst
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
const SymbolFlagsClass = ast.SymbolFlagsClass
const SymbolFlagsInterface = ast.SymbolFlagsInterface
const SymbolFlagsEnum = ast.SymbolFlagsEnum
const SymbolFlagsTypeAlias = ast.SymbolFlagsTypeAlias
