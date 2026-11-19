import { For, createStore } from "solid-js";

export function Signup() {
  const [state, setState] = createStore({ entries: [] as string[], error: "" });
  return (
    <form
      onSubmit={(event) => {
        event.preventDefault();
        const data = new FormData(event.currentTarget);
        const email = String(data.get("email") ?? "");
        setState((draft) => {
          if (email.includes("@")) {
            draft.entries.push(email);
            draft.error = "";
          } else {
            draft.error = "invalid email";
          }
        });
      }}
    >
      <input name="email" />
      <button type="submit">join</button>
      <p>{state.error}</p>
      <ul>
        <For each={state.entries}>{(entry) => <li>{entry}</li>}</For>
      </ul>
    </form>
  );
}
