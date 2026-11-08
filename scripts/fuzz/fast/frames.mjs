// The App.tsx positions on an error stack, in stack order (innermost first). Pure, so it is
// unit-tested outside Vitest (frames.test.mjs); runner.tsx imports it from its work directory.
/**
 * @param {string} id the case id (its directory name)
 * @param {string} stack an Error.stack, source-mapped by Vitest
 * @returns {{ file: string, line: number, column: number }[]}
 */
/**
 * Review R66 (ADR 0907): every frame of the case's own modules, innermost first: App.tsx with its position, any other module of the case
 * (`helpers.ts`) as `{ file }` only, as the Chrome host records it (chrome-host/runner.tsx `servedFrames`, then `mapPositions`). Package and
 * runtime frames are not the case's and are left out.
 * @param {string} id the case id (its directory name)
 * @param {string} stack an Error.stack, source-mapped by Vitest
 * @returns {({ file: string, line: number, column: number } | { file: string })[]}
 */
export function programFrames(id, stack) {
  const rows = [];
  for (const text of String(stack ?? "").split("\n")) {
    const at = text.indexOf(`${id}/`);
    if (at < 0) continue;
    const match = /^([^/:?\s)]+\.tsx?)(?:\?[^:\s)]*)?:(\d+):(\d+)/.exec(text.slice(at + id.length + 1));
    if (!match) continue;
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
