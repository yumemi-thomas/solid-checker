import { readShared } from "./helpers";
import { View, readTotal } from "./View";

export function App() {
  // Through View.tsx's export: uncertifiable when View.tsx is refused.
  const value = readTotal();
  // Independent of View.tsx: the violation stands.
  const shared = readShared();
  return <span>{value}{shared}<View /></span>;
}
