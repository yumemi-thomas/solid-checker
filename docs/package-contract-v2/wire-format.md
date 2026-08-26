# Compact wire-format draft

This is a design draft for temporary `schemaVersion: 2`. The generated JSON
Schema remains the structural authority once implementation begins; cross-field
and proof invariants remain Rust validation responsibilities.

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

## Entrypoints

An entrypoint has either one unconditional artifact or an explicit case list.

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

The resolution trace is provenance for an already resolved artifact. The
contract does not implement package-export resolution by matching host/mode
tokens. Exactly one case must agree with the actual resolved import.

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
      "on": {
        "event": "call",
        "schedule": "same-stack"
      },
      "tracking": "tracked",
      "owner": {
        "source": "created",
        "resource": "effect-owner",
        "children": "allowed"
      },
      "count": { "min": 1, "max": 1 }
    },
    {
      "id": "apply",
      "kind": "invoke",
      "on": {
        "event": "flush"
      },
      "tracking": "untracked",
      "owner": {
        "source": "ambient-at-run"
      },
      "after": ["compute"],
      "count": { "min": 0, "max": "many" }
    }
  ]
}
```

The exact final spelling should be chosen for compactness after corpus
measurement. The semantic distinctions are mandatory even if the wire uses
shorter names.

## Recursive values

```json
{
  "kind": "tuple",
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
and disjoint.

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

## Structural limits

The validator must enforce configurable limits with initial safety defaults:

- main document at most 1 MiB;
- recursive shape/guard depth at most 32;
- bounded summary, export, operation, resource, and edge counts;
- bounded string and path lengths;
- no path traversal outside the package root;
- no reference cycles;
- linear-time expansion and normalization.

These are denial-of-service limits, not compactness targets.
