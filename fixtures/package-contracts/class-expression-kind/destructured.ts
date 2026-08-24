// An object pattern destructures a *member* of its initializer, so neither
// the class expression nor the class identifier says anything about the
// binding: both of these hold a string. No syntactic search could reason about
// that in either direction -- `const { Inner } = Container` is a static class
// member and really is a constructor that invokes its callback, while
// `const { name } = class Named {}` is a string -- so this entrypoint used to
// be refused outright, and these two correct `value` claims were the cost.
//
// The constructability fact answers the pattern directly: both bindings are
// `nonCallable` and `nonConstructable`, which is the full negative, so both
// publish `kind: "value"` and this entrypoint emits. It is the positive pin on
// that now.
//
// Still its own entrypoint: it was carved out when a refusal cost the
// entrypoint it sat on, and keeping it separate keeps this claim readable
// beside the class-expression proofs on `.`.
import { DependencyCache } from "bundled-dependency";

const { name: inlineCacheName } = class Named {};
const { name: dependencyCacheName } = DependencyCache;

export { dependencyCacheName, inlineCacheName };
