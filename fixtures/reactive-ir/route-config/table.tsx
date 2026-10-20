import { createSignal } from "solid-js";

const [count] = createSignal(0);

// Positive: a constant route table whose only uses are its export and the
// `routes` of a router in another file.
function Tabled() {
  const value = count();
  return <p>{value}</p>;
}

export const routes = [{ path: "/tabled", component: Tabled }];
