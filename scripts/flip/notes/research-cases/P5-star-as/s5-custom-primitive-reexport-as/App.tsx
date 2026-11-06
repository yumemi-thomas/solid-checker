import { Solid } from "./re";
export function App() {
  const [st] = Solid.createStore({ a: 1 }); st.a = 5;
  return <div />;
}
