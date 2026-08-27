# Compact wire format

Status: **implemented for temporary `schemaVersion: 2` on 2026-08-27**. The
checked-in [temporary JSON Schema](../../schema/solid-reactivity-contract-v2.schema.json)
is the structural authority; cross-field, normalization, and proof invariants
remain Rust validation responsibilities. Shorter aliases are not permitted
unless a later corpus measurement changes this document, schema, and decoder
atomically.

## Root

```json
{
  "format": "solid-reactivity-contract",
  "schemaVersion": 2,
  "semanticModelVersion": 1,
  "package": {
    "name": "example-package",
    "version": "1.2.3",
    "integrity": "sha512-...",
    "manifest": {
      "path": "package.json",
      "sha256": "..."
    }
  },
  "summaries": {},
  "entrypoints": {},
  "sidecars": {
    "proof": { "sha256": "..." },
    "probes": { "sha256": "..." }
  }
}
```

`format` distinguishes the replacement from the legacy schema that also uses
stable version 1 after the final renumber. There is no `schemaStatus`.

The root intentionally excludes generator identity, review status,
`compilerFactsProtocol`, embedded claim evidence, and trust labels.

`sidecars.proof.sha256` and `sidecars.probes.sha256` are 64-digit wire hashes
of the complete sidecar bytes. The backend canonicalizes them to normalized
`sha256:` digests and requires exact content matches during evidence
validation. A supplied sidecar without the corresponding main-document
reference is an orphan and is refused. Detailed evidence document shape and
the reverse semantic binding are defined in
[Proof, evidence, and acceptance](proof-and-evidence.md); neither sidecar is
part of normalized contract meaning or the ordinary analysis hot path.

## Entrypoints

An entrypoint has either one unconditional artifact or an explicit case list.
Both forms may carry an optional `transform` file identity and the exact case
may carry `stability: "experimental"`. A transform is a `{ path, sha256 }`
artifact and is omitted when no materialized transform participates.

```json
{
  ".": {
    "artifact": {
      "path": "./dist/index.js",
      "sha256": "...",
      "closureSha256": "..."
    },
    "declarations": {
      "path": "./dist/index.d.ts",
      "sha256": "..."
    },
    "exports": {
      "createThing": "create-thing"
    }
  }
}
```

Conditional example:

```json
{
  ".": {
    "cases": [
      {
        "resolution": {
          "runtimeBranch": "/exports/./browser/development/import/default",
          "typesBranch": "/exports/./browser/development/import/types"
        },
        "artifact": {
          "path": "./dist/dev.js",
          "sha256": "...",
          "closureSha256": "..."
        },
        "declarations": {
          "path": "./types/index.d.ts",
          "sha256": "..."
        },
        "exports": {}
      }
    ]
  }
}
```

The resolution trace is provenance for an already resolved artifact. The two
wire branches normalize to ordered semantic steps named `runtime` and `types`;
Phase 7 independently attests the full runtime/types branch traces and exact
per-export target bindings, then replaces these provisional public-name
bindings at the backend normalization boundary. The contract does not
implement package-export resolution by matching host/mode tokens. Exactly one
case must agree with the actual resolved import.
Artifact-case IDs are derived from exact entrypoint, trace, artifact,
declaration, transform, and closure identities at normalization and are not a
wire field.

## Export mappings and summaries

An export maps directly to one summary ID. Overrides are not supported.

```json
{
  "summaries": {
    "trigger-pair": {
      "shape": "callable",
      "call": {}
    }
  },
  "entrypoints": {
    ".": {
      "artifact": {},
      "declarations": {},
      "exports": {
        "createSignal": "trigger-pair"
      }
    }
  }
}
```

If two exports differ semantically they use different summaries. If behavior
branches on a verified call condition, guarded cases live inside the summary.

## Local closure

