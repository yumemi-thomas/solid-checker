
import { saveEvent } from "./api";



export function Toolbar() {
  const when = new Date();
  const tags = new Set<string>();
  return (
    <button onClick={() => saveEvent(when, tags)}>save</button>
  );
}
