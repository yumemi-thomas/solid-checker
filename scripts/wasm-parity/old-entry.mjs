// The package's pre-S2 entry points, resolved from the isolated install by
// their ordinary specifier: `require`, `import`, and one in-memory check.
import { createRequire } from "node:module";
import { createHash } from "node:crypto";

const require = createRequire(import.meta.url);
const cjs = require("solid-checker-wasm");
const esm = await import("solid-checker-wasm");
const projectId = "/workspace/example/tsconfig.json";
const path = "/workspace/example/src/App.tsx";
const source = "export default function App() { return <main />; }\n";
const snapshot = JSON.parse(cjs.checkSync(JSON.stringify({
  projectId,
  generation: 1,
  sources: [{
    path,
    source,
    compilerOptions: {
      moduleName: "dom", generate: "dom", hydratable: false, dev: false,
      effectWrapper: "", wrapConditionals: true, staticMarker: "_$", builtIns: [],
    },
  }],
  typeFacts: {
    schema: 2, generation: 1, projectId,
    sources: [{ path, sha256: `sha256:${createHash("sha256").update(source).digest("hex")}` }],
    entities: [], symbols: [], files: [],
  },
})));
console.log(JSON.stringify({
  engine: process.versions.bun ? `bun ${process.versions.bun}` : `node ${process.version}`,
  resolved: require.resolve("solid-checker-wasm"),
  requireExports: Object.keys(cjs).sort(),
  importCheckSync: typeof (esm.checkSync ?? esm.default?.checkSync),
  importPlanSync: typeof (esm.planSync ?? esm.default?.planSync),
  status: snapshot.status,
  findings: snapshot.findings.length,
}));
