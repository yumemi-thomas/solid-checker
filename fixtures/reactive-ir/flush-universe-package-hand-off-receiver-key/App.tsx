import { flush } from "solid-js";
import { runStep } from "steplib";

// The review of #229, B2 (P10) and ADR 0643's premise: a prototype written through a computed key no file spells (`"__pro" + "to__"`). A getter of the literal reads the prototype's
// getter through `this` in a step an installed package (`steplib`, its source is not read) runs; the flush keeps its three uncertifiable rows (`flush-in-action-unaccounted`,
// `flush-in-effect-unaccounted`, `leaf-operation-unaccounted`), never silence, never a violation. No file of the program spells the key, so the text test of the key's spelling
// (`proto_key_spelled`) says nothing here; the key the write stores under is any key, and a receiver reads one in code a package may run.
const protoKeyBase = {
  get g(): number {
    flush();
    return 1;
  },
};
const protoKey: any = {
  get h(): number {
    void runStep(() => {
      void this.g;
    });
    return 1;
  },
};
const protoKeyName = "__pro" + "to__";
protoKey[protoKeyName] = protoKeyBase;
setTimeout(() => {
  void protoKey.h;
});

export function App() {
  return <p>x</p>;
}
