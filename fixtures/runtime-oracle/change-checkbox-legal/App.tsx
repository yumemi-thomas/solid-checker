import { For, createSignal, createStore } from "solid-js";

// ADR 1146: a `change` handler of a literal checkbox or radio runs on the host's stack with no owner: setters and a row's `index()` read are legal
// there, and the dev build is quiet at every site.
export function Controlled() {
  const [done, setDone] = createSignal(false);
  const [choice, setChoice] = createSignal("a");
  return (
    <form>
      <input type="checkbox" checked={done()} onChange={(event) => setDone(event.currentTarget.checked)} />
      <input type="radio" name="g" checked={choice() === "a"} onChange={() => setChoice("a")} />
      <input type="radio" name="g" checked={choice() === "b"} onChange={() => setChoice("b")} />
      <p>{done() ? "done" : "open"} {choice()}</p>
    </form>
  );
}

type Todo = { id: number; title: string; done: boolean };

export function Rows() {
  const [todos, setTodos] = createStore<Todo[]>([{ id: 1, title: "one", done: false }, { id: 2, title: "two", done: true }]);
  return (
    <ul>
      <For each={todos}>
        {(todo, index) => (
          <li>
            <input
              type="checkbox"
              checked={todo.done}
              onChange={() =>
                setTodos((draft) => {
                  draft[index()].done = !draft[index()].done;
                })
              }
            />
            {todo.title}
          </li>
        )}
      </For>
    </ul>
  );
}
