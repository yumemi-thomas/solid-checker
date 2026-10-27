// ai-memory-ui user-menu.tsx:95 (also login.tsx:58): onSettled calls a project
// function whose body, through leaf-fn-api.ts, holds a call the analysis cannot
// follow to its end; the result is reported at the outer call.
import { createSignal, onSettled } from "solid-js";
import { fetchCurrentUser, type CurrentUser } from "./leaf-fn-api";

const [currentUser, setCurrentUser] = createSignal<CurrentUser | null>(null);

function ensureCurrentUser() {
  return fetchCurrentUser().then((user) => setCurrentUser(user));
}

export function Avatar() {
  onSettled(() => {
    void ensureCurrentUser();
  });
  return <p>{currentUser()?.name ?? "guest"}</p>;
}
