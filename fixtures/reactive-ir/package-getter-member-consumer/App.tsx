import { createMemo, untrack } from "solid-js";
import { makeSize } from "reactive-package";
import * as pkg from "reactive-package";
declare function unknownCall(value: unknown): void;

export function Body() {
  const location = makeSize();
  const path = location.pathname;
  const search = location.search;
  const hash = location.hash;
  const state = location.state;
  return <div>{path}{search}{hash}{String(state)}</div>;
}
export function Tracked() {
  const location = makeSize();
  const path = createMemo(() => location.pathname);
  return <div>{path()}{location.search}{location.hash}{String(location.state)}</div>;
}
export function Inert() {
  const location = makeSize();
  const key = location.key;
  const query = location.query;
  const keys = Object.keys(location);
  const has = Reflect.has(location, "pathname");
  const ownKeys = Reflect.ownKeys(location);
  return <div>{key}{keys.join()}{String(has)}{String(ownKeys)}{query.q}</div>;
}
export function QueryRead() {
  const location = makeSize();
  const q = location.query.q;
  return <div>{q}</div>;
}
export function QueryAlias() {
  const location = makeSize();
  const query = location.query;
  const q = query.q;
  return <div>{q}</div>;
}
export function Destructure() {
  const { pathname } = makeSize();
  return <div>{pathname}</div>;
}
export function DestructureLater() {
  const location = makeSize();
  const { pathname } = location;
  return <div>{pathname}</div>;
}
export function Spread() {
  const location = makeSize();
  const copy = { ...location };
  return <div>{copy.pathname}</div>;
}
export function Assign() {
  const location = makeSize();
  const copy = Object.assign({}, location);
  return <div>{copy.pathname}</div>;
}
export function Serialize() {
  const location = makeSize();
  const text = JSON.stringify(location);
  return <div>{text}</div>;
}
export function Untracked() {
  const location = makeSize();
  const path = untrack(() => location.pathname);
  return <div>{path}</div>;
}
export function Computed() {
  const location = makeSize();
  return <div>{location["pathname"]}</div>;
}
export function Escaped() {
  const location = makeSize();
  unknownCall(location);
  return <div>{location.pathname}</div>;
}
export function Mutable() {
  let location = makeSize();
  return <div>{location.pathname}</div>;
}
export function Alias() {
  const location = makeSize();
  const alias = location;
  return <div>{alias.pathname}</div>;
}
export function Reflective() {
  const location = makeSize();
  const descriptor = Object.getOwnPropertyDescriptor(location, "pathname");
  return <div>{String(descriptor)}</div>;
}
export function ProxyEscape() {
  const location = makeSize();
  const boxed = { query: location.query };
  return <div>{boxed.query.q}</div>;
}
export function HasSyntax() {
  const location = makeSize();
  const present = "pathname" in location;
  return <div>{String(present)}</div>;
}
export function CopyQuery() {
  const location = makeSize();
  const copy = { ...location };
  const q = copy.query.q;
  return <div>{q}</div>;
}
export function DirectFactory() {
  const path = makeSize().pathname;
  return <div>{path}</div>;
}
export function Wrapped() {
  const location = makeSize();
  const path = (location as typeof location).pathname;
  return <div>{path}</div>;
}
export function Namespace() {
  const location = pkg.makeSize();
  const path = location.pathname;
  return <div>{path}</div>;
}
export function HelperTracked() {
  const location = makeSize();
  function read() { return location.pathname; }
  return <div>{read()}</div>;
}
export function HelperBody() {
  const location = makeSize();
  function read() { return location.pathname; }
  const path = read();
  return <div>{path}</div>;
}
export function HelperMixed() {
  const location = makeSize();
  function read() { return location.pathname; }
  unknownCall(read);
  return <div>{read()}</div>;
}
export function Shadowed() {
  const makeSize = () => ({ pathname: "plain" });
  const location = makeSize();
  const path = location.pathname;
  return <div>{path}</div>;
}
