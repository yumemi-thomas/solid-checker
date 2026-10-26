// kui kernel Select.tsx:205 (and the kernel's exported hooks and helpers):
// the package is a workspace member a sibling depends on (frontend/package.json
// workspaces; feature-* depend on @kui/kernel), so its program is open
// (ADR 0444) and no exported symbol's callers can be enumerated.
import { createContext, createSignal, onCleanup, useContext, For, Show } from "solid-js";

const DensityContext = createContext<{ compact: boolean }>({ compact: false });

export function useDensity() {
  const density = useContext(DensityContext);
  onCleanup(() => console.debug("density released"));
  return density;
}

export function describeOption(option: { label: () => string }) {
  return option.label().trim();
}

export function Select(props: { options: string[]; disabled?: boolean }) {
  const [open, setOpen] = createSignal(false);
  return (
    <div>
      <button
        onClick={() => {
          if (props.disabled === true) return;
          setOpen(!open());
        }}
      >
        {props.options.length}
      </button>
      <Show when={open()}>
        <ul>
          <For each={props.options}>{(option) => <li>{option}</li>}</For>
        </ul>
      </Show>
    </div>
  );
}
