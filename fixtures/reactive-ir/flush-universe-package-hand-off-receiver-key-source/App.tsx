import { flush } from "solid-js";
import { PROTO } from "dep-keys";

// The review of #229, S2: a key the program does not name (`PROTO`, exported by a package whose source the build reads) may be the prototype key, and the package's source is where
// the program spells it (no file of the project does: this text spells it nowhere). The text test of the key's spelling reads every file of the program, dependency source included, so a write through the key is a possible prototype
// install and the getter that flushes is handed: its three uncertifiable rows (`flush-in-action-unaccounted`, `flush-in-effect-unaccounted`, `leaf-operation-unaccounted`).
// Chrome (rc.21) is quiet where a timer reads it: the conservative answer of a net, never a violation.
const sourceKeyBase = {
  get g(): number {
    flush();
    return 1;
  },
};
const sourceKeyTarget: any = {};
sourceKeyTarget[PROTO] = sourceKeyBase;
setTimeout(() => {
  void sourceKeyTarget.g;
});

export function App() {
  return <p>x</p>;
}
