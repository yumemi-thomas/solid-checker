import { createSignal, For, Show } from "solid-js";

// ---- Negatives: a helper nested in a component whose reads are all in its own
// JSX. The compiler lowers each of these positions so the read does not run
// while the helper builds the JSX: an attribute or child expression becomes a
// tracked effect, a component property becomes a getter its consumer reads.
// Calling such a helper from a <For> render callback is no strict-window read.

export function AttributeRead(props: { rows: string[] }) {
  const [opened] = createSignal<ReadonlySet<string>>(new Set());
  const line = (id: string) => <li aria-expanded={opened().has(id) ? "true" : "false"}>{id}</li>;
  return <ul><For each={props.rows}>{(row) => line(row)}</For></ul>;
}

export function ChildRead(props: { rows: string[] }) {
  const [opened] = createSignal<ReadonlySet<string>>(new Set());
  const line = (id: string) => <li>{opened().has(id) ? "open" : "closed"}</li>;
  return <ul><For each={props.rows}>{(row) => line(row)}</For></ul>;
}

export function PropertyRead(props: { rows: string[] }) {
  const [opened] = createSignal<ReadonlySet<string>>(new Set());
  const line = (id: string) => <Show when={opened().has(id)}><li>{id}</li></Show>;
  return <ul><For each={props.rows}>{(row) => line(row)}</For></ul>;
}

export function NestedHelperCall(props: { rows: string[] }) {
  const [opened] = createSignal<ReadonlySet<string>>(new Set());
  const isOpen = (id: string) => opened().has(id);
  // The call to `isOpen` is in the helper's tracked child expression.
  const line = (id: string) => <li>{isOpen(id) ? "open" : "closed"}</li>;
  return <ul><For each={props.rows}>{(row) => line(row)}</For></ul>;
}

// ---- Positives: the helper reads while it is called, so a call from the
// <For> render callback (a strict-read window) is reported.

export function ReadDuringCall(props: { rows: string[] }) {
  const [opened] = createSignal<ReadonlySet<string>>(new Set());
  const label = (id: string) => (opened().has(id) ? "open" : "closed");
  return <ul><For each={props.rows}>{(row) => { const text = label(row); return <li>{text}</li>; }}</For></ul>;
}

export function ReadBeforeJsx(props: { rows: string[] }) {
  const [opened] = createSignal<ReadonlySet<string>>(new Set());
  const line = (id: string) => {
    const isOpen = opened().has(id);
    return <li>{isOpen ? "open" : "closed"}</li>;
  };
  return <ul><For each={props.rows}>{(row) => line(row)}</For></ul>;
}
