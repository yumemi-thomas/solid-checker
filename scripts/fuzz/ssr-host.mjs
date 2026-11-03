// The SSR host's render process (ssr.mjs `renderPrograms`): Node with `--conditions=development`, so `@solidjs/web` and `solid-js`
// resolve to the pinned install's `server.dev.js` builds, as a dev server renders. It reads one JSON array of `{ id, module }`
// (`module`: the program's App compiled for SSR, an absolute .mjs path inside the install) on stdin, renders each through
// `renderToStream` inside its own request event (`createRequestEvent`, the `solid.RequestContext` store a server adapter installs),
// turns the stream into a `Response` with `createSSRResponse` (the head commits at the shell flush), reads the whole body, and prints
// one JSON line per program: `{ id, status, headers, body }` or `{ id, failure }`. Programs run one at a time: a request's head is its
// own. A program that does not finish within its deadline is a failure, never a head.
import { AsyncLocalStorage } from "node:async_hooks";
import { createRequire } from "node:module";
import { join } from "node:path";
import { pathToFileURL } from "node:url";

const install = process.argv[2];
const deadlineMs = Number(process.argv[3] ?? 10_000);
const require = createRequire(join(install, "package.json"));
const RequestContext = Symbol.for("solid.RequestContext");
globalThis[RequestContext] ??= new AsyncLocalStorage();
process.stdout.on("error", () => {});

const chunks = [];
for await (const chunk of process.stdin) chunks.push(chunk);
const programs = JSON.parse(Buffer.concat(chunks).toString("utf8"));
const web = await import(pathToFileURL(require.resolve("@solidjs/web")).href);
// The entry Node picked must be the pinned dev server build: a browser or production build would answer another runtime's question.
const entry = require.resolve("@solidjs/web");
const print = (row) => process.stdout.write(`${JSON.stringify(row)}\n`);
if (!entry.endsWith("/dist/server.dev.js")) {
  for (const { id } of programs) print({ id, failure: `@solidjs/web resolved to ${entry}, not dist/server.dev.js` });
  process.exit(0);
}

const within = (promise, ms, what) => {
  let timer;
  return Promise.race([promise, new Promise((_, reject) => { timer = setTimeout(() => reject(new Error(`${what}: no answer within ${ms} ms`)), ms); })]).finally(() => clearTimeout(timer));
};

for (const { id, module } of programs) {
  try {
    const App = (await import(pathToFileURL(module).href)).default;
    if (typeof App !== "function") throw new Error("the program exports no default component");
    const event = web.createRequestEvent(new Request(`http://ssr.test/${id}`));
    const response = await within(globalThis[RequestContext].run(event, () => web.createSSRResponse(web.renderToStream(() => App()), event)), deadlineMs, "the shell");
    const body = await within(response.text(), deadlineMs, "the stream");
    print({ id, status: response.status, headers: Object.fromEntries(response.headers.entries()), body: body.slice(0, 2000) });
  } catch (error) {
    print({ id, failure: String(error?.stack ?? error).split("\n").slice(0, 3).join(" | ").slice(0, 600) });
  }
}
