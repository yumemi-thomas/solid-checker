# closed-export-argument-primitive-object

Negative twin of `closed-export-argument-primitive` (same closed program and helpers): a second tag renders `Label` with `text={{ x: 1 }}`, so `props.text` is not a primitive on every run and the census refuses. The `SC9012` `parameter-use-unmodeled` rows at `App.tsx:4` (`record(props.text)`) and `App.tsx:5` (`relay(props.text)`) stand. Stubs from `closed-export-member-dispatch`.
