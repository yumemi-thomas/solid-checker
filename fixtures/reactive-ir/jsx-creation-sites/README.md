# JSX creation sites (ADR 0883)

Written JSX has no call node, so no owner rule saw it. rc.21, Chrome: an intrinsic's
dynamic child, attribute or spread is a render effect, NO_OWNER_EFFECT with no owner
(SC4001). In an effect apply it is PRIMITIVE_IN_EFFECT_CALLBACK (SC6002), and in a leaf
PRIMITIVE_IN_FORBIDDEN_SCOPE (SC3001); a fragment's child and a component element are
too. With no owner, a component element (its own root, ADR 0869) and a fragment child
(a memo) are quiet. A static child that may hold a function, and JSX under a component
element's getter, are uncertifiable.
