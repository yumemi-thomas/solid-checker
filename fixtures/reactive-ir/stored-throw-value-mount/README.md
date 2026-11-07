# stored-throw-value-mount

**Claim (ADR 0893).** An exported component mounted only as a value from another analyzed file is asked where that value goes: `Dynamic`'s `component` as a tag is, and any other value use is open. `Handled` and `Routed` are `computation-error-unobserved`; `Reported` (no boundary) and `Unused` (no value use anywhere) stay SC4002 violations.

Runtime: rc.21 Chrome, probe `cases/xf`: silent under the boundary, a halt with none. `Unused` is never mounted. The fuzz generator prints a site in one file only, so it cannot express a site in another file's exported component: this fixture holds the shape. Premises: the closed program is the `private` manifest.
