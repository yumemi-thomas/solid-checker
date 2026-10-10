---
status: accepted
---

# Controlled import-free TypeScript erasure

## Decision

Add `node-strip-import-free-esm-v1` beside ADR 0026's inert profile. It admits
one recipe-selected `creates` candidate from an import-free `.ts` module when a
native preservation validator proves that the derived module differs only by
blanking erasable TypeScript syntax. The pinned Node transformer must produce
those exact bytes. The original-source Type Facts census remains mandatory;
its authority is joined to the preservation witness and exact derived digest,
so it cannot be reused for another transform.

The result is still a checker-owned controlled execution. Its scoped receipt
cannot enter an ordinary contract catalog and does not assert compatibility
with an application's compiler, bundler, minifier, browser, or Node version.
A finite probe is only the mandatory contradiction veto after the native
census; it never supplies positive closure evidence.

## Measured basis

The POC completed finite samples for 26 of the original 40 TypeScript
candidates. A fresh production-census diagnostic isolates each of those 26 by
semantic claim. Fourteen reach their mandatory gate:

- 0.9.2: `addItemToArray`, `removeItemFromArray`, `isArray`, `isFunction`,
  `isNumber`, `contains`, `getDocument`, `isFrame`, `noop`, `clamp`,
  `snapValueToStep`, and `getEventPoint`;
- 2.0.0-alpha.0: `clamp` and `getPrecision`.

The other twelve refuse before probing: one indirect `CallableFunction.call`,
one recursive path, five missing complete helper transcripts, three coercion
forms, and two unknown element-accessor forms. These are census limitations,
not erasure or runtime contradictions. The diagnostic JSON is
`/private/tmp/import-free-census/results.json`, SHA-256
`d9a4f03650610ea50130800120f7c190c7201926afebb9b49a3bc3dcf42fe71c`.

## Preservation relation

The native parser accepts a complete ESM module only when:

1. it parses as TypeScript without recovery, hashbang, directives, static or
   dynamic imports, import-equals, namespaces, enums, decorators, JSX, or
   other transform-requiring syntax;
2. the selected export is directly declared in that module;
3. every changed byte lies in a parser-identified erasable type span, every
   removed non-newline byte becomes ASCII whitespace, line terminators and
   byte length are unchanged, and the result parses as JavaScript; and
4. pinned Node 24.11.1 strip-only output equals the native expected bytes.

The admitted type spans are annotations, type-parameter declarations and
instantiations, type aliases, interfaces, `as`/`satisfies`/type assertions,
non-null assertions, and optional parameter markers. Adding another syntax
class changes the preservation claim and requires a new profile identity.

This proves that the executable token stream and every source position outside
erased type spans are unchanged. The Type Facts implementation census over the
source therefore describes the exact executable structure loaded by the
controlled consumer. The receipt binds source and derived digests, the native
preservation identity, module format, selected export and claim, recipe plan,
dependency closure (empty), Node and harness identities, sandbox policy, and
producer session evidence.

## Consumer and refusal boundary

Only the checker can consume the capability, immediately and in the same
transaction. It re-runs the selected recipe in fresh workers after receipt
authentication, re-derives and compares the transform, and requires the same
gate identity. Existing consumers reject the new receipt/profile before
granting knowledge. A profile name or digest supplied by an application is
never evidence that its toolchain implements this interpretation.

Source reflection can observe blanked annotations, and another compiler can
print or lower different bytes. Such observations may differ across profiles;
the existing reflection counterexample therefore remains a required negative
control. Within this profile, the exact derived bytes are what both veto and
controlled consumer execute, and dynamic evaluation or unresolved invoking
forms still make the native census refuse. The scoped claim is defensible
because it says only that the selected `creates` closure held for this bound
derived execution under the native census and veto; it says nothing about a
different consumer.

## Identity changes

The worker protocol and sandbox policy scheme must advance because a worker
may now execute a non-inert derived module and replay its recipe as the
controlled consumer. The receipt version and signature domain must advance so
old consumers cannot interpret the broader preservation witness as ADR 0026's
inert result. Existing private workspace mode, environment clearing, process
group and `killpg`, frame count, primordial capture and freezing, exact
resolution checks, dependency authentication, and detect-and-refuse write
isolation remain mandatory.

The focused mismatch, contradiction and reflection controls pass. Fourteen of
the 26 measured import-free candidates pass the independent census and complete
the controlled profile; the final result-set SHA-256 is
`bb80a49e8b87932697e53ec64b149da1dce3140aabf8374a5c193be73b9caabb`.
