// ai-memory-ui confirm-dialog.tsx:22: `onSettled` focuses an element held in a `let` filled by `ref`.
import { onSettled } from "solid-js";

export function ConfirmDialog(props: { title: string; onClose: () => void }) {
  let dialog: HTMLDivElement | undefined;

  onSettled(() => {
    dialog?.focus();
  });

  return (
    <div ref={dialog} role="dialog" tabindex="-1" onClick={() => props.onClose()}>
      {props.title}
    </div>
  );
}
