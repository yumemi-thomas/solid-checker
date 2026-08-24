// Control: a default-exported class *expression*. The parentheses are not a
// node the facts see, so the export records the class expression's own span —
// the same span `visit_class` recorded for it, so `declares_class_at` matches
// it too, redundantly: a class expression is a constructor by the same
// language definition, and ordinary `Constructability` already answers this
// correctly without it.
export default (class {});
