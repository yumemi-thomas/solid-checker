# Lazy per-key getter consumer (draft)

Hand-stated authored contract. Copy this tree to
fixtures/reactive-ir/package-lazy-getter-consumer and add its node_modules
.gitignore exceptions. There is no acceptance catalog. Coverage must authorize
the copied tree via .solid-checker/authorize-contract.json.

reactive-package/package.json is byte-identical to
package-own-tracked-read-consumer. Its manifest/closure digests and import
record shape are preserved; only export names and contract meaning differ.
The synthetic package defines its own types. Real primitive probe pairs use
installed published typings separately.

TrackedJsx, MemoRead, StaticLiteral and TransparentReceiver must have no lazy
dispatch finding. UnprimedRead and PrimedRead each retain one uncertifiable
reactive-dispatch-unresolved obligation, never a violation: the initial model
does not prove initial or retained cache state. ComputedKey and DeferredAlias
also retain obligations. Escaped retains an obligation at construction and
at its member Get. StaticFunctionValue refuses the primitive-valued argument
proof. MutatedBinding and RetainedSetter have an obligation at construction.
ClearedUntrack remains an obligation: absence of a strict-read warning is not
permission to erase missing cache/resource facts. wrapSize retains an
obligation at the returned factory call; WrapperOpen must not acquire the
factory's argument-index/key recipe through an unproven project summary.
GenericInput retains its unknown initial-value/key obligations. NamespaceRead
is clean only if Type Facts resolves the exact namespace export; an unresolved
target must remain uncertifiable. ShadowedFactory is a plain local object and
must never inherit the package recipe by name.

All expected results are hand assertions, not observed snapshots. The lead
must run armed coverage/authorization before creating a snapshot. This draft
adds zero proven lazy-read violations.
