// A named write anywhere vetoes the key, including through another receiver.
export function replaceRemote(target: { remoteRun(): void }) {
  target.remoteRun = () => {};
}
