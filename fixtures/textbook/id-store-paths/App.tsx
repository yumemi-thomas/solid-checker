import { createStore } from "solid-js";

export function Profile() {
  const [state, setState] = createStore({
    user: { name: "Ada", address: { city: "London" } },
    scores: [1, 2, 3],
    groups: [{ name: "a", members: ["x"] }],
  });
  return (
    <div>
      <p>
        {state.user.name} {state.user.address.city} {state.scores.join(",")} {state.groups[0].members.join(",")}
      </p>
      <button
        type="button"
        onClick={() =>
          setState((draft) => {
            draft.user.address.city = "Paris";
            draft.scores[1] = 20;
            draft.groups[0].members.push("y");
          })
        }
      >
        update
      </button>
    </div>
  );
}
