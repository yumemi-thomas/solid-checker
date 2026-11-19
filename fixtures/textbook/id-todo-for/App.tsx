import { For, createSignal, createStore } from "solid-js";

type Todo = { id: number; title: string; done: boolean };

export function Todos() {
  const [todos, setTodos] = createStore<Todo[]>([]);
  const [title, setTitle] = createSignal("");
  let next = 1;
  const add = () => {
    const text = title().trim();
    if (!text) return;
    setTodos((draft) => {
      draft.push({ id: next++, title: text, done: false });
    });
    setTitle("");
  };
  return (
    <div>
      <input value={title()} onInput={(event) => setTitle(event.currentTarget.value)} />
      <button type="button" onClick={add}>
        add
      </button>
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
    </div>
  );
}
