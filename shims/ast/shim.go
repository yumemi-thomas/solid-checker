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

//go:linkname HasSyntacticModifier github.com/microsoft/typescript-go/internal/ast.HasSyntacticModifier
func HasSyntacticModifier(node *ast.Node, flags ast.ModifierFlags) bool

//go:linkname IsArrayBindingPattern github.com/microsoft/typescript-go/internal/ast.IsArrayBindingPattern
func IsArrayBindingPattern(node *ast.Node) bool

//go:linkname IsArrowFunction github.com/microsoft/typescript-go/internal/ast.IsArrowFunction
func IsArrowFunction(node *ast.Node) bool

//go:linkname IsAwaitExpression github.com/microsoft/typescript-go/internal/ast.IsAwaitExpression
func IsAwaitExpression(node *ast.Node) bool

//go:linkname IsBinaryExpression github.com/microsoft/typescript-go/internal/ast.IsBinaryExpression
func IsBinaryExpression(node *ast.Node) bool

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

//go:linkname IsConditionalExpression github.com/microsoft/typescript-go/internal/ast.IsConditionalExpression
func IsConditionalExpression(node *ast.Node) bool

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

//go:linkname IsIfStatement github.com/microsoft/typescript-go/internal/ast.IsIfStatement
func IsIfStatement(node *ast.Node) bool

//go:linkname IsIterationStatement github.com/microsoft/typescript-go/internal/ast.IsIterationStatement
func IsIterationStatement(node *ast.Node, lookInLabeledStatements bool) bool

//go:linkname IsMethodDeclaration github.com/microsoft/typescript-go/internal/ast.IsMethodDeclaration
func IsMethodDeclaration(node *ast.Node) bool

//go:linkname IsModuleDeclaration github.com/microsoft/typescript-go/internal/ast.IsModuleDeclaration
func IsModuleDeclaration(node *ast.Node) bool

//go:linkname IsPartOfTypeNode github.com/microsoft/typescript-go/internal/ast.IsPartOfTypeNode
func IsPartOfTypeNode(node *ast.Node) bool

//go:linkname IsReturnStatement github.com/microsoft/typescript-go/internal/ast.IsReturnStatement
func IsReturnStatement(node *ast.Node) bool

//go:linkname IsSwitchStatement github.com/microsoft/typescript-go/internal/ast.IsSwitchStatement
func IsSwitchStatement(node *ast.Node) bool

//go:linkname IsThrowStatement github.com/microsoft/typescript-go/internal/ast.IsThrowStatement
func IsThrowStatement(node *ast.Node) bool

//go:linkname IsTryStatement github.com/microsoft/typescript-go/internal/ast.IsTryStatement
func IsTryStatement(node *ast.Node) bool

//go:linkname IsVariableDeclaration github.com/microsoft/typescript-go/internal/ast.IsVariableDeclaration
func IsVariableDeclaration(node *ast.Node) bool

//go:linkname IsVariableStatement github.com/microsoft/typescript-go/internal/ast.IsVariableStatement
func IsVariableStatement(node *ast.Node) bool

const KindAmpersandAmpersandToken = ast.KindAmpersandAmpersandToken
const KindBarBarToken = ast.KindBarBarToken
const KindDefaultClause = ast.KindDefaultClause
const KindQuestionQuestionToken = ast.KindQuestionQuestionToken
const ModifierFlagsAsync = ast.ModifierFlagsAsync
const ModifierFlagsExport = ast.ModifierFlagsExport

type Node = ast.Node
type SourceFile = ast.SourceFile
type Symbol = ast.Symbol

const SymbolFlagsAlias = ast.SymbolFlagsAlias
const SymbolFlagsValue = ast.SymbolFlagsValue
