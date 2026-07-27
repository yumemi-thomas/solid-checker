import {
  createMemo,
  type Component,
} from "solid-js";

export const JsxRead: Component<{ name: string }> = props => (
  <h1>{props.name}</h1>
);

export const TrackedRead: Component<{ name: string }> = props => {
  const name = createMemo(() => props.name);
  return <h1>{name()}</h1>;
};

export const EventRead: Component<{ name: string }> = props => (
  <button onClick={() => console.log(props.name)}>{props.name}</button>
);

function format(options: { name: string }) {
  const { name } = options;
  return name.toUpperCase();
}
export { format };
