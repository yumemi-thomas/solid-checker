import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import ts from "typescript";

// Syntax transformation only. The native analysis selects exact functions,
// operations, derived origins and return-expression relationships.
export function instrumentFeedbackSource(text, path, model, runtimeSpecifier) {
  assert.equal(model.path, path);
  assert.equal(model.sourceSha256, `sha256:${createHash("sha256").update(text).digest("hex")}`, "Development model source changed");
  const source = ts.createSourceFile(path, text, ts.ScriptTarget.Latest, true,
    /x$/.test(path) ? ts.ScriptKind.TSX : ts.ScriptKind.TS), f = ts.factory;
  const bytes = offset => Buffer.byteLength(text.slice(0, offset));
  const key = span => `${span.start}:${span.end}`;
  const at = node => key({ start: bytes(node.getStart(source)), end: bytes(node.end) });
  const functions = new Map(model.functions.map(row => [key(row.span), row]));
  const operations = new Map(model.operations.map(row => [key(row.span), row]));
  let prefix = "__scDevelopment"; while (text.includes(prefix)) prefix += "_";
  const runtime = f.createIdentifier(prefix + "Runtime"), tokens = new Map();
  for (const row of model.functions) tokens.set(key(row.span), f.createIdentifier(`${prefix}_${row.span.start}`));
  const call = (name, args) => f.createCallExpression(f.createPropertyAccessExpression(runtime, name), undefined, args);
  const data = value => f.createStringLiteral(JSON.stringify(value));
  const zero = () => f.createNull();
  const counts = { functions: 0, operations: 0, skipped: 0 };
  const matched = new Set();
  function suspends(node) {
    if (ts.isFunctionLike(node)) return false;
    if (ts.isAwaitExpression(node) || ts.isYieldExpression(node)) return true;
    return ts.forEachChild(node, suspends) ?? false;
  }
  const result = ts.transpileModule(text, { fileName: path,
    compilerOptions: { target: ts.ScriptTarget.ESNext, module: ts.ModuleKind.ESNext, jsx: ts.JsxEmit.Preserve,
      sourceMap: true, inlineSources: true },
    transformers: { before: [context => root => {
      function visit(node, active = null) {
        if (ts.isFunctionLike(node)) {
          // Oxc's function span excludes the containing export declaration;
          // TypeScript includes its export/default modifiers in this node.
          const modifier = node.modifiers?.find(item => ![ts.SyntaxKind.ExportKeyword, ts.SyntaxKind.DefaultKeyword].includes(item.kind));
          const keyword = ts.isFunctionDeclaration(node) && node.getChildren(source).find(item => item.kind === ts.SyntaxKind.FunctionKeyword);
          const canonical = modifier ?? keyword;
          const bodies = node.body ? model.functions.filter(row => key(row.body) === at(node.body)) : [];
          const row = functions.get(at(node)) ?? (canonical ? functions.get(key({ start: bytes(canonical.getStart(source)), end: bytes(node.end) })) : undefined) ??
            (bodies.length === 1 ? bodies[0] : null);
          if (!row || row.generator || !(ts.isArrowFunction(node) || ts.isFunctionExpression(node) || ts.isFunctionDeclaration(node) ||
            ts.isMethodDeclaration(node) || ts.isGetAccessorDeclaration(node) || ts.isSetAccessorDeclaration(node) || ts.isConstructorDeclaration(node)) || !node.body) {
            counts.skipped++; return node;
          }
          matched.add(key(row.span)); counts.functions++;
          const token = tokens.get(key(row.span));
          const parent = row.parent && matched.has(key(row.parent)) ? tokens.get(key(row.parent)) : zero();
          const body = ts.visitNode(node.body, child => visit(child, row));
          const statements = ts.isBlock(body) ? [...body.statements] : [f.createReturnStatement(body)];
          // Keep directive prologues at the beginning of the body.
          let boundary = 0;
          while (boundary < statements.length && ts.isExpressionStatement(statements[boundary]) && ts.isStringLiteral(statements[boundary].expression)) boundary++;
          statements.splice(boundary, 0, f.createVariableStatement(undefined, f.createVariableDeclarationList([
            f.createVariableDeclaration(token, undefined, undefined, call("enterFunction", [data({ path, sourceSha256: model.sourceSha256, ...row }), parent]))
          ], ts.NodeFlags.Const)));
          const nextBody = f.createBlock(statements, true);
          if (ts.isArrowFunction(node)) return f.updateArrowFunction(node, node.modifiers, node.typeParameters, node.parameters, node.type, node.equalsGreaterThanToken, nextBody);
          if (ts.isFunctionExpression(node)) return f.updateFunctionExpression(node, node.modifiers, node.asteriskToken, node.name, node.typeParameters, node.parameters, node.type, nextBody);
          if (ts.isMethodDeclaration(node)) return f.updateMethodDeclaration(node, node.modifiers, node.asteriskToken, node.name, node.questionToken, node.typeParameters, node.parameters, node.type, nextBody);
          if (ts.isGetAccessorDeclaration(node)) return f.updateGetAccessorDeclaration(node, node.modifiers, node.name, node.parameters, node.type, nextBody);
          if (ts.isSetAccessorDeclaration(node)) return f.updateSetAccessorDeclaration(node, node.modifiers, node.name, node.parameters, nextBody);
          if (ts.isConstructorDeclaration(node)) return f.updateConstructorDeclaration(node, node.modifiers, node.parameters, nextBody);
          return f.updateFunctionDeclaration(node, node.modifiers, node.asteriskToken, node.name, node.typeParameters, node.parameters, node.type, nextBody);
        }
        const operation = ts.isCallExpression(node) && operations.get(at(node));
        const next = ts.visitEachChild(node, child => visit(child, active), context);
        if (!operation || !active || key(operation.function) !== key(active.span)) return next;
        // Moving await/yield or direct eval into a new arrow changes semantics.
        if (suspends(node) || node.expression.kind === ts.SyntaxKind.SuperKeyword || node.expression.kind === ts.SyntaxKind.ImportKeyword ||
          (ts.isIdentifier(node.expression) && node.expression.text === "eval")) { counts.skipped++; return next; }
        counts.operations++;
        return ts.setTextRange(ts.setOriginalNode(call("withOperation", [tokens.get(key(active.span)),
          data({ path, sourceSha256: model.sourceSha256, ...operation }),
          f.createArrowFunction(undefined, undefined, [], undefined, f.createToken(ts.SyntaxKind.EqualsGreaterThanToken), next)]), node), node);
      }
      const transformed = ts.visitNode(root, visit);
      const statements = [...transformed.statements];
      let boundary = 0;
      while (boundary < statements.length && ts.isExpressionStatement(statements[boundary]) && ts.isStringLiteral(statements[boundary].expression)) boundary++;
      statements.splice(boundary, 0, f.createImportDeclaration(undefined,
        f.createImportClause(false, undefined, f.createNamedImports([f.createImportSpecifier(false, f.createIdentifier("reads"), runtime)])),
        f.createStringLiteral(runtimeSpecifier)));
      return f.updateSourceFile(transformed, statements);
    }] }
  });
  return { code: result.outputText, map: JSON.parse(result.sourceMapText), counts };
}
