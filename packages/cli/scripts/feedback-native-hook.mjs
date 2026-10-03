import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import ts from "typescript";

// Reviewed RC.9 shared reader/untrack sites from native-read-hook-v3 in the
// experiment. Exact bytes own this profile; names alone confer no behavior.
export const sharedReaderSha256 = "sha256:70b88ba97dcb1107878ccc161cd00651b3cff09d7e17aee5443f1cbf9689463e";
export function instrumentFeedbackReads(text, path, runtimeSpecifier) {
  assert.equal(`sha256:${createHash("sha256").update(text).digest("hex")}`, sharedReaderSha256,
    "Native reader bytes have no reviewed development profile");
  const f = ts.factory, source = ts.createSourceFile(path, text, ts.ScriptTarget.Latest, true, ts.ScriptKind.JS);
  // TypeScript only parses and prints the pinned JavaScript. Package behavior,
  // call dispatch, and user-source semantics are not inferred in this adapter.
  let prefix = "__solidCheckerRead";
  while (text.includes(prefix)) prefix += "_";
  const id = suffix => f.createIdentifier(prefix + suffix);
  const trace = () => f.createPropertyAccessExpression(f.createIdentifier("globalThis"), "__solidCheckerReads");
  const call = (name, args) => f.createCallExpression(f.createPropertyAccessExpression(id("Trace"), name), undefined, args);
  const decl = (name, value) => f.createVariableStatement(undefined, f.createVariableDeclarationList([
    f.createVariableDeclaration(name, undefined, undefined, value)
  ], ts.NodeFlags.Const));
  let reader = 0, untrack = 0, observer = 0;
  const result = ts.transpileModule(text, { fileName: path,
    compilerOptions: { target: ts.ScriptTarget.ESNext, module: ts.ModuleKind.ESNext, sourceMap: true, inlineSources: true },
    transformers: { before: [context => root => {
      function visit(node) {
        if (ts.isFunctionDeclaration(node) && node.getStart(source) === 130767 && node.end === 130897) {
          observer++;
          function returned(child) {
            if (ts.isReturnStatement(child)) return f.updateReturnStatement(child,
              f.createConditionalExpression(id("Trace"), undefined, call("observerQuery", [child.expression, f.createIdentifier("getOwner")]), undefined, child.expression));
            return ts.visitEachChild(child, returned, context);
          }
          const body = ts.visitNode(node.body, returned);
          return f.updateFunctionDeclaration(node, node.modifiers, node.asteriskToken, node.name, node.typeParameters,
            node.parameters, node.type, f.updateBlock(body, [decl(id("Trace"), trace()), ...body.statements]));
        }
        if (ts.isFunctionDeclaration(node) && node.getStart(source) === 262883 && node.end === 271299) {
          reader++;
          function returned(child) {
            if (child !== node.body && ts.isFunctionLike(child)) return child;
            if (ts.isReturnStatement(child)) {
              const value = child.expression ?? f.createVoidZero();
              const next = f.updateReturnStatement(child, f.createConditionalExpression(id("Trace"), undefined,
                call("finish", [value, id("Ticket")]), undefined, value));
              return ts.setTextRange(ts.setOriginalNode(next, child), child);
            }
            return ts.visitEachChild(child, returned, context);
          }
          const body = ts.visitNode(node.body, returned);
          return f.updateFunctionDeclaration(node, node.modifiers, node.asteriskToken, node.name, node.typeParameters,
            node.parameters, node.type, f.updateBlock(body, [decl(id("Trace"), trace()),
              decl(id("Ticket"), f.createConditionalExpression(id("Trace"), undefined, call("begin", [
                f.createIdentifier("el"), f.createIdentifier("getObserver"), f.createIdentifier("getOwner")
              ]), undefined, f.createNull())), ...body.statements]));
        }
        if (ts.isFunctionDeclaration(node) && node.getStart(source) === 242109 && node.end === 242584) {
          untrack++;
          return f.updateFunctionDeclaration(node, node.modifiers, node.asteriskToken, node.name, node.typeParameters,
            node.parameters, node.type, f.createBlock([decl(id("Trace"), trace()),
              decl(id("Intent"), f.createConditionalExpression(id("Trace"), undefined, call("enterIntent", []), undefined, f.createNumericLiteral(0))),
              f.createTryStatement(node.body, undefined, f.createBlock([f.createIfStatement(id("Trace"),
                f.createExpressionStatement(call("leaveIntent", [id("Intent")])))]))]));
        }
        return ts.visitEachChild(node, visit, context);
      }
      const transformed = ts.visitNode(root, visit);
      return f.updateSourceFile(transformed, [f.createImportDeclaration(undefined, undefined,
        f.createStringLiteral(runtimeSpecifier)), ...transformed.statements]);
    }] }
  });
  assert.equal(reader, 1); assert.equal(untrack, 1); assert.equal(observer, 1);
  return { code: result.outputText, map: JSON.parse(result.sourceMapText) };
}
