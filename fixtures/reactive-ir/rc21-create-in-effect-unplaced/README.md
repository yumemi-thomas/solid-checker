# rc21-create-in-effect-unplaced

ADR 0710: `createMemo` or `createRoot` in an unplaced function (object or class method, getter) an apply reaches: SC6002 obligation, `function-run-unplaced` (Chrome throws). Inside `createRoot`/`runWithOwner`: silent. In the apply: violation.
