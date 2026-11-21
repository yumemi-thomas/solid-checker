import { createContext, createEffect, createSignal, onCleanup } from "solid-js";
const Ctx = createContext<number>();
export function App() { const [n] = createSignal(0); createEffect(() => n(), () => {
      const made = (
        <Ctx value={1}>
          {(onCleanup(() => {}), 1)}
        </Ctx>
      );
      void made; }); return <p />; }
