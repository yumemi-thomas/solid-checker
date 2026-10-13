import type { JSX } from "@solidjs/web";

type Decor = { notes?: () => JSX.Element; header?: () => JSX.Element };

function Card(props: { notes?: JSX.Element }) {
  return <p>{props.notes}</p>;
}
export function AttributeConsumer(props: { decor?: Decor }) {
  return <Card notes={props.decor?.notes?.()} />;
}
export function ExpressionConsumer(props: { decor?: Decor }) {
  const decorated = () => {
    const draw = props.decor?.header;
    return draw === undefined ? undefined : draw();
  };
  return <p>{decorated()}</p>;
}
