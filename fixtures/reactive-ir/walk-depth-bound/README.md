# The effect-phase walk's depth bound (ADR 0883)

Written JSX starts the walk, and not every step counts as a helper call, so the walk's
nesting has its own bound of 64 layers. Past it the walk answers `search-depth-limit`: an
uncertifiable SC6002 at the site, never a proof and never silent. rc.21 raises
PRIMITIVE_IN_EFFECT_CALLBACK on both, so the 70-layer site is a true obligation.
