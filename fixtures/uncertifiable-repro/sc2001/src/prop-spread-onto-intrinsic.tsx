// app-game web-audio/range.tsx:19 (used at web-audio-page.tsx:113-150): a component spreads all its props onto an element, so its
// callback prop (`valueChange`) reaches the element under a key the compiler does not lower as an event, and it calls the prop as well.
import { createSignal } from "solid-js";
import type { ComponentProps } from "@solidjs/web";

type RangeProps = { value?: number; valueChange?: (value: number) => void } & ComponentProps<"div">;

function Range(props: RangeProps) {
  return (
    <div {...props}>
      <input type="range" value={props.value} onInput={(event) => props.valueChange?.(event.target.valueAsNumber)} />
    </div>
  );
}

export function Settings() {
  const [gain, setGain] = createSignal(1);
  return <Range value={gain()} valueChange={(value) => setGain(value)} />;
}
