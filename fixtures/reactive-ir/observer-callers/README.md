# observer-callers

**Claim (ADR 0614).** `resolve` and `until` guard on the observer, so an action's first step answers to its callers and an IIFE in a flow child's JSX runs under the render effect: both are obligations rather than certified clean. Measured on rc.17: `FromTracked` and `InFlowJsx` throw `RESOLVE_IN_REACTIVE_SCOPE`; `FromHandler`, `PastYield` and `FlowHandler` report nothing.
