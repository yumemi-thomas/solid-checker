# A component prop is a getter

ADR 0641. rc.21 warns `ASYNC_OUTSIDE_LOADING_BOUNDARY` from a render effect that hits the pending
value. `<Child value={memo()} />` compiles to a getter, so the child decides: it may render it, read
it in an effect, or never read it. SC5003 is uncertifiable there. On an intrinsic element, and on a
control-flow tag the dialect runs in a render, it stays proven. A member tag (`<props.Comp>`, `<ui.Unread>`) is a component whatever its head's case.
