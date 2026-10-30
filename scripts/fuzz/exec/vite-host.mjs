// The Rust executor's Vite helper: the Chrome host's Vite server (chrome-host.mjs `startViteHost`, the same plugin options and page),
// in its own process so its transforms and source-map work never wait on the orchestrator's thread. It prints one line,
// `{ base, work, control }`, once it serves; `control` answers the executor:
//   POST /__sc/freeze        freeze the runner as transformed (after the executor's warm page loaded it)
//   POST /__sc/stage         stage cases (a JSON array of `{ id, dir, variant, files? }`) where the pages load them (in memory)
//   POST /__sc/map?id=<id>   a record (JSON body) with its App.tsx positions mapped through Vite's source maps
//   POST /__sc/done          forget the cases (a JSON array of ids)
// The executor kills this process when its run ends and removes `work`.
import http from "node:http";
import { startViteHost } from "../chrome-host.mjs";

const settings = JSON.parse(process.argv[2] ?? "{}");
const vite = await startViteHost({ install: settings.install });

const routes = {
  "/__sc/freeze": async () => { await vite.freeze(); return {}; },
  "/__sc/stage": async (body) => { vite.stage(JSON.parse(body)); return {}; },
  "/__sc/map": async (body, url) => vite.map(url.searchParams.get("id"), JSON.parse(body)),
  "/__sc/done": async (body) => { await vite.done(JSON.parse(body)); return {}; },
};

const control = http.createServer((request, response) => {
  const chunks = [];
  request.on("data", (chunk) => chunks.push(chunk));
  request.on("end", async () => {
    const url = new URL(request.url ?? "/", "http://127.0.0.1");
    const route = routes[url.pathname];
    try {
      if (!route) throw new Error(`no route ${url.pathname}`);
      const text = JSON.stringify(await route(Buffer.concat(chunks).toString("utf8"), url));
      response.writeHead(200, { "Content-Type": "application/json", "Content-Length": Buffer.byteLength(text) });
      response.end(text);
    } catch (error) {
      const text = String(error?.stack ?? error);
      response.writeHead(500, { "Content-Type": "text/plain", "Content-Length": Buffer.byteLength(text) });
      response.end(text);
    }
  });
});
control.listen(0, "127.0.0.1", () => {
  process.stdout.write(`${JSON.stringify({ base: vite.base, work: vite.work, control: `http://127.0.0.1:${control.address().port}/` })}\n`);
});
