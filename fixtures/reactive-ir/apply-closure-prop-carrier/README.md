# A prop call in an inline carrier in an apply (ADRs 1222, 1229)

`[1].forEach(() => props.r1())` in an effect apply runs the callback once in place: the untracked read is a violation (`CSeeded`). `([] as number[]).forEach(...)` runs it zero times: uncertifiable (`CEmpty`, `function-unentered`). Chrome: `fixtures/runtime-oracle/apply-closure-prop`.
