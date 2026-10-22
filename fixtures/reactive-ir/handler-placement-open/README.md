# handler-placement-open

ADR 0358: exact member/tuple and intrinsic-spread handlers seed SC4001 under an
open boundary. Lowercase keys install listeners; namespaced keys do not.
Props, mutation, getters and generators retain obligations. Clean only writes
a signal. App passes published rc.13 typings; local stubs model those APIs.
Twin: `handler-placement-closed`.
