import { createSignal } from "solid-js";

type Render = () => JSX.Element;

function Runs(props: { render: Render }) {
  const el = props.render();
  return <div>{el}</div>;
}

// Positive control: the body calls the prop it was given (ADR 0204).
export function Called() {
  const [count] = createSignal(0);
  return (
    <Runs
      render={() => {
        const n = count();
        return <span>{n}</span>;
      }}
    />
  );
}

function Aliased(props: { render: Render }) {
  const saved = props.render;
  const alias = props;
  alias.render = () => <span>replacement</span>;
  props.render();
  return <div>{saved()}</div>;
}

export function AliasedProps() {
  const [count] = createSignal(0);
  return (
    <Aliased
      render={() => {
        const n = count();
        return <span>{n}</span>;
      }}
    />
  );
}

function Deleting(props: { render?: Render }) {
  const saved = props.render;
  delete props.render;
  (props as { render?: Render }).render?.();
  return <div>{saved?.()}</div>;
}

export function DeletedProp() {
  const [count] = createSignal(0);
  return (
    <Deleting
      render={() => {
        const n = count();
        return <span>{n}</span>;
      }}
    />
  );
}

type Replacing = { render: Render; replace: () => void };

function Replaced(props: Replacing) {
  const saved = props.render;
  props.replace();
  props.render();
  return <div>{saved()}</div>;
}

export function ReplacedThroughThis() {
  const [count] = createSignal(0);
  return (
    <Replaced
      replace={function (this: Replacing) {
        this.render = () => <span>replacement</span>;
      }}
      render={() => {
        const n = count();
        return <span>{n}</span>;
      }}
    />
  );
}

function ByArguments(props: { render: Render }) {
  const saved = props.render;
  Reflect.set(arguments[0], "render", () => <span>replacement</span>);
  props.render();
  return <div>{saved()}</div>;
}

export function ThroughArguments() {
  const [count] = createSignal(0);
  return (
    <ByArguments
      render={() => {
        const n = count();
        return <span>{n}</span>;
      }}
    />
  );
}

function LoopReplaced(props: { render: Render }) {
  for (props.render of [() => <span>replacement</span>]) {
    break;
  }
  return props.render();
}

export function LoopHead() {
  const [count] = createSignal(0);
  return (
    <LoopReplaced
      render={() => {
        const n = count();
        return <span>{n}</span>;
      }}
    />
  );
}

type Redeclared = { render: Render };

function Redeclaring(props: Redeclared) {
  var props: Redeclared = { render: () => <span>replacement</span> };
  return props.render();
}

export function RedeclaredProps() {
  const [count] = createSignal(0);
  return (
    <Redeclaring
      render={() => {
        const n = count();
        return <span>{n}</span>;
      }}
    />
  );
}

function Constructed(props: { render: Render }) {
  class Later {
    value = props.render();
  }
  return <button onClick={() => void new Later()}>click</button>;
}

export function ClassField() {
  const [count] = createSignal(0);
  return (
    <Constructed
      render={() => {
        const n = count();
        return <span>{n}</span>;
      }}
    />
  );
}

let Swapped = (props: { render: Render }): JSX.Element => props.render();

function Later(props: { render: Render }) {
  return <button onClick={() => props.render()}>click</button>;
}
Swapped = Later;

export function SwappedTag() {
  const [count] = createSignal(0);
  return (
    <Swapped
      render={() => {
        const n = count();
        return <span>{n}</span>;
      }}
    />
  );
}