```json
{
  "call": {
    "closed": ["callbacks", "reads", "writes", "creates", "throws"],
    "callbacks": [
      {
        "from": { "arg": 0, "path": [] },
        "operation": "run"
      }
    ],
    "reads": [],
    "writes": [],
    "creates": [],
    "throws": []
  }
}
```

Here callbacks are completely known and the other listed domains are proved
absent. An unmentioned domain is unknown unless it has partial positive items.

Rules:

- `closed` names only immediate sibling set-valued domains;
- a closed name requires its collection;
- an open empty collection is invalid;
- duplicate domain names or duplicate items are invalid;
- closure never transfers through a summary reference;
- normalizer expansion occurs before closure is interpreted.

## Operations

```json
{
  "operations": [
    {
      "id": "compute",
      "kind": "invoke",
      "trigger": {
        "event": "call"
      },
      "at": { "event": "flush", "schedule": "queued" },
      "tracking": "tracked",
      "owner": {
        "source": "created",
        "resource": "effect-owner",
        "requires": "required",
        "children": "allowed",
        "cleanup": "supported",
        "lifetime": "resource"
      },
      "count": { "scope": "call", "min": 0, "max": "many" }
    },
    {
      "id": "apply",
      "kind": "invoke",
      "trigger": { "operation": "compute" },
      "at": { "event": "flush", "schedule": "same-stack" },
      "tracking": "untracked",
      "owner": {
        "source": "captured",
        "resource": "effect-owner",
        "requires": "required",
        "children": "forbidden",
        "cleanup": "supported",
        "lifetime": "resource"
      },
      "count": { "scope": "trigger", "min": 0, "max": 1 }
    }
  ],
  "edges": [
    {
      "kind": "orders",
      "from": "compute",
      "to": "apply"
    }
  ]
}
```

Operation `kind` is one of `invoke`, `return`, `read`, `write`, `invalidate`,
`create`, `cleanup`, or `dispose`. `trigger` names either an event, an operation,
or a resource event. `at.event` is one of `call`, `render`, `flush`, `settle`,
`transition`, `async-emission`, `cleanup`, `external-event`, `request`, or
`response-commitment`; `at.schedule` is `same-stack`, `queued`, or `external`.

`tracking` is `tracked`, `untracked`, or `ambient-at-execution`. Omission means
unknown. Owner `source` is `none`, `ambient-at-call`, `ambient-at-execution`,
`captured`, or `created`; `captured` and `created` require `resource`. Owner
requirements, child capability, cleanup capability, and lifetime are separate
keys and may be omitted independently when unknown.

`requires` constrains the current owner. `requiresChildren` and
`requiresCleanup` independently constrain child-owner and cleanup capability.
Owner production is a separate locally closed domain:

```json
{
  "closed": ["productions"],
  "productions": [
    {
      "resource": "effect-owner",
      "children": "allowed",
      "cleanup": "supported",
      "lifetime": "owner"
    }
  ]
}
```

Resource-bound lifetimes may use the owner/resource field already in scope, as
in `"lifetime": "owner"`, or the exact form
`{ "kind": "owner", "resource": "effect-owner" }`. The finite kinds are
`call`, `resource`, `owner`, `request`, `transition`, and `async-source`;
`call` never names a resource. A `count` whose scope is `resource` carries its
exact sibling `resource` ID.

`count.scope` is `trigger`, `call`, or `resource`. `min` and finite `max` are
non-negative integers; `max` may be `many`. A bound requires a scope, `min`
must not exceed finite `max`, and an omitted bound stays unknown. Graph edge
`kind` is `orders`, `data`, `invalidates`, `error`, `cleanup`, or `lifetime`.

## Resources

```json
{
  "resources": [
    {
      "id": "effect-owner",
      "kind": "owner",
      "states": ["active", "disposed"]
    }
  ]
}
```

