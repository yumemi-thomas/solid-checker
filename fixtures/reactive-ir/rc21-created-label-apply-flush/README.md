# A flush the program may call under an owner keeps the apply's delivery open

ADR 1099's guard. The program also calls `flush()` from a handler, a position that names no ownerless caller, so a drain (and the
apply it runs) may be on a stack with an owner. The apply's delivery keeps the owner veto: SC4001 and SC4002 uncertifiable. (In
Chrome the handler's flush runs from a click with no owner, so this is a fail-closed trade: the answer is the one before the
flush census, which itself asks the dispatch question.)
