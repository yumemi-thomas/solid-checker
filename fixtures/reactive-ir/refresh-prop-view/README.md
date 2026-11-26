# refresh-prop-view

**Claim (ADR 1214, slice A).** `refresh(view.m1)` on a `const` `merge(literal…, props)` or `omit(props, "k"…)` view of a component's props parameter, whose Get of `m1` is the parameter's own Get (`local_access::view_get_is_props_get`, ADR 1100's rc.21 view semantics), names `props.m1` (`static_api::props_target_slot`). With every caller passing one exact memo for `m1` (ADR 1174), the write is that memo's and is placed by the call's own role.

Violations: `MergeView`, `OmitView` (in the component body; main: SC2001 uncertifiable, `function-run-unplaced`). Clean: `HandlerView` (in an event handler). Uncertifiable (fail closed): `PropsFirst` (a later source may answer `m1`), `Rebound` (the shadowed variant: the view is a `let` written again), `ViewOfView` (the member variant: a view of a view).
