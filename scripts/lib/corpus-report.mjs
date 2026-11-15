// Reading and writing a corpus sweep report without ever holding it as one string.
//
// A sweep report is one JSON object: a few small members (digests, configuration, counts) and `results`, one element per
// project. With a whole corpus the document no longer fits V8's longest string (`buffer.constants.MAX_STRING_LENGTH`, about
// 512 MB of characters), so `JSON.stringify(summary)` throws `RangeError: Invalid string length` after every project has run,
// and `JSON.parse(readFileSync(path, "utf8"))` fails the same way on the file. Both ends go through this module instead:
//
//   writeReportFile(path, head, results)   the same bytes as `JSON.stringify({ ...head, results }, null, 2) + "\n"`, written one
//                                          project at a time
//   readReportFile(path, { onResult })     the same object as `JSON.parse(readFileSync(path, "utf8"))`, read in chunks; with
//                                          `onResult` each element of `results` is handed to it and not kept, so a reader that
//                                          needs only a few fields per project (`--order-from`) holds none of the findings
//
// The file format is unchanged, so a report written here is read by `JSON.parse` too while it fits a string, and the reports
// written before are read here. A single project's element must still fit a string (one checker run's output does).
import { closeSync, openSync, readSync, writeSync } from "node:fs";

const CHUNK = 1 << 24;
const QUOTE = 0x22;
const BACKSLASH = 0x5c;
const OPEN_BRACE = 0x7b;
const CLOSE_BRACE = 0x7d;
const OPEN_BRACKET = 0x5b;
const CLOSE_BRACKET = 0x5d;
const COMMA = 0x2c;
const COLON = 0x3a;
const isSpace = (byte) => byte === 0x20 || byte === 0x0a || byte === 0x0d || byte === 0x09;

/**
 * Writes `{ ...head, results }` the way `JSON.stringify(value, null, 2)` followed by a newline would, without building the whole text:
 * the head, then each element of `results`, are written separately. `head` is an object that has no `results` of its own.
 */
export function writeReportFile(path, head, results) {
  if ("results" in head) throw new Error("writeReportFile: the head carries `results`; pass the results separately");
  const fd = openSync(path, "w");
  try {
    const write = (text) => { writeSync(fd, text); };
    const headText = JSON.stringify(head, null, 2);
    const inner = headText === "{}" ? "" : headText.slice(2, -2);
    write(`{\n${inner ? `${inner},\n` : ""}  "results": `);
    if (results.length === 0) write("[]");
    else {
      write("[\n");
      results.forEach((result, index) => {
        write(`${index === 0 ? "" : ",\n"}    ${JSON.stringify(result, null, 2).replace(/\n/g, "\n    ")}`);
      });
      write("\n  ]");
    }
    write("\n}\n");
  } finally {
    closeSync(fd);
  }
}

/** Reads a file by chunk: the byte under the cursor, and the raw bytes of one JSON value. */
class Cursor {
  constructor(fd, size) {
    this.fd = fd;
    this.size = size;
    this.chunk = Buffer.allocUnsafe(size);
    this.buffer = this.chunk.subarray(0, 0);
    this.position = 0;
    this.done = false;
  }

  /** The next chunk, or false at the end of the file. */
  fill() {
    if (this.done) return false;
    const read = readSync(this.fd, this.chunk, 0, this.size, null);
    if (read === 0) {
      this.done = true;
      return false;
    }
    this.buffer = this.chunk.subarray(0, read);
    this.position = 0;
    return true;
  }

  /** The byte under the cursor (not consumed), or -1 at the end. */
  peek() {
    while (this.position >= this.buffer.length) if (!this.fill()) return -1;
    return this.buffer[this.position];
  }

  next() {
    const byte = this.peek();
    if (byte !== -1) this.position += 1;
    return byte;
  }

  skipSpace() {
    while (isSpace(this.peek())) this.position += 1;
  }

