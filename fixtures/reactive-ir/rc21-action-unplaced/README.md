# rc21-action-unplaced

ADR 0715: an action call in a function no call edge places (getter, tag, `Map` slot, constructor, instance field) is an SC2002 obligation; Chrome (rc.21) throws `ACTION_CALLED_IN_OWNED_SCOPE` at each. Kept: `Direct` violation. Silent: static block and field.
