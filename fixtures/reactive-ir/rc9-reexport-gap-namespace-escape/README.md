# rc9-reexport-gap-namespace-escape

Fail-closed: the namespace object escapes into an array (any non-member-read reference escapes), so which export is reached cannot be bounded even though the one member read names a declared export. Expected: the `SC9014` notice keeps the re-export gap, with an evidence step at the escaping reference (`rc9-reexport-gap-none`).
