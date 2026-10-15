// CBOR frame helpers the parity gate's negative controls and update replay
// use to forge or compare producer answers. Moved from the S1 owned-check
// prototype; never used by the package's transport, which parses nothing.

export function frame(payload) {
  const bytes = Buffer.alloc(4 + payload.length);
  bytes.writeUInt32LE(payload.length, 0);
  payload.copy(bytes, 4);
  return bytes;
}

// cborEnd returns the offset just past the one data item at `offset`.
function cborEnd(bytes, offset) {
  const initial = bytes[offset];
  const major = initial >> 5;
  const info = initial & 0x1f;
  let at = offset + 1;
  let argument = info;
  if (info === 24) argument = bytes[at++];
  else if (info === 25) { argument = bytes.readUInt16BE(at); at += 2; }
  else if (info === 26) { argument = bytes.readUInt32BE(at); at += 4; }
  else if (info === 27) { argument = Number(bytes.readBigUInt64BE(at)); at += 8; }
  else if (info > 27) throw new Error(`indefinite or reserved CBOR item at ${offset}`);
  switch (major) {
    case 0: case 1: case 7: return at;
    case 2: case 3: return at + argument;
    case 4: for (let index = 0; index < argument; index++) at = cborEnd(bytes, at); return at;
    case 5: for (let index = 0; index < 2 * argument; index++) at = cborEnd(bytes, at); return at;
    default: return cborEnd(bytes, at);
  }
}
export function cborHead(major, value) {
  if (value < 24) return Buffer.from([(major << 5) | value]);
  if (value < 0x100) return Buffer.from([(major << 5) | 24, value]);
  if (value < 0x10000) { const b = Buffer.alloc(3); b[0] = (major << 5) | 25; b.writeUInt16BE(value, 1); return b; }
  if (value < 0x100000000) { const b = Buffer.alloc(5); b[0] = (major << 5) | 26; b.writeUInt32BE(value, 1); return b; }
  const b = Buffer.alloc(9); b[0] = (major << 5) | 27; b.writeBigUInt64BE(BigInt(value), 1); return b;
}
const cborText = text => Buffer.concat([cborHead(3, Buffer.byteLength(text)), Buffer.from(text)]);
function decodeText(item) {
  const info = item[0] & 0x1f;
  const skip = info < 24 ? 1 : 1 + ({ 24: 1, 25: 2, 26: 4, 27: 8 }[info]);
  return item.subarray(skip).toString("utf8");
}
function decodeUint(item) {
  const info = item[0] & 0x1f;
  if (info < 24) return info;
  if (info === 24) return item[1];
  if (info === 25) return item.readUInt16BE(1);
  if (info === 26) return item.readUInt32BE(1);
  return Number(item.readBigUInt64BE(1));
}
// topLevel splits one response payload's root map into raw entries, without
// re-encoding any value; rejoining them unchanged reproduces the payload.
export function topLevel(payload) {
  const initial = payload[0];
  if (initial >> 5 !== 5) throw new Error("response payload is not a map");
  const headEnd = 1 + ({ 24: 1, 25: 2, 26: 4, 27: 8 }[initial & 0x1f] ?? 0);
  const entries = [];
  let at = headEnd;
  while (at < payload.length) {
    const keyEnd = cborEnd(payload, at);
    const valueEnd = cborEnd(payload, keyEnd);
    const keyBytes = payload.subarray(at, keyEnd);
    entries.push({ keyBytes, key: decodeText(keyBytes), value: payload.subarray(keyEnd, valueEnd) });
    at = valueEnd;
  }
  return { head: payload.subarray(0, headEnd), entries };
}
// rewrite replaces one root value of a response frame, leaving every other
// byte as the producer wrote it. Undefined when the key is absent.
function rewrite(responseFrame, key, change) {
  const payload = responseFrame.subarray(4);
  const { head, entries } = topLevel(payload);
  const rejoined = Buffer.concat([head, ...entries.flatMap(entry => [entry.keyBytes, entry.value])]);
  if (!rejoined.equals(payload)) throw new Error("response map did not split losslessly");
  const entry = entries.find(candidate => candidate.key === key);
  if (!entry) return undefined;
  entry.value = change(entry.value);
  return frame(Buffer.concat([head, ...entries.flatMap(item => [item.keyBytes, item.value])]));
}

// The S1 forged answers, in the witness's order. `when` selects the exchange
// (counted from 1 within one check); `apply` returns the forged frame.
export const FAULTS = {
  truncated: { when: index => index === 2, apply: response => response.subarray(0, response.length - 1) },
  trailing: { when: index => index === 2, apply: response => Buffer.concat([response, Buffer.from([0])]) },
  garbage: { when: index => index === 2, apply: () => frame(Buffer.alloc(8, 0xff)) },
  "wrong-request": {
    when: index => index === 2,
    apply: response => rewrite(response, "requestId", value => cborHead(0, decodeUint(value) + 1)),
  },
  "wrong-generation": {
    when: index => index === 2,
    apply: response => rewrite(response, "generation", value => cborHead(0, decodeUint(value) + 1)),
  },
  "wrong-project": {
    when: index => index === 2,
    apply: response => rewrite(response, "projectId", value => cborText(`${decodeText(value)}x`)),
  },
  // The first answer that carries a state token; it is checked when it is
  // next presented.
  "wrong-token": {
    when: (index, response) => topLevel(response.subarray(4)).entries.some(entry => entry.key === "stateToken"),
    apply: response => rewrite(response, "stateToken", value => cborText(`${decodeText(value)}x`)),
  },
  // The exchange is answered by a different program the same producer owns.
  "wrong-program": { when: index => index === 2, route: "decoy" },
};

// The open envelope, encoded exactly as typefacts::owned_open_envelope writes
// it for a program without a configuration overlay.
export function cborOpenEnvelope(projectId) {
  return Buffer.concat([cborHead(5, 1), cborText("projectId"), cborText(projectId)]);
}

// Analyze answers carry the producer's own wall-clock timings, which no two
// runs share; everything else in every answer must be byte-identical.
export function withoutTimings(response) {
  const { entries } = topLevel(response.subarray(4));
  const kept = entries.filter(entry => entry.key !== "timings");
  if (kept.length === entries.length) return response;
  return frame(Buffer.concat([Buffer.from(cborHead(5, kept.length)), ...kept.flatMap(entry => [entry.keyBytes, entry.value])]));
}
