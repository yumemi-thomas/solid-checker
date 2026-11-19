import { createSignal } from "solid-js";

function run(cb: () => unknown) {
  cb();
  return 1;
}

// ADR 1103: a prop read in an anonymous literal handed to a project helper (`run`) inside a named local function the body
// calls. The helper runs the literal, so the read runs untracked in the render (Chrome: STRICT_READ_UNTRACKED), but no summary
// carries a read of the literal to `go()`: it was a clean nothing explained. Now SC1001 uncertifiable, never silent.
function Handed(props: { value: number }) {
  const go = () => {
    run(() => {
      props.value;
    });
  };
  go();
  return <i />;
}

// Kept: the same read written directly in the named function is the function's own, which its call `go()` reports (violation).
function Direct(props: { value: number }) {
  const go = () => {
    props.value;
  };
  go();
  return <i />;
}

// Kept: the literal handed to the helper straight from the body is the body's (violation at the read).
function BodyHanded(props: { value: number }) {
  run(() => {
    props.value;
  });
  return <i />;
}

// Kept: an accessor read in the same handed literal is carried to `go()` by the summaries (violation there).
function HandedAccessor() {
  const [count] = createSignal(0);
  const go = () => {
    run(() => {
      count();
    });
  };
  go();
  return <i />;
}

export function Page() {
  const [value] = createSignal(1);
  return (
    <div>
      <Handed value={value()} />
      <Direct value={value()} />
      <BodyHanded value={value()} />
      <HandedAccessor />
    </div>
  );
}
