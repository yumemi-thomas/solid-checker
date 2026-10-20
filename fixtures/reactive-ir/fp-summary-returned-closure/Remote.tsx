import { createRemoteFlag, createRemoteMutation, createRemoteSnapshot } from "./hooks";

// ---- Negatives: the imported hooks are called in the body, their closures
// are only read in tracked or handler positions.

export function RemoteScreen() {
  const m = createRemoteMutation();
  return (
    <button disabled={m.busy()} onClick={() => m.run()}>
      go
    </button>
  );
}

export function RemoteFlag() {
  const [value, positive] = createRemoteFlag();
  return <span>{positive() ? value() : 0}</span>;
}

// ---- Positive control: the imported hook reads its signal while it runs.

export function RemoteSnapshot() {
  const snap = createRemoteSnapshot();
  return <span>{snap.first}</span>;
}
