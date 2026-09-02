// The published build of the source-condition entrypoint. Consumers that do
// not opt into the private condition reach exactly this file.
export function track(callback) {
  callback();
}
