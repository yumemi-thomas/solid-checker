import { createMemo, merge, omit } from "solid-js";

// no-destructure (SC1003): props read through the object, split with
// omit/merge, or destructured inside a tracking computation.
export function Card(props: { title: string; body: string }) {
  return (
    <article>
      <h2>{props.title}</h2>
      <p>{props.body}</p>
    </article>
  );
}

export function Field(props: { label: string; value: string }) {
  const rest = omit(props, "label");
  const merged = merge({ type: "text" }, rest);
  return <input {...merged} aria-label={props.label} />;
}

export function Selection(props: { id: string }) {
  const selection = createMemo(() => {
    const { id } = props;
    return id;
  });
  return <p>{selection()}</p>;
}
