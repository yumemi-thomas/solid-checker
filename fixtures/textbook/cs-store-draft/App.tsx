import { For, createStore } from "solid-js";

export function Tags() {
  const [state, setState] = createStore({ owner: { name: "Ada" }, tags: ["a", "b"] });
  return (
    <div>
      <p>{state.owner.name}</p>
      <ul>
        <For each={state.tags}>{(tag) => <li>{tag}</li>}</For>
      </ul>
      <button
        type="button"
        onClick={() =>
          setState((draft) => {
            draft.owner.name = "Grace";
            draft.tags.push("c");
          })
        }
      >
        add
      </button>
      <button type="button" onClick={() => setState((draft) => ({ ...draft, tags: draft.tags.filter((tag) => tag !== "a") }))}>
        drop a
      </button>
    </div>
  );
}
