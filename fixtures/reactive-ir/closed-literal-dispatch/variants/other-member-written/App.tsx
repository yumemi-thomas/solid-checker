import { createSignal } from "solid-js";

const [, setW] = createSignal(0);

export function OtherMemberWritten() {
  const t: { then(f: () => void): void; init?: () => void } = { then(f: () => void) { f(); } };
  t.init = () => {};
  (t as unknown as Promise<void>).then(() => { setW(6); });
  return <p />;
}
