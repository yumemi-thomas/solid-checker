import { box, created, dates, defined, plain, proxied, valued } from "fz-dep";
import "fz-globals";

export function Reads() {
  const a = defined.v;
  const b = box.v;
  const c = proxied.v;
  const d = created.v;
  return <p title={String(a + b + c + d)} />;
}

export function Clean() {
  const a = valued.v;
  const b = plain.w;
  const c = dates.size;
  return <p title={String(a + b + c)} />;
}
