# value-flow-review

Regression reproducers from the soundness reviews of ADRs 0209-0215 that were wrongly certified or reported. The program is closed (private `package.json`; ADR 0213/0214 discharges apply). Each case is `SC9012` except `GeneratorMethod` (calling a generator runs none of its body) and `Destructured` (the immutable helper is only called from tracked JSX): prototype keys, constructor returns, defaults, thenables, reassigned helpers, getter arguments, rejoined/overwritten/aliased/computed/redeclared values, patterns, bound getters. Each class uses its own method name.
