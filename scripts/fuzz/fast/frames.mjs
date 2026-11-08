// The App.tsx positions on an error stack, in stack order (innermost first). Pure, so it is
// unit-tested outside Vitest (frames.test.mjs); runner.tsx imports it from its work directory.
/**
 * @param {string} id the case id (its directory name)
 * @param {string} stack an Error.stack, source-mapped by Vitest
 * @returns {{ file: string, line: number, column: number }[]}
 */
/**
 * Reviews R66 and its follow-up (ADR 0907): every frame of the program under test, innermost first: any file under the case's own
 * directory (`<id>/…`) but its `node_modules` (a generated package is a package), whatever its extension or depth. App.tsx keeps its
 * position; any other file (`helpers.ts`, `helpers.js`, `sub/helpers.ts`) is recorded by its path only, as the Chrome host records it.
 * Package and runtime frames are left out.
 * @param {string} id the case id (its directory name)
 * @param {string} stack an Error.stack, source-mapped by Vitest
 * @returns {({ file: string, line: number, column: number } | { file: string })[]}
 */
export function programFrames(id, stack) {
  const rows = [];
  for (const text of String(stack ?? "").split("\n")) {
    const at = text.indexOf(`/${id}/`);
    if (at < 0) continue;
    const match = /^([^:?\s)]+?)(?:\?[^:\s)]*)?:(\d+):(\d+)/.exec(text.slice(at + id.length + 2));
    if (!match || match[1].startsWith("node_modules/")) continue;
    rows.push(match[1] === "App.tsx" ? { file: "App.tsx", line: Number(match[2]), column: Number(match[3]) } : { file: match[1] });
  }
  return rows;
}

export function appFrames(id, stack) {
  const rows = [];
  for (const text of String(stack ?? "").split("\n")) {
    const at = text.indexOf(`${id}/App.tsx:`);
    if (at < 0) continue;
    const match = /^App\.tsx:(\d+):(\d+)/.exec(text.slice(at + id.length + 1));
    if (match) rows.push({ file: "App.tsx", line: Number(match[1]), column: Number(match[2]) });
  }
  return rows;
}
