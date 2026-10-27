// The App.tsx positions on an error stack, in stack order (innermost first). Pure, so it is
// unit-tested outside Vitest (frames.test.mjs); runner.tsx imports it from its work directory.
/**
 * @param {string} id the case id (its directory name)
 * @param {string} stack an Error.stack, source-mapped by Vitest
 * @returns {{ file: string, line: number, column: number }[]}
 */
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
