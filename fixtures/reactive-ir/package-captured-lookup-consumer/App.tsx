import { createSignal } from "solid-js";
import { lookup } from "reactive-package";
import * as Package from "reactive-package";

export function Body() {
  const [dict] = createSignal({ hello: "hello" }); const t = lookup(dict);
  const current = t("hello"); // proven SC1001
  return <div>{String(current)}</div>;
}
export function Tracked() {
  const [dict] = createSignal({ hello: "hello" }); const t = lookup(dict);
  return <div>{String(t("hello"))}</div>; // clean
}
export function Namespace() {
  const [dict] = createSignal({ hello: "hello" }); const t = Package.lookup(dict);
  t(".hello"); // proven SC1001; one leading dot only
  return <div />;
}
export function SetterRetained() {
  const [dict, setDict] = createSignal({ hello: "hello" }); const t = lookup(dict);
  void setDict; return <div>{String(t("hello"))}</div>; // uncertifiable
}
export function MissingKey() {
  const [dict] = createSignal({ hello: "hello" }); const t = lookup(dict);
  return <div>{String(t("toString"))}</div>; // uncertifiable, inherited dispatch
}
export function DynamicKey(props: { key: string }) {
  const [dict] = createSignal({ hello: "hello" }); const t = lookup(dict);
  return <div>{String(t(props.key))}</div>; // uncertifiable
}
export function CustomResolver() {
  const [dict] = createSignal({ hello: "hello" }); const t = lookup(dict, value => value);
  return <div>{String(t("hello"))}</div>; // uncertifiable
}
export function Getter() {
  const [dict] = createSignal({ get hello() { return "hello"; } }); const t = lookup(dict);
  return <div>{String(t("hello"))}</div>; // uncertifiable
}
export function Escaped() {
  const [dict] = createSignal({ hello: "hello" }); const t = lookup(dict);
  const alias = t; return <div>{String(alias("hello"))}</div>; // uncertifiable
}
export function Deferred() {
  const [dict] = createSignal({ hello: "hello" }); const t = lookup(dict);
  const read = () => t("hello"); return <div>{read()}</div>; // uncertifiable lifetime/mode
}
export function ExtraArgument() {
  const [dict] = createSignal({ hello: "hello" }); const t = lookup(dict);
  return <div>{String(t("hello", "params"))}</div>; // unsupported invocation slice
}
export function Shadowed() {
  const createSignal = <T,>(value: T): [() => T] => [() => value];
  const [dict] = createSignal({ hello: "hello" }); const t = lookup(dict);
  return <div>{String(t("hello"))}</div>; // no branded read proof
}
