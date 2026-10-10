// Authored diagnostic control, never automatic-generation or proof authority.
// Test whether conservative accessor bounds are the remaining node gate wall.
import assert from "node:assert/strict";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { consumerState } from "../../scripts/contract-coverage-census.mjs";
import { withheldClosuresFromNativeOutput, withheldOperationsFromNativeOutput } from "../../packages/cli/scripts/certify-contract.mjs";

const read = path => JSON.parse(readFileSync(path));
const write = (path, value) => writeFileSync(path, JSON.stringify(value, null, 2) + "\n");
const original = resolve(process.argv[2]), out = resolve(process.argv[3]);
const scope = process.argv[4] ?? "target";
assert(["target", "all"].includes(scope));
assert(!existsSync(out), "Choose a fresh directory; preserve earlier evidence");
assert(process.env.SOLID_CHECKER_NATIVE_BIN && existsSync(process.env.SOLID_CHECKER_NATIVE_BIN));
mkdirSync(out, { recursive: true });
const request = read(original), candidate = read(request.graph.root.planning.proposal);
assert.equal(candidate.package.name, "@solid-primitives/media");
assert.equal(candidate.package.version, "4.0.0-next.2");
assert.deepEqual(request.graph.root.planning.exportConditions, ["import", "node"]);
const ref = candidate.entrypoints["."].cases[0].exports.makeMediaQueryListener;
const summary = candidate.summaries[typeof ref === "string" ? ref : ref.summary];
assert(summary.call.accessorBounds?.length > 0);
const removedBounds = scope === "target"
  ? [{ summary: ref, bounds: summary.call.accessorBounds }]
  : Object.entries(candidate.summaries).filter(([, value]) => value.call?.accessorBounds?.length)
    .map(([key, value]) => ({ summary: key, bounds: value.call.accessorBounds }));
for (const item of removedBounds) delete candidate.summaries[item.summary].call.accessorBounds;
const proposal = join(out, "proposal.json"); write(proposal, candidate);
request.graph.root.planning.proposal = proposal;
request.catalogRoot = join(out, "accepted");
request.trustConfigurationOutput = join(out, "trust.json");
write(join(out, "execution.json"), request);
const process_ = Bun.spawn([process.env.SOLID_CHECKER_NATIVE_BIN, "--execute-contract-certification", join(out, "execution.json")],
  { stdout: "pipe", stderr: "pipe", env: { ...process.env, SOLID_CHECKER_DAEMON: "0" } });
const [status, stdout, stderr] = await Promise.all([process_.exited, new Response(process_.stdout).text(), new Response(process_.stderr).text()]);
const document = { authority: false, kind: "media-accessor-bounds-diagnostic", authoredProposals: 1,
  original, scope, removedBounds, native: { status, stdout, stderr } };
if (status === 0) {
  const pointer = read(join(request.catalogRoot, "accepted-contracts.json"));
  const accepted = read(join(request.catalogRoot, pointer.contracts[0].document));
  document.surface = Object.entries(accepted.entrypoints["."].cases[0].exports).map(([name, reference]) =>
    ({ export: name, state: consumerState(accepted, reference) }));
  document.withheldClosures = withheldClosuresFromNativeOutput(stdout);
  document.withheldOperations = withheldOperationsFromNativeOutput(stdout);
}
write(join(out, "results.json"), document);
console.log(JSON.stringify({ kind: document.kind, status, removedBounds, surface: document.surface,
  targetWithheld: document.withheldClosures?.filter(item => item.node?.package === candidate.package.name && item.export === "makeMediaQueryListener") }));
assert.equal(status, 0, stderr);
