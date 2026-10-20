// The one byte transport between the checker reactor and the Type Facts
// producer reactor (P5 S2).
//
// The checker imports six functions from `solid_typefacts`; this closure
// answers them from the producer's exports, copying bytes between the two
// linear memories during each call. Every producer output is copied out of
// the producer's memory before it is retained, so nothing later written
// there can change an answer the checker has not yet read. Lifecycle bytes
// are never parsed here. There is no callback, replacement or observation
// path: the only read-out is `stats()`, which returns fresh arrays and
// numbers.
//
// Addresses and lengths are u32 on both sides (P5 S3b): the producer's
// exported pointers and lengths are normalized as the u32 they encode and
// must lie inside its memory, so an address at or above 2 GiB is neither
// negative nor clipped. The checker's arguments arrive normalized by the
// owned boundary (`protocolImports`) and are range-checked here.
import { u32 } from "./owned-boundary.mjs";

const textDecoder = new TextDecoder();

function producerBytes(producer, pointer, length, what) {
  const at = u32(pointer, `producer ${what} pointer`);
  const size = u32(length, `producer ${what} length`);
  if (at + size > producer.memory.buffer.byteLength) {
    throw new RangeError(`producer ${what} [${at}, ${at + size}) is outside its memory`);
  }
  return new Uint8Array(producer.memory.buffer, at, size);
}

/**
 * @param producer the producer instance's exports: `memory`, `allocate_input`,
 *   `output_pointer`, `output_length`, `typefacts_handshake`, `typefacts_open`,
 *   `typefacts_lifecycle`, `typefacts_close`.
 */
export function createTransport(producer) {
  let checkerMemory;
  const results = new Map();
  const live = new Set();
  let opened = [];
  let closed = [];
  let calls = [];
  let exchanges = 0;

  const checkerBytes = () => new Uint8Array(checkerMemory.buffer);
  const inChecker = (pointer, length) =>
    Number.isInteger(pointer) && Number.isInteger(length) && pointer >= 0 && length >= 0 &&
    pointer + length <= checkerMemory.buffer.byteLength;
  const readChecker = (pointer, length) => {
    if (!inChecker(pointer, length)) throw new RangeError("checker pointer out of bounds");
    return checkerBytes().slice(pointer, pointer + length);
  };
  const call = (operation, input, ...rest) => {
    if (input) {
      producerBytes(producer, producer.allocate_input(input.length), input.length, "input").set(input);
    }
    const status = producer[operation](...rest);
    const output = producerBytes(producer, producer.output_pointer(), producer.output_length(), "output").slice();
    return { status, output };
  };
  const hold = (handle, status, output) => {
    results.set(handle, output);
    return status;
  };
  const record = (op, status) => {
    calls.push({ op, status });
    return status;
  };

  const imports = Object.freeze({
    open(pointer, length, handleOut) {
      const { status, output } = call("typefacts_open", readChecker(pointer, length));
      record("open", status);
      if (status !== 0) return hold(0, status, output);
      if (output.length !== 4) {
        throw new Error(`producer opened a program with a ${output.length}-byte handle: ${textDecoder.decode(output)}`);
      }
      const handle = new DataView(output.buffer).getUint32(0, true);
      opened.push(handle);
      live.add(handle);
      if (!inChecker(handleOut, 4)) throw new RangeError("handle pointer out of bounds");
      new DataView(checkerMemory.buffer).setUint32(handleOut, handle, true);
      return 0;
    },
    handshake(handle) {
      const { status, output } = call("typefacts_handshake");
      return hold(handle, record("handshake", status), output);
    },
    exchange(handle, pointer, length) {
      const request = readChecker(pointer, length);
      exchanges++;
      const { status, output } = call("typefacts_lifecycle", request, handle);
      return hold(handle, record("exchange", status), output);
    },
    result_length(handle) {
      return results.get(handle)?.length ?? 0;
    },
    copy_result(handle, destination, capacity) {
      const result = results.get(handle);
      results.delete(handle);
      if (!result || result.length > capacity || !inChecker(destination, result.length)) return 1;
      checkerBytes().set(result, destination);
      return 0;
    },
    close(handle) {
      const { status } = call("typefacts_close", undefined, handle);
      results.delete(handle);
      closed.push(handle);
      if (status === 0) live.delete(handle);
      return record("close", status);
    },
  });

  return Object.freeze({
    imports,
    /** Binds the checker instance's memory, once. */
    bind(memory) {
      if (checkerMemory) throw new Error("transport is already bound to a checker");
      checkerMemory = memory;
    },
    /** Starts one check's counts. */
    begin() {
      opened = [];
      closed = [];
      calls = [];
      exchanges = 0;
    },
    /** This check's counts and what is still held, as fresh values. */
    stats() {
      return {
        opened: [...opened],
        closed: [...closed],
        exchanges,
        calls: calls.map(entry => ({ ...entry })),
        pendingResults: [...results.keys()],
        livePrograms: [...live],
      };
    },
  });
}