Resource `kind` is `owner`, `reactive-source`, `async-computation`,
`transition`, `cleanup`, `request`, `response`, `stream`, or
`server-function-reference`. Only conclusion-relevant finite states are legal:
owner active/disposed; cleanup installed/disposed; async
pending/settled/errored/cancelled; transition active/settled/reverted; response
uncommitted/committed; stream unclaimed/claimed. State omission means unknown;
listing states without locally closing the corresponding state claim is
partial knowledge.

## Recursive values

```json
{
  "kind": "tuple",
  "closed": ["items"],
  "items": [
    {
      "kind": "reactive",
      "role": "accessor",
      "resource": "signal"
    },
    {
      "kind": "reactive",
      "role": "setter",
      "resource": "signal"
    },
    { "kind": "unknown" }
  ]
}
```

Object, tuple, array, and choice children carry independent knowledge. An
unknown leaf does not invalidate known siblings.

`closed` at a composite node names only that node's immediate `items`,
`properties`, or `alternatives` collection. A closed tuple item collection
proves exact length. An object property is `{ "name": ..., "value": ... }`;
closing properties proves there are no other statically visible own
properties. An array has one `element` shape and optional `length: { min, max }`;
element knowledge and length-interval knowledge are independent. The same local
closure rules as call domains apply: a closed name requires its collection,
and an open empty collection is invalid.

Version-1 value `kind` is `unknown`, `plain`, `parameter`, `tuple`, `array`,
`object`, `choice`, `callable`, `promise`, `async-iterable`, `reactive`,
`store`, `action`, `component`, `cleanup`, `ref-application`, or
`server-function-reference`. A reactive value has role `accessor` or `setter`.
Observable capabilities use only `readable`, `writable`, `refreshable`,
`pending-aware`, and `optimistic`, and must satisfy the contradictions rejected
by the semantic normalizer. A resource-bound capability may be written as
`{ "capability": "refreshable", "resource": "query" }`; intrinsic
`readable` and `writable` capabilities never name a resource. The local
`closed: ["capabilities"]` convention applies to reactive and store values.
Projection and snapshot are not nominal wire kinds.

## Guarded behavior

```json
{
  "cases": [
    {
      "when": {
        "all": [
          { "signature": "overload-2" },
          { "arg": 1, "path": ["effect"], "kind": "callable" }
        ]
      },
      "operations": ["apply-effect"]
    },
    {
      "otherwise": true,
      "operations": []
    }
  ]
}
```

Only the restricted, statically decidable atoms defined in
[semantic-model.md](semantic-model.md) are legal. An `otherwise` case can close
the partition only when the verifier proves every earlier guard well-formed
and disjoint. Atom order is normalized away. There is at most one
`otherwise`, it is last, and it means the complement of every preceding case;
case fallthrough and first-match semantics do not exist.

## Stability

Experimental status attaches to an export mapping or exact artifact case:

```json
{
  "ServerComponent": {
    "summary": "server-component",
    "stability": "experimental"
  }
}
```

Absence means unknown stability. A general `stable` default is not inferred.
`experimental` is the only version-1 stability value.

## Structural limits

The validator must enforce configurable limits with initial safety defaults:

- main document at most 1 MiB;
- recursive shape/guard depth at most 32;
- at most 1,024 entrypoints, 1,024 artifact cases, 16,384 summaries, and
  65,536 effective exports per package;
- at most 4,096 operations, 4,096 resources, 8,192 edges, 256 guarded cases,
  and 256 atoms per guard in one expanded summary;
- strings at most 16 KiB and package-relative paths at most 4 KiB;
- no path traversal outside the package root;
- no reference cycles;
- linear-time expansion and normalization.

The implementation additionally caps total expanded operation/resource/edge/
guard nodes at 1,000,000. This closes the multiplicative case where a document
under 1 MiB points tens of thousands of exports at one maximum-size summary.
The grammar permits summary references only from exports, so recursive summary
references are structurally impossible; unused and dangling references are
still rejected explicitly.

These are denial-of-service limits, not compactness targets. A policy may lower
them but may not silently raise them while accepting an existing receipt;
resource-limit policy is receipt-bound.
