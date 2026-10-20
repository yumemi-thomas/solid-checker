import { createSignal } from "solid-js";
import {
  useClientQuery,
  useCyclicQuery,
  useDocumentQuery,
  useEarlyQuery,
  useGlobalThisQuery,
  useShadowedQuery,
  useWindowQuery,
  useWrittenQuery
} from "dep-query";

// Violation: `var isServer2 = typeof window === "undefined"`, read once set.
export function WindowQuery() {
  const query = useWindowQuery({ search: "a" });
  const [config] = createSignal(query.search);
  return <main>{config()}</main>;
}

// Violation: `var isClient = typeof window !== "undefined"`, negated.
export function ClientQuery() {
  const query = useClientQuery({ search: "a" });
  const [config] = createSignal(query.search);
  return <main>{config()}</main>;
}

// Violation: `"undefined" === typeof document`.
export function DocumentQuery() {
  const query = useDocumentQuery({ search: "a" });
  const [config] = createSignal(query.search);
  return <main>{config()}</main>;
}

// No proof: the alias is written.
export function WrittenQuery() {
  const query = useWrittenQuery({ search: "a" });
  const [config] = createSignal(query.search);
  return <main>{config()}</main>;
}

// No proof: `window` is the package function's own parameter.
export function ShadowedQuery() {
  const query = useShadowedQuery({ search: "a" });
  const [config] = createSignal(query.search);
  return <main>{config()}</main>;
}

// No proof: `globalThis.window` is not the realm's binding.
export function GlobalThisQuery() {
  const query = useGlobalThisQuery({ search: "a" });
  const [config] = createSignal(query.search);
  return <main>{config()}</main>;
}

// No proof: a module-level call before the alias may read it unset.
export function EarlyQuery() {
  const query = useEarlyQuery({ search: "a" });
  const [config] = createSignal(query.search);
  return <main>{config()}</main>;
}

// No proof: the alias's module is in an import cycle.
export function CyclicQuery() {
  const query = useCyclicQuery({ search: "a" });
  const [config] = createSignal(query.search);
  return <main>{config()}</main>;
}
