// Corpus adjudication 2026-10-23 (scripts/corpus/reviewed-findings.json): an
// onCleanup that runs with no owner. rc.17 dev warns NO_OWNER_CLEANUP and the
// cleanup never runs (@solidjs/signals dist/dev.js:2377-2389).
import { For, createSignal, onCleanup, onSettled } from "solid-js";

// probus-hk Layout.tsx:73, RailDiagram.tsx:211,325, SwipeActions.tsx:195: a named
// ref callback; @solidjs/web rc.17 ref() applies it under runWithOwner(null)
// (dist/web.dev.js:1019-1022).
export function NamedRefCleanup() {
  const watch = (el: HTMLDivElement) => {
    const observer = new ResizeObserver(() => undefined);
    observer.observe(el);
    onCleanup(() => observer.disconnect());
  };
  return <div ref={watch} />;
}

// probus-hk VirtualRows.tsx:184: an inline ref arrow on every row.
export function InlineRefCleanup() {
  const [rows] = createSignal([1, 2]);
  return (
    <For each={rows()}>
      {(row) => (
        <div
          ref={(el) => {
            el.dataset.row = String(row);
            onCleanup(() => el.removeAttribute("data-row"));
          }}
        />
      )}
    </For>
  );
}

// app-game rotating-cube-2.tsx:303: cleanup registered after an await, in an
// async function onSettled starts.
async function main(canvas: HTMLCanvasElement) {
  await Promise.resolve();
  const frame = requestAnimationFrame(() => undefined);
  onCleanup(() => cancelAnimationFrame(frame));
  canvas.dataset.started = "yes";
}
export function AfterAwaitCleanup() {
  const canvas = (<canvas />) as HTMLCanvasElement;
  onSettled(() => {
    void main(canvas);
  });
  return <>{canvas}</>;
}

// kui session.ts:749 via MessagesTab.stories.tsx:40: a factory that registers
// cleanup, called at module scope for a story's args.
function createSession() {
  const [rows] = createSignal(0);
  onCleanup(() => undefined);
  return { rows };
}
const story = { args: { session: createSession() } };
export function ModuleFactory() {
  return <p>{story.args.session.rows()}</p>;
}
