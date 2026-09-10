// Diagnostic host simulation only. Never produces receipts or acceptance data.
// The published modules are not rewritten; EventSource cannot access a network.
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { pathToFileURL } from 'node:url';
import { join } from 'node:path';

const root = process.argv[2];
const mode = process.argv[3] ?? 'simulated-worker';
assert.ok(root, 'pass one exact retained package root');
assert.ok(['simulated-worker', 'absent-host'].includes(mode));
const manifest = JSON.parse(readFileSync(join(root, 'package.json')));
assert.equal(manifest.name, '@solid-primitives/sse');
const digest = file => 'sha256:' + createHash('sha256').update(readFileSync(file)).digest('hex');
const runtime = join(root, 'dist/worker-handler.js');
const sources = ['package.json', 'dist/worker-handler.js', 'dist/worker-handler.d.ts', 'dist/sse.js']
  .map(path => ({ path, digest: digest(join(root, path)) }));
const connections = [];
const trace = [];
class Channel {
  constructor(name) { this.name = name; this.listeners = new Map(); this.messages = []; }
  addEventListener(type, listener) {
    assert.equal(typeof listener, 'function');
    const list = this.listeners.get(type) ?? [];
    list.push(listener); this.listeners.set(type, list);
    trace.push({ operation: 'register', channel: this.name, type });
  }
  removeEventListener(type, listener) {
    this.listeners.set(type, (this.listeners.get(type) ?? []).filter(x => x !== listener));
  }
  dispatch(type, event) { for (const listener of [...(this.listeners.get(type) ?? [])]) listener(event); }
  postMessage(message) { this.messages.push(structuredClone(message)); }
  start() { trace.push({ operation: 'start', channel: this.name }); }
}
class SimulatedEventSource extends Channel {
  constructor(url, options) {
    super('event-source-' + connections.length);
    this.url = url; this.options = options; this.readyState = 0; this.closed = false;
    connections.push(this);
  }
  close() { this.closed = true; this.readyState = 2; }
}
// These objects are declared simulation assumptions, not authenticated Web APIs.
globalThis.EventSource = SimulatedEventSource;
const host = new Channel('self');
if (mode === 'simulated-worker') globalThis.self = host;
else delete globalThis.self;
let loadError;
try { await import(pathToFileURL(runtime).href); } catch (error) { loadError = error; }
if (mode === 'absent-host') {
  assert.ok(loadError instanceof ReferenceError);
  assert.match(loadError.message, /self/);
  console.log(JSON.stringify({ diagnostic: true, certificationEvidence: false, mode,
    package: manifest.name, version: manifest.version, sources,
    loadError: { name: loadError.name, message: loadError.message } }));
} else {
  assert.equal(loadError, undefined);
  assert.deepEqual([...host.listeners.keys()].sort(), ['connect', 'message']);
  assert.equal(connections.length, 0, 'registration must not open a connection');
  const connect = id => ({ type: 'connect', id, url: 'https://example.invalid/events', events: ['custom'] });
  host.dispatch('message', { data: connect('dedicated') });
  assert.equal(connections.length, 1);
  connections[0].dispatch('message', { data: 'payload', target: connections[0] });
  assert.deepEqual(host.messages, [{ type: 'message', id: 'dedicated', data: 'payload', eventType: 'message' }]);
  host.dispatch('message', { data: { type: 'disconnect', id: 'dedicated' } });
  assert.equal(connections[0].closed, true);
  assert.ok([...connections[0].listeners.values()].every(x => x.length === 0));
  host.dispatch('connect', { ports: [] });
  const a = new Channel('port-a'), b = new Channel('port-b');
  host.dispatch('connect', { ports: [a] }); host.dispatch('connect', { ports: [b] });
  a.dispatch('message', { data: connect('shared') });
  connections[1].dispatch('custom', { data: 'custom-payload', target: connections[1] });
  assert.deepEqual(a.messages, [{ type: 'message', id: 'shared', data: 'custom-payload', eventType: 'custom' }]);
  assert.deepEqual(b.messages, []);
  // Pin observed limits: connection ownership is keyed by id, not channel.
  b.dispatch('message', { data: { type: 'disconnect', id: 'shared' } });
  assert.equal(connections[1].closed, true);
  a.dispatch('message', { data: connect('duplicate') });
  a.dispatch('message', { data: connect('duplicate') });
  a.dispatch('message', { data: { type: 'disconnect', id: 'duplicate' } });
  assert.equal(connections[2].closed, false);
  assert.equal(connections[3].closed, true);
  connections[2].close();
  console.log(JSON.stringify({ diagnostic: true, certificationEvidence: false, mode,
    package: manifest.name, version: manifest.version, sources, trace,
    observations: { registrationOnlyAtLoad: true, dedicatedRoundTrip: true,
      sharedResponsesReturnToOrigin: true, missingPortIgnored: true,
      disconnectCanCrossPorts: true, duplicateIdLeavesPriorConnectionOpen: true },
    limitations: ['Simulated host; no browser or worker-host attestation',
      'Node dependency resolution is not a worker dependency-selection proof',
      'Finite scenarios do not prove all callback executions',
      'No receipt, ordinary applicability or coverage claim'] }));
}
