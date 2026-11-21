import { createContext, createMemo } from "solid-js";
import { RenderApply } from "r1147r";
const Ctx = createContext<number>();
export function App() {
  return (
    <RenderApply
      render={() => (
        <Ctx value={1}>
          {createMemo(() => 1)()}
        </Ctx>
      )}
    />
  );
}
