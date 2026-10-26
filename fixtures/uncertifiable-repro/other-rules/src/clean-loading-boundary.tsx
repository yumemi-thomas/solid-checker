// Clean. Mirrors every-deck-of-cards src/Layout.tsx:35-37 and expenses-app src/lib/current-user.tsx:28-34:
// a Loading boundary around children, with a fallback element and a non-keyed Show reading context-free state.
import { createMemo, Loading, Show } from "solid-js";
import type { JSX } from "@solidjs/web";

function Page(props: { children?: JSX.Element }) {
  const me = createMemo(async () => ({ name: "ada" }));
  return (
    <Loading fallback={<output>Loading page...</output>}>
      <Show when={me()}>{(user) => <header>{user().name}</header>}</Show>
      {props.children}
    </Loading>
  );
}

export function App() {
  return (
    <Page>
      <main>body</main>
    </Page>
  );
}
