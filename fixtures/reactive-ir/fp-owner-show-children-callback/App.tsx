import { For, Show } from "solid-js";

// Positive control: the function that IS <Show>'s children is the one whose
// parameter is an accessor (non-keyed `Show` hands `(item: Accessor<...>)`).
export function ShowChildrenParameter(props: { user: string | undefined }) {
  return (
    <Show when={props.user}>
      {(user) => <span class={`user-${user}`} />}
    </Show>
  );
}

// The same through the explicit `children` prop.
export function ShowChildrenProp(props: { user: string | undefined }) {
  return <Show when={props.user} children={(user) => <span class={`prop-${user}`} />} />;
}

// Negative: functions merely written inside <Show> belong to their own
// element. A default-keyed <For> hands the RAW row item, an input handler gets
// a plain Event, and a ref callback gets the element.
export function PlainCallbacksInShow(props: { ok: boolean; tasks: readonly string[]; onPick: (value: string) => void }) {
  return (
    <Show when={props.ok}>
      <For each={props.tasks}>{(task) => <span class={`task-${task}`} />}</For>
      <input
        onChange={(event) => {
          event.currentTarget.value = "";
          props.onPick(`${event.type}`);
        }}
        ref={(element) => {
          element.value = "initial";
        }}
      />
    </Show>
  );
}

// Positive control: <For>'s own second parameter IS an accessor, and a
// <Show> around it does not change that.
export function ForIndexInShow(props: { ok: boolean; tasks: readonly string[] }) {
  return (
    <Show when={props.ok}>
      <For each={props.tasks}>{(task, index) => <span class={`row-${index}-${task}`} />}</For>
    </Show>
  );
}
