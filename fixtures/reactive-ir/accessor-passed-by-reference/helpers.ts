// Calls its parameter in its own synchronous body, from another module.
export function readNow(read: () => number) {
  return read() + 1;
}
