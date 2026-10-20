import { createSignal, For, Match, Repeat, Show, Switch } from "solid-js";

// ---- P1: a helper merely mentioned in a control-flow attribute is not the
// render callback. Each reads `page` inside a nested helper that is called
// from a tracked attribute, so none of these reads is untracked.

export function WhenHelper() {
  const [page] = createSignal(1);
  const visible = () => page() + 1;
  return <Show when={visible() > 0}><div /></Show>;
}
export function WhenHelperKeyed() {
  const [page] = createSignal(1);
  const visible = () => page() + 1;
  return <Show when={visible() > 0} keyed><div /></Show>;
}
export function WhenHelperFallback() {
  const [page] = createSignal(1);
  const visible = () => page() + 1;
  return <Show when={visible() > 0} fallback={<div />}><div /></Show>;
}
export function EachHelper() {
  const [page] = createSignal(1);
  const visible = () => page() + 1;
  return <For each={[visible()]}>{(x) => <div>{x}</div>}</For>;
}
export function CountHelper() {
  const [n] = createSignal(1);
  const v = () => n() + 1;
  return <Repeat count={v()}>{(i) => <div>{i}</div>}</Repeat>;
}
export function MatchHelper() {
  const [n] = createSignal(1);
  const v = () => n() + 1;
  return (
    <Switch fallback={<div />}>
      <Match when={v() > 1}><div /></Match>
    </Switch>
  );
}
export function HelperInChildAttribute() {
  const [n] = createSignal(1);
  const v = () => n() + 1;
  return <Show when={n() > 1}><div title={String(v())} /></Show>;
}

// ---- P4: a function inside an attribute of the control-flow element is an
// ordinary expression of that attribute, not the render callback.

export function EachFilter(props: { id: string; items: { slug: string }[] }) {
  return <For each={props.items.filter((p) => p.slug !== props.id)}>{(p) => <div>{p.slug}</div>}</For>;
}
export function EachFromSignal() {
  const [n] = createSignal(3);
  return <For each={Array.from({ length: n() }, (_, i) => i * n())}>{(i) => <div>{i}</div>}</For>;
}
export function WhenSome(props: { id: string; items: { slug: string }[] }) {
  return <Show when={props.items.some((p) => p.slug === props.id)}><div /></Show>;
}

// ---- P5: a read directly in a JSX fragment returned by a render callback is
// compiled into the fragment's own insert effect: tracked.

export function FragmentInShow(props: { tone?: string }) {
  return <Show when={props.tone}>{(tone) => (<><b>{tone()}</b>{tone().length}</>)}</Show>;
}
export function FragmentInFor(props: { items: string[] }) {
  return <For each={props.items}>{(it, i) => (<><b>{it}</b>{i() < 3 && <span>/</span>}</>)}</For>;
}

// ---- Positive controls: the render callback itself runs once per item (or per
// truthy `when`) outside any tracking scope, so a read written directly in
// its body is a proven untracked read.

export function DirectBodyRead() {
  const [n] = createSignal(1);
  const now = n();
  return <div title={String(now)} />;
}
export function ComponentPropRead(props: { variant?: string }) {
  const variant = props.variant ?? "primary";
  return <div title={variant} />;
}
export function ShowCallbackBodyRead(props: { tone?: string }) {
  const [n] = createSignal(1);
  return <Show when={props.tone}>{(tone) => { const snapshot = n(); return <div title={tone() + snapshot} />; }}</Show>;
}
export function ForCallbackBodyRead(props: { items: string[] }) {
  const [n] = createSignal(1);
  return <For each={props.items}>{(item) => { const snapshot = n(); return <div title={item + snapshot} />; }}</For>;
}
export function NamedRenderCallbackBodyRead(props: { items: string[] }) {
  const [n] = createSignal(1);
  const renderItem = (item: string) => { const snapshot = n(); return <div title={item + snapshot} />; };
  return <For each={props.items}>{renderItem}</For>;
}
