import { Errored } from "solid-js";
import { GuardedRouter } from "./routes";

export function MountGuarded() {
  return (
    <Errored fallback={(error) => <p>{String(error())}</p>}>
      <GuardedRouter />
    </Errored>
  );
}
