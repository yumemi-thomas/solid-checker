// ADR 0707: SSR audit D11 and D4, the rc.21 transport (`isJSONSafe`).
import * as api from "./api";
import { nothing, upload, zero } from "./api";

const z = -0;

export function App() {
  return (
    <button
      onClick={() => {
        // Violation before, uncertain now: `-0` is typed `0` and rc.21 rejects it.
        void zero(-0);
        void zero(z);
        // Clean: a literal zero, and a nonzero constant.
        void zero(0);
        void zero(1);
        // Uncertain (D4): a leading undefined is sent as null before a body.
        void upload(undefined, new Uint8Array(2));
        // Violation: an undefined-only argument with nothing after it throws.
        void nothing(undefined);
        // D9: a namespace member call is the exported server function: NaN is
        // typed `number` (uncertain); an undefined-only argument throws (violation).
        void api.zero(Number.NaN);
        void api.nothing(undefined);
      }}
    >
      x
    </button>
  );
}
