import { createContext, resolve, useContext } from "solid-js";
const Ctx = createContext(7);
const fromRef = async () => {
  await Promise.resolve();
  useContext(Ctx);
};
export function App() {
  return <p ref={() => fromRef()}>x</p>;
}
