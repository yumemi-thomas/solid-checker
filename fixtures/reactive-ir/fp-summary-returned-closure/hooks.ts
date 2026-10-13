import { createSignal } from "solid-js";

// Hooks defined in another file than their callers, so the call summary the
// component body consumes is a project-level one.

// The returned property closure and tuple element closure read `state` only
// when the closure runs.
export function createRemoteMutation() {
  const [state, setState] = createSignal<{ kind: string }>({ kind: "idle" });
  return {
    run: () => setState({ kind: "running" }),
    busy: () => state().kind === "running",
  };
}

export function createRemoteFlag() {
  const [s] = createSignal(0);
  return [s, () => s() > 0] as const;
}

// The hook itself reads while it is called.
export function createRemoteSnapshot() {
  const [s] = createSignal(0);
  const first = s();
  return { first, later: () => s() + 1 };
}
