import { createSignal } from "solid-js";

declare function opaque(callback: (x: number) => JSX.Element): JSX.Element;

function Kids(props: { children: (x: number) => JSX.Element }) {
  const el = props.children(1);
  return <div>{el}</div>;
}

// Positive: `Kids` calls its function child in its body.
export function BodyChild() {
  const [count] = createSignal(0);
  return (
    <Kids>
      {(x) => {
        console.log(count());
        return <span>{x}</span>;
      }}
    </Kids>
  );
}

// Positive: the same child inside a tracked JSX expression of the caller,
// which lowers the `<Kids>` element but does not run the child.
export function BodyChildInRegion() {
  const [count] = createSignal(0);
  const [shown] = createSignal(true);
  return (
    <div>
      {shown() && (
        <Kids>
          {(x) => {
            console.log(count());
            return <span>{x}</span>;
          }}
        </Kids>
      )}
    </div>
  );
}

function Handed(props: { children: (x: number) => JSX.Element }) {
  return <div>{opaque(props.children)}</div>;
}

// Uncertifiable: the child is handed to a function with no body.
export function OpaqueChild() {
  const [count] = createSignal(0);
  return (
    <Handed>
      {(x) => {
        console.log(count());
        return <span>{x}</span>;
      }}
    </Handed>
  );
}

// Uncertifiable: a spread beside the child refuses the proof.
export function SpreadChild(rest: { title?: string }) {
  const [count] = createSignal(0);
  return (
    <Kids {...rest}>
      {(x) => {
        console.log(count());
        return <span>{x}</span>;
      }}
    </Kids>
  );
}

function InJsx(props: { children: (x: number) => JSX.Element }) {
  return <div>{props.children(1)}</div>;
}

// Negative: the child is called in a tracked JSX expression.
export function TrackedChild() {
  const [count] = createSignal(0);
  return (
    <InJsx>
      {(x) => {
        const n = count();
        return <span>{x + n}</span>;
      }}
    </InJsx>
  );
}

function OnClick(props: { children: () => void }) {
  return <button onClick={() => props.children()}>x</button>;
}

// Negative: the child runs only from a click handler.
export function EventChild() {
  const [count] = createSignal(0);
  return <OnClick>{() => console.log(count())}</OnClick>;
}

// Negative: a function child of an intrinsic element is inserted and run
// by the compiler in a tracked effect.
export function IntrinsicChild() {
  const [count] = createSignal(0);
  return <div>{() => count()}</div>;
}

function Wrap(props: { children: JSX.Element }) {
  return <div>{props.children}</div>;
}

// Negative: an expression child that is not a function is the compiler's
// `children` getter, read where `Wrap` reads it, as before.
export function ExpressionChild() {
  const [count] = createSignal(0);
  return <Wrap>{count()}</Wrap>;
}

function Aliased(props: { children: () => JSX.Element }) {
  const saved = props.children;
  const alias = props;
  alias.children = () => <span>replacement</span>;
  props.children();
  return <div>{saved()}</div>;
}

// Uncertifiable, no violation: `props` escapes into an alias that replaces
// `children`, so the body's call runs the replacement, not this child.
export function AliasedChild() {
  const [count] = createSignal(0);
  return (
    <Aliased>
      {() => {
        const n = count();
        return <span>{n}</span>;
      }}
    </Aliased>
  );
}

function Deleting(props: { children?: () => JSX.Element }) {
  const saved = props.children;
  delete props.children;
  (props as { children?: () => JSX.Element }).children?.();
  return <div>{saved?.()}</div>;
}

// Uncertifiable, no violation: the body deletes `children` before the call.
export function DeletedChild() {
  const [count] = createSignal(0);
  return (
    <Deleting>
      {() => {
        const n = count();
        return <span>{n}</span>;
      }}
    </Deleting>
  );
}

type Replacing = { children: () => JSX.Element; replace: () => void };

function Replaced(props: Replacing) {
  const saved = props.children;
  props.replace();
  props.children();
  return <div>{saved()}</div>;
}

// Uncertifiable, no violation: `props.replace()` runs with `props` as `this`
// and replaces `children` before the call.
export function ReplacedChild() {
  const [count] = createSignal(0);
  return (
    <Replaced
      replace={function (this: Replacing) {
        this.children = () => <span>replacement</span>;
      }}
    >
      {() => {
        const n = count();
        return <span>{n}</span>;
      }}
    </Replaced>
  );
}

function ByArguments(props: { children: () => JSX.Element }) {
  const saved = props.children;
  Reflect.set(arguments[0], "children", () => <span>replacement</span>);
  props.children();
  return <div>{saved()}</div>;
}

// Uncertifiable, no violation: `arguments[0]` is `props`, and the body
// replaces `children` through it before the call.
export function ArgumentsChild() {
  const [count] = createSignal(0);
  return (
    <ByArguments>
      {() => {
        const n = count();
        return <span>{n}</span>;
      }}
    </ByArguments>
  );
}
