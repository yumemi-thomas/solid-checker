import { unknownPrimitive } from "reactive-package";

export function App() {
  unknownPrimitive();
  return <div>unknown</div>;
}

export function plain() {
  return 1;
}
