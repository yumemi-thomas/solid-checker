// An isolated package-shaped subject for the observation experiment. The
// signal is created here, is not passed by the recipe, and is not exposed.
// The setter is an ordinary public operation; there is no read counter or
// source-list API on the subject. Runtime bytes are the existing audited
// install, with production and development treated as separate experiments.
export async function loadSubject(profile) {
  const { createSignal, untrack } = profile === "development"
    ? await import("../../../rust/target/tsc-oracle/v2/node_modules/@solidjs/signals/dist/dev.js")
    : await import("../../../rust/target/tsc-oracle/v2/node_modules/@solidjs/signals/dist/prod/index.js");
  const [owned, setOwned] = createSignal(0);
  return {
    changeOwned(value) {
      setOwned(value);
    },
    readsOwned() {
      return owned();
    },
    readsOwnedButDiscardsValue() {
      owned();
      return 0;
    },
    readsOwnedUntracked() {
      return untrack(owned);
    },
    readsOwnedUntrackedButDiscardsValue() {
      untrack(owned);
      return 0;
    },
    noRead() {
      return 0;
    },
    invokesCaller(callback) {
      return callback();
    },
    readsCallerMember(caller) {
      return caller.value;
    }
  };
}
