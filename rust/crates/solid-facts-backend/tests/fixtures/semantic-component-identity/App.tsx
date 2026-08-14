import {
  createRoot,
  createSignal,
  type Component,
  type ComponentProps,
} from "solid-js";

const [count, setCount] = createSignal(0);

type Accessor<T> = () => T;
const localSameName: Accessor<number> = () => 1;

const lowercase: Component = () => {
  setCount(1);
  return null;
};

function UppercaseFactory() {
  setCount(2);
}

function Route(props: { component: Component }) {
  return props.component({});
}

function Home({ homeName }: { homeName: string }) {
  return <div>{homeName}</div>;
}

declare function setPage(component: Component): void;

// A reference nested inside an event callback is a value passed to setPage;
// it does not turn Home itself into the event callback.
void <button onClick={() => setPage(Home)} />;

// The createRoot callback is the argument. Nested is merely declared inside
// that callback and remains independently component-shaped.
const rooted = createRoot(() => {
  const Nested = ({ nestedName }: { nestedName: string }) => (
    <div>{nestedName}</div>
  );
  return Nested;
});
void rooted;

// JSX-producing render helpers are callbacks, not components. Neither an
// Array.map row nor a function-valued children prop accepts component props.
void [{ id: 1 }].map(({ id }) => <div>{id}</div>);
function Consumer(props: { children: (value: { id: number }) => unknown }) {
  return props.children({ id: 1 });
}
void <Consumer children={({ id }) => <div>{id}</div>} />;

const app: Component = () => {
  lowercase({});
  localSameName();
  return <div />;
};

const props: ComponentProps<typeof app> = {};
void props;
void <Route component={Home} />;
function SpreadHome({ spreadName }: { spreadName: string }) {
  return <div>{spreadName}</div>;
}
void <Route {...{ component: SpreadHome }} />;
UppercaseFactory();
void app;
