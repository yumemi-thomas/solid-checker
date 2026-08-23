// An object pattern destructures a *member* of its initializer, so neither
// the class expression nor the class identifier says anything about the
// binding: both of these hold a string. That is exactly why the class search
// is gated off for a pattern (`identifier_binding_at`) -- and exactly why
// `nonCallable` cannot be published as `kind: "value"` here either, because
// `nonCallable` is a class type's answer too and nothing looked. A static
// class member reached this way (`const { Inner } = Container`) really is a
// constructor that invokes its callback.
//
// Its own entrypoint because a refusal costs the entrypoint it is on, and the
// three class-expression proofs on `.` are the point of this fixture.
import { DependencyCache } from "bundled-dependency";

const { name: inlineCacheName } = class Named {};
const { name: dependencyCacheName } = DependencyCache;

export { dependencyCacheName, inlineCacheName };
