# A forbidden call in a leaf's IIFE is placed at the call (SC3001)

An IIFE the leaf callback runs in place puts its body on the leaf's stack, so a
forbidden call there runs in the leaf scope, and rc.21 raises the code from that
call's line (Chrome). The violation sits on the call, as it does for a call written
directly in the callback, not on the IIFE.

Past the placement's bound of 16 layers there is no answer: an obligation at the call.
