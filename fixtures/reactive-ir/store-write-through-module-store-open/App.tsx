import { direct } from "./stores";

// ADR 1232: the program is open, so the store's references in the project are not all of them: uncertifiable (rc.21 drops the
// write outside the setter here too; a recall miss of the premise).
export function Direct() {
  direct.a.v = 7;
  return <i />;
}
