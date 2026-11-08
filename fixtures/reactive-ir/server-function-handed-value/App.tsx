// ADR 0704: a server function read as a value reaches calls SC7007 never
// sees. Each reference outside server code is an obligation, blocker
// server-function-value-handed (flip witnesses b02-b05).
import { archive } from "./server-module";
import { save } from "./api";

const run = (f: (d: Date) => Promise<number>) => f(new Date());

export function App() {
  return (
    <button
      onClick={() => {
        // Positive: an alias.
        const s = save;
        void s(new Date());
        // Positive: a shorthand object slot.
        const o = { save };
        void o.save(new Date());
        // Positive: a higher-order argument.
        void run(save);
        // Positive: `.call`.
        void save.call(undefined, new Date());
        // Positive: a module-directive export handed on.
        void run(archive);
        // Negative: a direct call is the existing rule's (a Date: a violation).
        void save(new Date());
      }}
    >
      x
    </button>
  );
}

// Negative: a type query.
export type Save = typeof save;
