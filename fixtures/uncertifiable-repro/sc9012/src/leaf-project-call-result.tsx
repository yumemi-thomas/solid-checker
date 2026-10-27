// donegeon-client BoardControllerCore.tsx:311 (`const navigate = useNavigate()`): createTrackedEffect calls a
// function a hook returned, which is a function value, not a declaration the analysis can read.
import { createContext, createSignal, createTrackedEffect, useContext } from "solid-js";

type Navigate = (to: string, options?: { replace?: boolean }) => void;

const NavigateContext = createContext<Navigate>(() => {});

function useNavigate(): Navigate {
  return useContext(NavigateContext);
}

export function Redirect() {
  const navigate = useNavigate();
  const [target, setTarget] = createSignal("/");
  createTrackedEffect(() => {
    navigate(target(), { replace: true });
  });
  return <button onClick={() => setTarget("/next")}>go</button>;
}
