import { onCleanup, onSettled } from "solid-js";

// onCleanup in compiler event handlers. The program also calls `click()` and
// `focus()`, so ADR 0356/0361 make every click, input, change and submit handler
// conditional; a host event (here the driver, outside the program) still runs
// each with no owner, so the dev build warns. Shapes from the corpus
// adjudication of the seeded mutants.

// ai-memory-ui backups.tsx:64: a download helper clicks a detached anchor.
function download(name: string) {
  const anchor = document.createElement("a");
  anchor.download = name;
  anchor.click();
}

export function Handlers() {
  let dialog!: HTMLDivElement;
  // ai-memory-ui access.tsx:311: focus moves into the dialog once settled.
  onSettled(() => {
    dialog.focus();
  });
  // ai-memory-ui audit.tsx:60 and shell.tsx:606: named handlers.
  const submit = (event: SubmitEvent) => {
    onCleanup(() => undefined);
    event.preventDefault();
  };
  const toggleCollapsed = () => {
    onCleanup(() => undefined);
  };
  return (
    <div ref={dialog} tabindex="-1">
      <button
        type="button"
        onClick={() => {
          onCleanup(() => undefined);
        }}
      >
        trigger
      </button>
      <input
        onInput={() => {
          onCleanup(() => undefined);
        }}
      />
      <select
        onChange={() => {
          onCleanup(() => undefined);
        }}
      >
        <option>never</option>
      </select>
      <form onSubmit={submit}>
        <button type="button" onClick={toggleCollapsed}>collapse</button>
      </form>
      <button type="button" onClick={() => download("backup.json")}>download</button>
    </div>
  );
}
