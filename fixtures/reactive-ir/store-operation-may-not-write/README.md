# store-operation-may-not-write

ADR 0718: a store `??=`/`||=`/`&&=`, `Object.assign`, `sort`/`reverse` is a violation only when proven to write, clean when a literal proves it does not, else `store-operation-may-not-write`.
