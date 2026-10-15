// The host POSIX reference for the path-semantics controls. Always run under
// Node: libuv hands each path to the OS unchanged, while Bun's realpath cleans
// `..` first, which would make the reference itself lexical.
//   node path-host.mjs <request.json> <out.json>
import { readFileSync, writeFileSync } from "node:fs";
import { hostRow } from "./path-semantics.mjs";

const [request, out] = process.argv.slice(2);
const { root, rows, mutations } = JSON.parse(readFileSync(request, "utf8"));
const results = {};
for (const row of rows) results[row.id] = hostRow(row, root);
for (const { row, root: copy } of mutations) results[row.id] = hostRow(row, copy);
writeFileSync(out, `${JSON.stringify(results, null, 1)}\n`);
