import { useLocation, useRouter } from "dep-router";

declare const choose: boolean;
declare const opaque: { search: string };

export function BothKnown() {
  const location = choose ? useLocation() : useLocation();
  const search = location.search;
  return <p>{search}</p>;
}

export function UnknownBranch() {
  const location = choose ? useLocation() : opaque;
  const search = location.search;
  return <p>{search}</p>;
}

export function InertBranch() {
  const location = choose ? useLocation() : { search: "/" };
  const search = location.search;
  return <p>{search}</p>;
}

export function TrackedConditional() {
  const location = choose ? useLocation() : useLocation();
  return <p>{location.search}</p>;
}

export function DestructuredObject() {
  const { location } = useRouter();
  const search = location.search;
  return <p>{search}</p>;
}

export function RenamedObject() {
  const { location: current } = useRouter();
  const search = current.search;
  return <p>{search}</p>;
}

export function MutableObject() {
  let { location } = useRouter();
  const search = location.search;
  return <p>{search}</p>;
}

export function TrackedObject() {
  const { location } = useRouter();
  return <p>{location.search}</p>;
}
