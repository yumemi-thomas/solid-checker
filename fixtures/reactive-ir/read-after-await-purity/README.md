# read-after-await-purity

SC1002 argument purity and the synchronous prefix (ADR 0348). Expectations: App.tsx header.

SC1001, ADR 0401: a read in a `runWithOwner` callback the computation reaches with no await before it is untracked and unwarned, so n01 to n03 (the `value()` read in the callback's synchronous prefix) and n05 (`value() > 5`; its second `value()` may follow the callback's own await, uncertifiable) each carry an SC1001 beside the header's "nothing reported", which counts SC1002 only. n04 and n06 run after an await, which is SC1002's.
