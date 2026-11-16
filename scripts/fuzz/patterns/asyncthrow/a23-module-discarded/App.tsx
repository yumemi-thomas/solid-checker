import { createContext, useContext } from "solid-js";
const Ctx = createContext(7);
const boom = async () => {
  await Promise.resolve();
  useContext(Ctx);
};
export function App() {
  return <p ref={() => { boom(); }}>x</p>;
}
