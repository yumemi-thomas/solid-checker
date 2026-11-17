import { createMemo, onSettled } from "solid-js";

function Router(props: { children: (route: { path: string }) => any }): any {
  onSettled(() => {
    props.children({ path: "/" });
  });
  return null;
}

export function App() {
  return (
    <Router>
      {(route) => {
        createMemo(() => route.path);
        return null;
      }}
    </Router>
  );
}
