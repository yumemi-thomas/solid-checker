// The published default build of the browser-gap entrypoint. Its `browser`
// sibling names a file the artifact does not contain, which stays a refusal.
export function schedule(callback) {
  callback();
}
