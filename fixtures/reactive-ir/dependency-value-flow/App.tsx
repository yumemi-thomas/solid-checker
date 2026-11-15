// ADR 0998: a value a package the build does not read returned is read as possibly reactive on every route to the read, not only where the read's own
// expression leads back to the call. `dep-router` is not read as source (its closure cannot be read), and `useLocation()` returns getters over a memo:
// Chrome rc.21 warns STRICT_READ_UNTRACKED at each untracked `.search` below (`fixtures/runtime-oracle/package-value-flow`).
import { createContext, useContext } from "solid-js";
import { Router, useLocation } from "dep-router";

// U (package-value-read:untracked, ADR 0981): the read's own expression leads back to the call.
function Direct() {
  const location = useLocation();
  const s = location.search;
  return <p>{s}</p>;
}

// U (package-value-read:flowed): the value goes through an array literal and a pattern.
function ViaArray() {
  const [location] = [useLocation()];
  const s = location.search;
  return <p>{s}</p>;
}

// U: a binding assigned after it is declared.
function ViaLet() {
  let location;
  location = useLocation();
  const s = location.search;
  return <p>{s}</p>;
}

// U: a slot of an object literal.
function ViaObject() {
  const o = { location: useLocation() };
  const s = o.location.search;
  return <p>{s}</p>;
}

// U: a parameter of a project function.
function show(l: { search: string }) {
  return l.search;
}
function ViaParam() {
  const s = show(useLocation());
  return <p>{s}</p>;
}

// U: a copy a standard function makes.
function ViaCopy() {
  const location = useLocation();
  const copy = Object.assign({}, { location });
  const s = copy.location.search;
  return <p>{s}</p>;
}

// no row: the same routes, read where an expression tracks the read.
function TrackedObject() {
  const o = { location: useLocation() };
  return <p>{o.location.search}</p>;
}
function TrackedLet() {
  let location;
  location = useLocation();
  return <p>{location.search}</p>;
}

// no row: a project function of the same name is no package, and its object has no accessor.
function Local() {
  const useLocation = () => ({ search: "?q=1" });
  const o = { location: useLocation() };
  const s = o.location.search;
  return <p>{s}</p>;
}

// ADR 1002: the value handed on as a JSX attribute is followed where the tag names its reader.
// U (package-value-read:flowed): a project component reads the prop untracked in its body.
function Child(props: { location: { search: string } }) {
  const s = props.location.search;
  return <p>{s}</p>;
}
function ViaProp() {
  const location = useLocation();
  return <Child location={location} />;
}

// U: the child takes its props by a pattern.
function ChildPattern({ location }: { location: { search: string } }) {
  const s = location.search;
  return <p>{s}</p>;
}
function ViaPropPattern() {
  const location = useLocation();
  return <ChildPattern location={location} />;
}

// U: a context provider's value, read by a consumer of the context.
const LocationContext = createContext<{ search: string }>();
function Consumer() {
  const location = useContext(LocationContext);
  const s = location.search;
  return <p>{s}</p>;
}
function ViaContext() {
  const location = useLocation();
  return (
    <LocationContext value={location}>
      <Consumer />
    </LocationContext>
  );
}

// no row: the child reads the prop where an expression tracks the read.
function ChildTracked(props: { location: { search: string } }) {
  return <p>{props.location.search}</p>;
}
function ViaPropTracked() {
  const location = useLocation();
  return <ChildTracked location={location} />;
}

export function App() {
  return (
    <Router>
      <Direct /><ViaArray /><ViaLet /><ViaObject /><ViaParam /><ViaCopy /><TrackedObject /><TrackedLet /><Local />
      <ViaProp /><ViaPropPattern /><ViaContext /><ViaPropTracked />
    </Router>
  );
}
