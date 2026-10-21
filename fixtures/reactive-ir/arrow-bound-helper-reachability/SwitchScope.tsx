// Negative: `helper` is declared in `case 0` and used in `case 1`; a jump to
// `case 1` skips the declarator. A `}` in a comment or string must not hide
// the switch from the proof, so the proof reads the facts, not the text.
import { createSignal } from "solid-js";

const [count, setCount] = createSignal(0);

// mode 1 jumps to `case 1`, skipping `helper`'s declarator: ReferenceError.
export function Switched(props: { mode: number }) {
  switch (props.mode) {
    case 0:
      // a stray brace in a comment: }
      const helper = () => {
        setCount(1);
      };
      break;
    case 1: {
      const run = () => helper();
      run();
      break;
    }
  }
  return <button>{count()}</button>;
}

// Control: no stray brace.
export function SwitchedPlain(props: { mode: number }) {
  switch (props.mode) {
    case 0:
      const helper = () => {
        setCount(2);
      };
      break;
    case 1: {
      const run = () => helper();
      run();
      break;
    }
  }
  return <button>{count()}</button>;
}
