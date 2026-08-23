# The claims an unresolved dispatch actually invalidates

The control half of the pair whose other half is
`unresolved-dispatch-attribution`. `Direct` has the same shape in both: it
invokes a callback parameter and returns an object built from `channelFor`'s
result. The only difference is that here `channelFor` resolves.

With the dispatch resolved, the generator claims the return outright:

```json
"returns": { "kind": "object", "properties": { "value": { "kind": "accessor" } } }
```

That is the claim the unresolved variant cannot make, and it is why an
unresolved dispatch has to mark `returns` unknown along with `reactiveReads`.
The returns description is derived from the same resolved callee summary, and
it does **not** fail closed on its own: with `channelFor` unresolved, nothing
would say the property is an accessor, so the whole `returns` field would be
omitted -- a certified-negative claim that the return carries nothing reactive.
`StructuredReturnUnresolved` is not the guard for this: it fires only for a
shorthand property bound to an import with no project declaration, which this
shape is not.

`callbacks` is claimed identically in both halves, which is the other half of
the pin: the obligation says nothing about the callback, and the unresolved
variant must keep that row rather than erase it along with everything else.

`inert` is the untouched control in both halves.
