import { createMemo, createSignal } from "solid-js";

const [id] = createSignal(0);
declare function fetchUser(id: number): Promise<string>;
declare const opts: { sync?: boolean };
declare const flag: boolean;

export function SyncOptions() {
  // Control: a literal sync: true with an async computation.
  const a = createMemo(async () => fetchUser(id()), { sync: true });
  // Options the syntax cannot read: uncertifiable.
  const b = createMemo(async () => fetchUser(id()), opts);
  const c = createMemo(async () => fetchUser(id()), { sync: flag });
  // Proven absent or false, or a synchronous computation: no finding.
  const d = createMemo(async () => fetchUser(id()), { sync: false });
  const e = createMemo(async () => fetchUser(id()), { name: "e" });
  const f = createMemo(() => id(), opts);
  return <div>{a()}{b()}{c()}{d()}{e()}{f()}</div>;
}
