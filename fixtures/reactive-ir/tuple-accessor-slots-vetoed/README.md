# tuple-accessor-slots-vetoed

**Claim (ADR 0801).** A program that patches array iteration has no exact tuple slot: `patched` keeps SC9012, and it warned ASYNC_OUTSIDE_LOADING_BOUNDARY in Chrome (rc.21). The veto is the project's, so one file stands for the arm. Every other spelling of an intrinsic is pinned one at a time by `only_a_value_of_an_array_intrinsic_reaches_iteration`.
