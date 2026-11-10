import { useContext } from "solid-js";
import * as Router from "dep-router-context";
import { RouterContext, RouterContext as Aliased } from "dep-router-context";

// Obligation (SC4002, `provider-not-proven-above`, ADR 0943): the package's own
// source creates this context with `createContext()` (no default), and nothing
// provides it above the read, so a render throws ContextNotFoundError. The
// providers of a package's context are not this program's to census, so the
// read is an obligation, never a violation and never silent.
function ReadBare() {
  useContext(RouterContext);
  return <i />;
}
export function Bare() {
  return <ReadBare />;
}

// The same read through an alias of the import: the same binding, the same
// obligation.
function ReadAliased() {
  useContext(Aliased);
  return <i />;
}
export function Alias() {
  return <ReadAliased />;
}

// A read through the namespace import names the binding by a member, so no
// provider or default is matched to it: the same obligation.
function ReadNamespace() {
  useContext(Router.RouterContext);
  return <i />;
}
export function Namespace() {
  return <ReadNamespace />;
}

// Silent: every render of the reading function sits under the provider.
function ReadProvided() {
  useContext(RouterContext);
  return <i />;
}
export function Provided() {
  return (
    <RouterContext value={{}}>
      <ReadProvided />
    </RouterContext>
  );
}

// Obligation: one render is under the provider and one is not.
function ReadTwice() {
  useContext(RouterContext);
  return <i />;
}
export function Twice() {
  return (
    <div>
      <RouterContext value={{}}>
        <ReadTwice />
      </RouterContext>
      <ReadTwice />
    </div>
  );
}
