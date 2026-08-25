# Solid 2 native callback contract variants (node)

This is the node half of the browser cases in `solid2-precision`. It pins the
exact `solid-js@2.0.0-rc.0` node export variants for `repeat`,
`createErrorBoundary`, and `createLoadingBoundary`. Those callbacks disagree
with the browser-default native table, so the package contract must refine
timing only after condition selection. The declarations reproduce the
published signatures used here; the source is valid TypeScript.
