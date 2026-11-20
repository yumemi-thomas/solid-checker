import { For, createSignal, createStore } from "solid-js";

// ADR 1146: a `change` handler of a literal checkbox or radio is delivered by its click activation alone, so its body runs on the host's stack with no
// owner: the setter, and a flow row's `index()` read, are legal there.
export function Controlled() {
  const [done, setDone] = createSignal(false);
  const [choice, setChoice] = createSignal("a");
  return (
    <form>
      <input type="checkbox" checked={done()} onChange={(event) => setDone(event.currentTarget.checked)} />
      <input type="radio" name="g" checked={choice() === "a"} onChange={() => setChoice("a")} />
      <input type="radio" name="g" checked={choice() === "b"} onChange={() => setChoice("b")} />
      <input type="CheckBox" onChange={() => setDone(!done())} />
    </form>
  );
}

type Todo = { id: number; done: boolean };

export function Rows() {
  const [todos, setTodos] = createStore<Todo[]>([{ id: 1, done: false }]);
  return (
    <For each={todos}>
      {(todo, index) => (
        <input
          type="checkbox"
          checked={todo.done}
          onChange={() =>
            setTodos((draft) => {
              draft[index()].done = !draft[index()].done;
            })
          }
        />
      )}
    </For>
  );
}

// Writes the program makes to controls that are not a `type` write: other properties, and a key a proof names. They retype nothing.
export function touch(el: HTMLInputElement) {
  el.value = "x";
  el.checked = true;
  const key = "disabled";
  (el as unknown as Record<string, unknown>)[key] = false;
}
