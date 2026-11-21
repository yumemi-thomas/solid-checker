import { children, createContext, onCleanup } from "solid-js";
const Ctx = createContext<number>();
export function Host() {
  setTimeout(() => {
    const made = (
      <Ctx value={1}>
        {(onCleanup(() => {}), 1)}
      </Ctx>
    );
    children(() => made)();
    onCleanup(() => {});
  }, 0);
  return <p />;
}