  expect(byte, what) {
    this.skipSpace();
    const found = this.next();
    if (found !== byte) throw new SyntaxError(`report: expected ${what}, found ${found === -1 ? "the end of the file" : `"${String.fromCharCode(found)}"`}`);
  }

  /**
   * The raw bytes of the next value (a string, an object, an array or a scalar), consumed. Brackets and quotes are ASCII and never part of
   * a multi-byte UTF-8 sequence, so the scan is by byte.
   */
  value() {
    this.skipSpace();
    const parts = [];
    let from = this.position;
    let depth = 0;
    let inString = false;
    let escaped = false;
    let started = false;
    let scalar = false;
    const keep = (end) => { if (end > from) parts.push(Buffer.from(this.buffer.subarray(from, end))); };
    for (;;) {
      if (this.position >= this.buffer.length) {
        keep(this.buffer.length);
        if (!this.fill()) { from = this.position; break; }
        from = 0;
      }
      const byte = this.buffer[this.position];
      if (inString) {
        this.position += 1;
        if (escaped) escaped = false;
        else if (byte === BACKSLASH) escaped = true;
        else if (byte === QUOTE) {
          inString = false;
          if (depth === 0) break;
        }
        continue;
      }
      if (!started) {
        started = true;
        if (byte === QUOTE) { inString = true; this.position += 1; continue; }
        if (byte === OPEN_BRACE || byte === OPEN_BRACKET) { depth = 1; this.position += 1; continue; }
        scalar = true;
        this.position += 1;
        continue;
      }
      if (scalar) {
        if (byte === COMMA || byte === CLOSE_BRACE || byte === CLOSE_BRACKET || isSpace(byte)) break;
        this.position += 1;
        continue;
      }
      this.position += 1;
      if (byte === QUOTE) inString = true;
      else if (byte === OPEN_BRACE || byte === OPEN_BRACKET) depth += 1;
      else if (byte === CLOSE_BRACE || byte === CLOSE_BRACKET) {
        depth -= 1;
        if (depth === 0) break;
      }
    }
    keep(this.position);
    if (!started) throw new SyntaxError("report: a value was expected, found the end of the file");
    return parts.length === 1 ? parts[0] : Buffer.concat(parts);
  }
}

const parse = (bytes) => JSON.parse(bytes.toString("utf8"));

/**
 * The report at `path`, as `JSON.parse(readFileSync(path, "utf8"))` returns it. With `onResult`, the elements of `results` are passed to it one
 * by one and left out of the returned object (the other members are returned whole). Accepts any JSON object at the top: a member named
 * `results` that is an array is read element by element, every other member is read whole. `chunkSize` (bytes read at a time) is for tests.
 */
export function readReportFile(path, { onResult, chunkSize = CHUNK } = {}) {
  const fd = openSync(path, "r");
  try {
    const cursor = new Cursor(fd, chunkSize);
    const report = {};
    cursor.expect(OPEN_BRACE, "an object");
    for (;;) {
      cursor.skipSpace();
      const lead = cursor.peek();
      if (lead === CLOSE_BRACE) { cursor.next(); break; }
      if (lead === COMMA) { cursor.next(); continue; }
      if (lead !== QUOTE) throw new SyntaxError(`report: expected a member name, found ${lead === -1 ? "the end of the file" : `"${String.fromCharCode(lead)}"`}`);
      const key = parse(cursor.value());
      cursor.expect(COLON, '":"');
      cursor.skipSpace();
      if (key === "results" && cursor.peek() === OPEN_BRACKET) {
        cursor.next();
        const results = [];
        for (;;) {
          cursor.skipSpace();
          const element = cursor.peek();
          if (element === CLOSE_BRACKET) { cursor.next(); break; }
          if (element === COMMA) { cursor.next(); continue; }
          if (element === -1) throw new SyntaxError("report: the file ends inside `results`");
          const row = parse(cursor.value());
          if (onResult) onResult(row);
          else results.push(row);
        }
        if (!onResult) report.results = results;
      } else {
        report[key] = parse(cursor.value());
      }
    }
    return report;
  } finally {
    closeSync(fd);
  }
}
