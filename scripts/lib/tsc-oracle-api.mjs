// The oracle's only contact with TypeScript 7's `typescript/unstable/sync` API.
//
// Runs under Node, not Bun: the sync API blocks on the native `tsc` process
// through a file descriptor that Bun's child-process streams do not expose
// (`stdout._handle.fd`). `scripts/tsc-oracle.mjs` therefore calls this file
// once per request with `execFileSync`: one JSON request on stdin, one JSON
// response on stdout. Everything unstable about the API stays in this file;
// callers see byte-offset diagnostics only.
//
// Requests (`typescriptRoot` is the provisioned install, `typescript` is
// resolved from it and from nowhere else):
//   { op: "check", typescriptRoot, work, files, passes: [{ name, options }] }
//   { op: "subjects", typescriptRoot, name, code, ranges: [[startByte, endByte]] }
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { basename, join } from "node:path";
import { pathToFileURL } from "node:url";

const request = JSON.parse(readFileSync(0, "utf8"));
const require = createRequire(join(request.typescriptRoot, "package.json"));
const load = (specifier) => import(pathToFileURL(require.resolve(specifier)).href);
const { API, ScriptKind, DiagnosticCategory } = await load("typescript/unstable/sync");
const is = await load("typescript/unstable/ast/is");

const subjectOf = (node) =>
  is.isCallExpression(node) ||
  is.isNewExpression(node) ||
  is.isJsxAttribute(node) ||
  is.isJsxElement(node) ||
  is.isJsxSelfClosingElement(node) ||
  is.isReturnStatement(node) ||
  is.isVariableDeclaration(node);

// The smallest subject node containing [start, end) (UTF-16 offsets), or the
// whole file.
const enclosingSubject = (source, start, end) => {
  let best = source;
  const width = (node) => node.getEnd() - node.getStart(source, false);
  const visit = (node) => {
    if (node.getStart(source, false) > start || node.getEnd() < end) return undefined;
    if (subjectOf(node) && width(node) < width(best)) best = node;
    node.forEachChild(visit);
    return undefined;
  };
  visit(source);
  return [best.getStart(source, false), best.getEnd()];
};

// UTF-16 offset <-> UTF-8 byte offset, for the file being described.
const byteOffset = (text, offset) => Buffer.byteLength(text.slice(0, offset), "utf8");
const characterOffsetAtByte = (text, byte) => {
  let low = 0;
  let high = text.length;
  while (low < high) {
    const middle = Math.ceil((low + high) / 2);
    if (Buffer.byteLength(text.slice(0, middle), "utf8") <= byte) low = middle;
    else high = middle - 1;
  }
  return low;
};

// `ts.flattenDiagnosticMessageText(messageText, " ")`.
const flatten = (diagnostic) =>
  [diagnostic.text, ...(diagnostic.messageChain ?? []).map(flatten)].join(" ");

const scriptKindOf = (name) => {
  if (/\.tsx$/.test(name)) return ScriptKind.TSX;
  if (/\.jsx$/.test(name)) return ScriptKind.JSX;
  if (/\.js$/.test(name)) return ScriptKind.JS;
  return ScriptKind.TS;
};

const api = new API({ cwd: request.work ?? process.cwd() });
try {
  if (request.op === "subjects") {
    const lease = api.createSourceFile(request.name, request.code, {
      scriptKind: scriptKindOf(request.name),
    });
    const source = lease.sourceFile;
    const subjects = request.ranges.map(([startByte, endByte]) => {
      const [start, end] = enclosingSubject(
        source,
        characterOffsetAtByte(request.code, startByte),
        characterOffsetAtByte(request.code, endByte),
      );
      return [byteOffset(request.code, start), byteOffset(request.code, end)];
    });
    process.stdout.write(JSON.stringify({ subjects }));
  } else if (request.op === "check") {
    const passes = {};
    for (const { name, options } of request.passes) {
      const parsed = api.parseJsonConfigFileContent(
        { compilerOptions: options, files: request.files },
        { configDirectory: request.work },
      );
      if (parsed.errors?.length) {
        throw new Error(`compiler options rejected: ${parsed.errors.map(flatten).join("; ")}`);
      }
      const snapshot = api.createSnapshot({
        createPrograms: [{ rootFiles: parsed.fileNames, compilerOptions: parsed.options }],
      });
      const program = snapshot.getProjects()[0].program;
      const out = [];
      for (const file of request.files) {
        const source = program.getSourceFile(file);
        if (!source) continue;
        const text = source.text;
        const diagnostics = [
          ...program.getSyntacticDiagnostics(file),
          ...program.getSemanticDiagnostics(file),
        ];
        for (const diagnostic of diagnostics) {
          const start = diagnostic.pos;
          const end = diagnostic.end;
          const [subjectStart, subjectEnd] = enclosingSubject(source, start, end);
          out.push({
            code: diagnostic.code,
            category: DiagnosticCategory[diagnostic.category].toLowerCase(),
            message: flatten(diagnostic),
            file: basename(file),
            line: diagnostic.startPosition.line + 1,
            column: diagnostic.startPosition.character + 1,
            startByte: byteOffset(text, start),
            endByte: byteOffset(text, end),
            subjectStartByte: byteOffset(text, subjectStart),
            subjectEndByte: byteOffset(text, subjectEnd),
            text: text.slice(start, end),
          });
        }
      }
      passes[name] = out;
    }
    process.stdout.write(JSON.stringify({ passes }));
  } else {
    throw new Error(`unknown op ${request.op}`);
  }
} finally {
  api.close();
}
