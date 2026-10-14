import fallback from "./source";
import { timer, reader, quoted, frozen, shorthand, throughReader, maybeReader, laterReader } from "./source";
export function MissingOwner() {
  return <button onClick={async () => { await Promise.resolve(); timer.useTicker(() => {}); }}>start</button>;
}
export function Reads() {
  const a = reader.readCount();
  const b = frozen.frozenRead();
  const c = shorthand.shorthandRead();
  const d = fallback.defaultRead();
  const e = reader?.readCount?.();
  const f = quoted.quotedRead();
  const g = throughReader();
  return <p>{a + b + c + d + e + f + g}</p>;
}
export function Owned() {
  timer.useTicker(() => {});
  return <p>owned</p>;
}
declare const flag: boolean;
export function UnprovenReads() {
  const n = maybeReader(flag);
  void laterReader();
  return <p>{n}</p>;
}
