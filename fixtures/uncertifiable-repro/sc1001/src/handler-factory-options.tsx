// kui kernel Dialog.tsx:99-101: a JSX attribute calls a project handler factory with an options object that reads props.
import { Show } from "solid-js";
import { Portal } from "@solidjs/web";

type ScrimOptions = { onClose: () => void; closeOnScrimClick?: boolean };

function scrimClickHandler(scrim: () => HTMLElement | undefined, options: ScrimOptions) {
  return (event: MouseEvent): void => {
    if (options.closeOnScrimClick === false) return;
    if (event.target === scrim()) options.onClose();
  };
}

export function Dialog(props: { open: boolean; onClose: () => void; closeOnScrimClick?: boolean }) {
  let scrim: HTMLElement | undefined;
  return (
    <Show when={props.open}>
      <Portal mount={document.body}>
        <div
          ref={(element: HTMLElement) => (scrim = element)}
          onClick={scrimClickHandler(() => scrim, {
            onClose: () => props.onClose(),
            ...(props.closeOnScrimClick !== undefined ? { closeOnScrimClick: props.closeOnScrimClick } : {}),
          })}
        />
      </Portal>
    </Show>
  );
}
