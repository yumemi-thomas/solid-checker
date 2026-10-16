// The worker's precedence in directory mode (P5 S3b): a store fault decides
// the check before the guest's own outcome -- an answer, a trap or a leaked
// program -- and is reported with the store's own reason; a store that
// cannot start fails initialization. Drives `serve` over a message channel
// with the shipped reactors; faults enter only through the internal
// adapters, which nothing a caller passes can reach.
import assert from "node:assert/strict";
import * as nodeFs from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { MessageChannel } from "node:worker_threads";
import { afterAll, test } from "vitest";
import { loadArtifacts } from "../owned-artifacts.mjs";
import { hostSource, identityOf } from "../owned-snapshot.mjs";
import { serve } from "../owned-worker-core.mjs";

const top = nodeFs.realpathSync(nodeFs.mkdtempSync(join(tmpdir(), "owned-worker-")));
const root = join(top, "project");
nodeFs.mkdirSync(root);
nodeFs.writeFileSync(join(root, "tsconfig.json"), '{ "compilerOptions": { "strict": true, "noEmit": true }, "files": ["App.ts"] }\n');
nodeFs.writeFileSync(join(root, "App.ts"), "export const answer = 42;\n");
afterAll(() => nodeFs.rmSync(top, { recursive: true, force: true }));

const directory = () => ({ input: root, root, identity: identityOf(nodeFs.lstatSync(root, { bigint: true })) });
const request = JSON.stringify({ projectId: `${root}/tsconfig.json` });

async function started(adapters, source = directory()) {
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
  port1.postMessage({ type: "init", checker, producer, directory: source });
  const first = await ready;
  const check = id => {
    const answered = next();
    port1.postMessage({ type: "check", id, requestJson: request });
    return answered;
  };
  return { first, check, close: () => { port1.close(); port2.close(); } };
}

test("a clean directory check answers", async () => {
  const worker = await started({});
  assert.equal(worker.first.type, "ready");
  const answer = await worker.check(1);
  assert.equal(answer.type, "result");
  assert.equal(answer.status, 0);
  worker.close();
});

test("a host where / can be searched but not read: both runtimes start and the check answers as on a readable /", async () => {
  const denied = code => Object.assign(new Error(code), { code });
  const searchOnly = {
    ...hostSource,
    open: path => { if (path === "/") throw denied("EACCES"); return hostSource.open(path); },
    readdir: path => { if (path === "/") throw denied("EACCES"); return hostSource.readdir(path); },
  };
  let store;
  const plain = await started({});
  const expected = await plain.check(1);
  plain.close();
  const worker = await started({ source: searchOnly, store: directory => { store = directory; } });
  assert.equal(worker.first.type, "ready");
  const answer = await worker.check(1);
  assert.equal(answer.type, "result");
  assert.equal(answer.output, expected.output);
  assert.equal(store.anchors(), 2);
  assert.equal(store.observe("F", "/"), undefined);
  worker.close();
});

test("a store fault wins over the guest's answer, its trap and a leaked program", async () => {
  let store;
  // The fault is raised and swallowed at the producer close, after every
  // read: the guest goes on to its own outcome.
  const faultAtClose = close => handle => {
    assert.throws(() => store.fs.writeSync(3), /read-only/);
    return close(handle);
  };
  for (const [name, extra] of [
    ["answer", { imports: imports => ({ ...imports, close: faultAtClose(imports.close) }) }],
    ["trap", { imports: imports => ({ ...imports, close: handle => { faultAtClose(() => 0)(handle); throw new Error("forced trap"); } }) }],
    ["ledger", { imports: imports => ({ ...imports, close: faultAtClose(() => 0) }) }],
  ]) {
    const worker = await started({ ...extra, store: value => { store = value; } });
    assert.equal(worker.first.type, "ready", name);
    const answer = await worker.check(1);
    assert.equal(answer.type, "poison", name);
    assert.match(answer.reason, /^the owned checker's input faulted: writeSync: the owned directory is read-only$/, name);
    worker.close();
  }
});

test("without a store fault the guest's own failure is reported as before", async () => {
  const worker = await started({ producer: exports => ({ ...exports, typefacts_open: () => { throw new Error("forced trap"); } }) });
  const answer = await worker.check(1);
  assert.equal(answer.type, "poison");
  assert.match(answer.reason, /^the check trapped: Error: forced trap$/);
  worker.close();
});

test("input changed during a check is caught by validation even though the guest answers from what it read", async () => {
  const app = join(root, "App.ts");
  let changed = false;
  const worker = await started({ imports: imports => ({ ...imports, close: handle => {
    // Every read the guest makes from here on is answered from records, so
    // it reaches its own answer; only validation sees the change.
    if (!changed) nodeFs.writeFileSync(app, "export const answer = 4200;\n");
    changed = true;
    return imports.close(handle);
  } }) });
  const answer = await worker.check(1);
  nodeFs.writeFileSync(app, "export const answer = 42;\n");
  assert.equal(answer.type, "poison");
  assert.match(answer.reason, /App\.ts: changed since it was read/);
  worker.close();
});

test("a store that cannot start fails initialization with its reason", async () => {
  const worker = await started({}, { ...directory(), identity: "0,0,0,0,0,0,0" });
  assert.equal(worker.first.type, "init-failed");
  assert.match(worker.first.error, /OwnedInputFault: .*is not the directory the checker was created for/);
  worker.close();
});
