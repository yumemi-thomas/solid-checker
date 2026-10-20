import { Show } from "solid-js";

// components-return-once (SC1004): one JSX tree, the condition inside it.
function LoginPrompt() {
  return <p>sign in</p>;
}
function Overview(props: { user: { name: string } }) {
  return <p>{props.user.name}</p>;
}
export function Dashboard(props: { user?: { name: string } }) {
  return (
    <Show when={props.user} fallback={<LoginPrompt />}>
      {(user) => <Overview user={user()} />}
    </Show>
  );
}
