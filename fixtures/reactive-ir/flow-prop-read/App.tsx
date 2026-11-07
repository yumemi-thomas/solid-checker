import { createMemo, For, Loading, Match, Show, Switch } from "solid-js";

// ADR 0892: rc.21 reads these props in a computation of the component's own, so
// a pending value suspends it: no SC5001. With no `Loading` the render still
// meets it (Chrome: ASYNC_OUTSIDE_LOADING_BOUNDARY), so SC5003 stays yours.
export function ShowWhen() {
  const pending = createMemo(async () => 1);
  return <Show when={pending()}><i /></Show>;
}

export function ForEach() {
  const pending = createMemo(async () => [1]);
  return <For each={pending()}>{() => <i />}</For>;
}

export function MatchWhen() {
  const pending = createMemo(async () => 1);
  return (
    <Switch>
      <Match when={pending()}>
        <i />
      </Match>
    </Switch>
  );
}

export function Covered() {
  const pending = createMemo(async () => 1);
  return (
    <Loading fallback={0}>
      <Show when={pending()}>
        <i />
      </Show>
    </Loading>
  );
}

// A memo over a value its caller hands in: not proved synchronous (ADR 0802),
// and no producer has a row at this read.
export function OpenMemoShow(props: { get: () => number }) {
  const tips = createMemo(() => props.get());
  return <Show when={tips()}><i /></Show>;
}

// Dropped: the helper's result is never rendered (Chrome rc.21 is silent), so the
// boundary claim is not proved.
export function DroppedElement() {
  const pending = createMemo(async () => 1);
  const helper = () => {
    return <Show when={pending()}><i /></Show>;
  };
  return <p>{(helper(), "x")}</p>;
}

// Review D20: not held. The compiler passes a `/*@static*/` value raw, read in
// the body (Chrome: PENDING_ASYNC_UNTRACKED_READ).
export function StaticMarker() {
  const pending = createMemo(async () => 1);
  return <Show when={/*@static*/ pending()}><i /></Show>;
}

// An element between: the read is that element's.
function Kid(props: { y: unknown }) {
  return <i>{String(props.y)}</i>;
}

export function ElementBetween() {
  const pending = createMemo(async () => 1);
  return <Show when={<Kid y={pending()} />}><i /></Show>;
}

// `Match` returns its props: outside a `Switch` nothing reads them in a memo.
export function MatchAlone() {
  const pending = createMemo(async () => 1);
  return <p>{String((<Match when={pending()}><i /></Match> as unknown as { when: unknown }).when)}</p>;
}

// A spread may supply anything for the tag.
export function SpreadOnTag(props: { keyed?: false }) {
  const pending = createMemo(async () => 1);
  return <Show {...props} when={pending()}><i /></Show>;
}

// A function between: a getter's own read, placed elsewhere.
export function FunctionBetween() {
  const pending = createMemo(async () => 1);
  return <Show when={(() => pending())}><i /></Show>;
}
