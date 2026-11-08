# A literal forwarded to a denied event has a delivery, not no role

kui's `RangeSelector` shape: a literal on a component prop reaches an intrinsic `onChange` in a `<For>`. The walk proves only events reach
it, and one is denied, so the literal is uncertifiable. A first version let the refused proof fall through to no role, and this program
went silent: the corpus diff found it, and a change that only withholds must never remove an obligation.
