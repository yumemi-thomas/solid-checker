import { createMemo, createSignal } from "solid-js";
import { Router, RouterContext, useLocation, useRouter } from "dep-router";

// Candidate: the Get runs the package's reactive `search` getter in the
// component body, but nothing proves the object or its member to this read.
export function SearchNow() {
  const location = useLocation();
  const [query] = createSignal(location.search);
  return <p>{query()}</p>;
}

// Candidate: the same getter through the router value's member, inline.
export function SearchThroughRouter() {
  const search = useRouter().location.search;
  return <p>{search}</p>;
}

// Candidate: destructuring Gets the member too.
export function SearchDestructured() {
  const { search } = useLocation();
  return <p>{search}</p>;
}

// Candidate: a literal computed key names the same member.
export function SearchComputed() {
  const location = useLocation();
  const search = location["search"];
  return <p>{search}</p>;
}

// Clean: a same-named project hook is not the package's; exact identity
// decides, not the name.
function useLocalLocation() {
  return { search: "/" };
}
export function SearchShadowed() {
  const location = useLocalLocation();
  const [query] = createSignal(location.search);
  return <p>{query()}</p>;
}

// Clean: a tracked JSX read.
export function SearchTracked() {
  const location = useLocation();
  return <p>{location.search}</p>;
}

// Clean: a memo tracks the Get.
export function SearchMemo() {
  const location = useLocation();
  const search = createMemo(() => location.search);
  return <p>{search()}</p>;
}

// Clean: inert getters and a data member read nothing reactive.
export function Inert() {
  const router = useRouter();
  const pending = router.pending;
  const key = router.location.key;
  const base = router.base;
  return <p>{pending ? key : base}</p>;
}

// Clean: an event handler is no untracked-read role.
export function SearchOnClick() {
  const location = useLocation();
  return <button onClick={() => console.log(location.search)}>log</button>;
}

// Scope control: a getter object the project creates is not this change's
// subject; its Get stays as before.
function makeLocation() {
  const [path] = createSignal("/");
  return {
    get search() {
      return path();
    },
  };
}
export function ProjectGetter() {
  const location = makeLocation();
  const [query] = createSignal(location.search);
  return <p>{query()}</p>;
}

export function App() {
  return (
    <Router>
      <SearchNow />
      <SearchThroughRouter />
      <SearchDestructured />
      <SearchComputed />
      <SearchShadowed />
      <SearchTracked />
      <SearchMemo />
      <Inert />
      <SearchOnClick />
      <ProjectGetter />
    </Router>
  );
}

export function StaticProvider() {
  const router = useRouter();
  const value = { ...router, location: { ...router.location, search: "" } };
  return <RouterContext value={value}><SearchNow /></RouterContext>;
}
