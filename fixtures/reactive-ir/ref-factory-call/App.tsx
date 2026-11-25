import { createSignal, untrack } from "solid-js";

// ADR 1209: `ref={dir()}` on an intrinsic element is a two-phase ref. The compiler evaluates the factory `dir()` inline, exactly once, where
// the element's markup is evaluated (`RefFactory`); the function it returns runs later, in the directive-apply phase. Markup written in the
// component's own body is evaluated while the body runs, so `dir`'s body runs there: untracked in the render (Chrome: STRICT_READ_UNTRACKED).

// Violation at `dir()`: a prop read in the factory.
function Read(props: { value: number }) {
  const dir = () => {
    props.value;
    return () => {};
  };
  return <p ref={dir()}>x</p>;
}

// Violation at `dir()`: the prop read in an IIFE in the factory (ADR 1184's census shape).
function ReadIife(props: { value: number }) {
  const dir = () => {
    (() => {
      props.value;
    })();
    return () => {};
  };
  return <p ref={dir()}>x</p>;
}

// Clean: the factory reads under untrack.
function Untracked(props: { value: number }) {
  const dir = () => {
    untrack(() => props.value);
    return () => {};
  };
  return <p ref={dir()}>x</p>;
}

// Kept uncertifiable: the element is a component's child, evaluated where the component reads its children, not in this body.
function Wrap(props: { children?: any }) {
  return <div>{props.children}</div>;
}
function InChild(props: { value: number }) {
  const dir = () => {
    props.value;
    return () => {};
  };
  return (
    <Wrap>
      <p ref={dir()}>x</p>
    </Wrap>
  );
}

// Kept uncertifiable: the factory is one arm of a conditional ref (evaluated at most once, not proven inline).
function Conditional(props: { value: number; on: boolean }) {
  const dir = () => {
    props.value;
    return () => {};
  };
  return <p ref={props.on ? dir() : undefined}>x</p>;
}

export function Page() {
  const [value] = createSignal(1);
  return (
    <div>
      <Read value={value()} />
      <ReadIife value={value()} />
      <Untracked value={value()} />
      <InChild value={value()} />
      <Conditional value={value()} on={true} />
    </div>
  );
}
