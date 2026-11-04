# SC1001 and SC2003 through a `merge` view

rc.21's `merge` walks its sources last to first with `in`, and a last source that
covers the union is returned as it is. A key a literal holds after the store is plain
data; one the store holds is the store's read; anything else is open. A write through a
view of several sources is silent in Chrome (ADR 0630).
