import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import ts from "typescript";

// Reviewed shared reader/untrack sites of `@solidjs/signals/dist/dev-shared.js`,
// one profile per release. Exact bytes own a profile; names alone confer no
// behavior. RC.9's come from native-read-hook-v3 in the experiment. RC.13's
// `getObserver` is byte-identical to RC.9's; its `read` adds the post-await
// read check, the derived-override guard and an earlier strict-read warning,
// and its `untrack` counts its depth. Neither changes the parameter, the
// returns, or the body the instrumentation wraps.
export const readerProfiles = {
  "2.0.0-rc.9": { sha256: "sha256:70b88ba97dcb1107878ccc161cd00651b3cff09d7e17aee5443f1cbf9689463e",
    observer: [130767, 130897], reader: [262883, 271299], untrack: [242109, 242584] },
  "2.0.0-rc.13": { sha256: "sha256:c19016529420dcc5d028f6084c6d3fb0633835ea8558cd4dc1b34809a661d074",
    observer: [156682, 156812], reader: [301718, 311816], untrack: [277947, 278538] }
};
export function instrumentFeedbackReads(text, path, runtimeSpecifier) {
  const sha256 = `sha256:${createHash("sha256").update(text).digest("hex")}`;
  const profile = Object.values(readerProfiles).find(row => row.sha256 === sha256);
  assert(profile, "Native reader bytes have no reviewed development profile");
  const at = (node, [start, end]) => node.getStart(source) === start && node.end === end;
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
        if (ts.isFunctionDeclaration(node) && at(node, profile.observer)) {
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
        if (ts.isFunctionDeclaration(node) && at(node, profile.reader)) {
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
        if (ts.isFunctionDeclaration(node) && at(node, profile.untrack)) {
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
