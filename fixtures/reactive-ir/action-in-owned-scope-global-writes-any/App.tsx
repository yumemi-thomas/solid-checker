import "fz-wany";

// A global written under a name the syntax does not fix (`Object.assign(globalThis, source)`) is not followed: the writer is recorded (`global_writers_any`), and no call of a
// global is a site for it. Measured: such a writer (storybook-solidjs-vite) made a row of every `Math.max`, `JSON.parse` and `new Map` of a twelve-file project.
export function AnyGlobal() {
  btoa("x"); // the one `source` holds: silent, a named miss
  atob("eA==");
  return null;
}
