import { BodyDeclared } from "./App";

// ADR 1194: another module renders `BodyDeclared` by its tag: a component, whose body's role is its own (its violation stands).
export function Entry() {
  return <BodyDeclared />;
}
