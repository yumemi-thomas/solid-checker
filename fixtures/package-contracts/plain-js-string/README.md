# A locally created callback is not a caller-supplied one

`convertMap` passes `x => x` to `Array.prototype.map` and `convertString` calls
the built-in `String`. Both are exported functions whose bodies call into the
standard library, and neither takes a callback from its caller -- so both stay
`callable` with no callback operation.

The distinction this pins is against `runtime-semantics`, where the higher-order
built-ins are reached with a *parameter* (`Array.from(items, mapper)`) and do
produce an operation. Attributing the locally written arrow to argument 0 would
publish a callback obligation no consumer can satisfy.
