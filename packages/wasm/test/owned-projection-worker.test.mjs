// The project projection through the worker and the shipped reactors (P5
// S3c): the checker asks only after WASI failed with NotFound, an outside
// link into the root is answered as the canonical request is, an injected
// answer is encoded against the store's root, and a check that leaves an
// answer pending poisons the checker. Faults and answers enter only through
// the internal adapters, which nothing a caller passes can reach.
import assert from "node:assert/strict";
import * as nodeFs from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { MessageChannel } from "node:worker_threads";
import { afterAll, test } from "vitest";
import { loadArtifacts } from "../owned-artifacts.mjs";
import { mountDirectory, validateMount } from "../owned-mount.mjs";
import { identityOf } from "../owned-snapshot.mjs";
import { serve } from "../owned-worker-core.mjs";

const top = nodeFs.realpathSync(nodeFs.mkdtempSync(join(tmpdir(), "owned-projection-worker-")));
const root = join(top, "project");
nodeFs.mkdirSync(root);
nodeFs.writeFileSync(join(root, "tsconfig.json"), '{ "compilerOptions": { "strict": true, "noEmit": true }, "files": ["App.ts"] }\n');
nodeFs.writeFileSync(join(root, "App.ts"), "export const answer = 42;\n");
nodeFs.symlinkSync("project", join(top, "alias"));
afterAll(() => nodeFs.rmSync(top, { recursive: true, force: true }));

const directory = () => ({ input: root, root, identity: identityOf(nodeFs.lstatSync(root, { bigint: true })) });

async function started(adapters, init = { directory: directory() }) {
  const { checker, producer } = await loadArtifacts();
  const { port1, port2 } = new MessageChannel();
  const messages = [];
  let wake;
  port1.on("message", message => {
    messages.push(message);
    wake?.();
  });
  serve(port2, adapters);
  const next = () => new Promise(resolve => {
    const seen = messages.length;
    wake = () => messages.length > seen && resolve(messages.at(-1));
  });
  const ready = next();
  port1.postMessage({ type: "init", checker, producer, ...init });
  assert.equal((await ready).type, "ready");
  let id = 0;
  const check = projectId => {
    const answered = next();
    port1.postMessage({ type: "check", id: ++id, requestJson: JSON.stringify({ projectId }) });
    return answered;
  };
  return { check, close: () => { port1.close(); port2.close(); } };
}

const counting = () => {
  const asked = [];
  return { asked, adapters: { projection: (spelling, compute) => { asked.push(spelling); return compute(); } } };
};
const error = answer => JSON.parse(answer.output).error;

test("the checker asks only after NotFound, and keeps the original error when nothing was outside", async () => {
  const { asked, adapters } = counting();
  const worker = await started(adapters);
  const canonical = await worker.check(`${root}/tsconfig.json`);
  assert.equal(canonical.type, "result");
  assert.equal(canonical.status, 0);
  const notDirectory = await worker.check(`${root}/App.ts/tsconfig.json`);
  assert.equal(notDirectory.status, 1);
  assert.match(error(notDirectory), /os error 54/);
  assert.deepEqual(asked, []);
  const missing = await worker.check(`${root}/absent/tsconfig.json`);
  assert.equal(error(missing), "No such file or directory (os error 44)");
  assert.deepEqual(asked, [`${root}/absent/tsconfig.json`]);
  worker.close();
});

test("an outside link into the root is answered exactly as the canonical spelling", async () => {
  const { asked, adapters } = counting();
  const worker = await started(adapters);
  const canonical = await worker.check(`${root}/tsconfig.json`);
  const aliased = await worker.check(`${top}/alias/tsconfig.json`);
  assert.equal(aliased.type, "result");
  assert.equal(aliased.output, canonical.output);
  // The same producer traffic; only the program handle (one per check) differs.
  const traffic = transport => ({ calls: transport.calls, exchanges: transport.exchanges, pendingResults: transport.pendingResults });
  assert.deepEqual(traffic(aliased.transport), traffic(canonical.transport));
  assert.deepEqual(asked, [`${top}/alias/tsconfig.json`]);
  const dotted = await worker.check(`${top}/alias/../project/tsconfig.json`);
  assert.equal(dotted.output, canonical.output);
  worker.close();
});

test("each refusal reaches the request as its own message", async () => {
  const answers = {
    [`${top}/a1`]: { tag: "listing-unavailable", errno: 2, directory: root },
    [`${top}/a2`]: { tag: "name-variant", path: `${root}/TSCONFIG.json` },
    [`${top}/a3`]: { tag: "outside-root", path: `${top}/other`, root },
    [`${top}/a4`]: { tag: "host-error", errno: 54 },
    [`${top}/a5`]: { tag: "faulted" },
  };
  const worker = await started({ projection: (spelling, compute) => answers[spelling] ?? compute() });
  const listing = error(await worker.check(`${top}/a1`));
  assert.match(listing, new RegExp(`^project ${top}/a1: listing ${root} to confirm its spelling failed: .*\\(os error 2\\); the spelling is unconfirmed, not absent$`));
  assert.equal(error(await worker.check(`${top}/a2`)),
    `project ${top}/a2: ${root}/TSCONFIG.json is not spelled as its directory entry; name-insensitive aliases are not resolved`);
  assert.equal(error(await worker.check(`${top}/a3`)), `project ${top}/a3 resolves on the host to ${top}/other, outside the directory root ${root}`);
  assert.match(error(await worker.check(`${top}/a4`)), /\(os error 54\)$/);
  assert.equal(error(await worker.check(`${top}/a5`)), "the owned checker's input faulted during project projection");
  worker.close();
});

test("an injected answer the store could not give is refused against its root, and poisons", async () => {
  const worker = await started({ projection: () => ({ tag: "listing-unavailable", errno: 2, directory: `${top}/elsewhere` }) });
  const answer = await worker.check(`${top}/x/tsconfig.json`);
  assert.equal(answer.type, "poison");
  assert.match(answer.reason, /OwnedProjectionError: .*the listed directory is not the root or below it/);
  worker.close();
});

test("a check that leaves an answer pending poisons, and nothing is answered after", async () => {
  const worker = await started({ projectionImports: imports => ({ ...imports, project_copy: () => 1 }) });
  const answer = await worker.check(`${top}/alias/tsconfig.json`);
  assert.equal(answer.type, "poison");
  assert.equal(answer.reason, "the check left a project projection answer behind");
  // A poisoned worker answers nothing more.
  const later = await Promise.race([worker.check(`${root}/tsconfig.json`), new Promise(resolve => setTimeout(() => resolve("silent"), 500))]);
  assert.equal(later, "silent");
  worker.close();
});

test("a mount has no host to ask: an outside spelling keeps the original error", async () => {
  const { asked, adapters } = counting();
  const mount = validateMount(mountDirectory(root));
  const worker = await started(adapters, { mount });
  assert.equal(error(await worker.check(`${top}/alias/tsconfig.json`)), "No such file or directory (os error 44)");
  assert.equal((await worker.check(`${root}/tsconfig.json`)).status, 0);
  assert.deepEqual(asked, []);
  worker.close();
});
