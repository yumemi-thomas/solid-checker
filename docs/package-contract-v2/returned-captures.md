# Explicit values retained by a returned callable

ADR 0256. This is additive to stable schemaVersion 1. It introduces no
generated or certified capture authority.

A whole returned-callable's call graph may carry a captures array. Each entry
has a distinct nonempty id and from names one exact source in the factory:
arg/path, resource/path, or operation/path. Callback from.capture/path names
that retained value in the returned graph. Ordinary from.arg continues to
name an invocation argument. A catalogue is not a parameter renumbering.

Captures are refused on factory graphs and effectful members. Recursive
captures, member classes, wildcard paths, dangling sources and capturing the
enclosing return itself are refused. The factory resources and operation ids
are separate from the returned graph's operation namespace. A capture cannot
select an invocation-local result by accident.

The consumer binds exact bare factory arguments for each immutable returned
instance. It never mutates AST facts or augments actual invocation arguments.
Private virtual callback slots and bound argument facts transport the value
to the existing callback consumer. Inline invocation requires an unguarded,
call-scoped positive lower bound, caller owner and ambient tracking. Deferred
invocation keeps deferred timing; unsupported capture owner/tracking contexts
withhold binding. Unknown callable implementations remain obligations.

Resource captures with explicit mandatory reads of the same factory resource
bind instance-locally. A factory operation result can do so only when a
mandatory unguarded same-stack producer explicitly outputs that resource's
accessor. Resource kind alone never proves a read or a callable. Unknown
operation results, resource-as-callback dispatch, paths, saved owners, state,
dictionary-selected values and parameter forwarding remain open. Reads use
the ordinary explicit reactive resource input vocabulary; no wildcard or
name-based substitution is introduced.

Each escape of a captured value, or of the returned function, remains a
reactive-dispatch-unresolved obligation: arrays, shorthand objects, aliases,
passing, returning, exporting, casts and JSX attribute values. For contracts
with captures this includes on* values. Old documents keep their old handler
exemptions and results. Binding is never derived from spelling; spelling can
only add an obligation for an unresolved reference in the declaration scope.

Canonical streams use semantic-captures:v1 only when a catalogue is nonempty.
The omitted/empty catalogue hashes exactly as before. Recipes in that family
bind the complete export graph, including capture source, context and count;
old recipe streams stay byte-identical. ValueSource tag 4 denotes capture.
Certification refuses captures as authored-only. Authoring requires installed
source citations at captureClosures[return-operation.capture-id], and records
captureProbeDigest over the complete claim, package, runtime, artifact cases,
pair metadata and pair bytes. A changed claim cannot reuse an old probe.

The scheduled replacement spec preserves the complete tracked spec and adds
debounce/throttle captures plus optional external dispatch. No createScheduled
or hostFree claim is added. createScheduled needs initial-state and transition
proofs: its schedule callback can set isDirty before the first invocation, and
an already-dirty invocation does not read track. Captures alone cannot prove
an unconditional read. Translator also needs callback-result provenance and
dynamic dictionary selection; its dict() capture alone cannot close dispatch.
