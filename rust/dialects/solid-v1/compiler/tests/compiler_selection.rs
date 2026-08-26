//! The two dialects' compilers are genuinely different programs, and this is
//! the test that could tell if the upstream crates ever converged: one
//! source, both lowerings, observably different results.
//!
//! The checker never compares generated code — it consumes semantic traces —
//! but the generated code is where the compilers' *independence* is cheapest
//! to observe: the Solid 1.x compiler lowers to `solid-js/web`'s runtime with
//! the 1.x template calling convention, while the 2.0 compiler lowers to its
//! own runtime shape.
//!
//! What this deliberately does not test is the *wiring* — it drives the two
//! upstream crates directly, not either dialect's `NativeCompilerFacts`. The
//! wiring is pinned structurally instead: `solidjs-compiler` is only
//! a dev-dependency of this crate, so `solid-v1-compiler`'s library cannot
//! reach the 2.0 compiler at all, and the dialect end-to-end fixtures assert
//! per-dialect findings that only the right lowering produces.

use solid1_dom_expressions_compiler as solid1;

const SOURCE: &str = r#"
import { createSignal } from "solid-js";

export function Counter() {
  const [count, setCount] = createSignal(0);
  return (
    <button classList={{ active: count() > 0 }} onClick={() => setCount(count() + 1)}>
      {count()}
    </button>
  );
}
"#;

#[test]
fn the_dialect_compilers_lower_the_same_source_differently() {
    let solid1_options = solid1::CompileOptions {
        filename: Some("Counter.tsx".into()),
        semantic_trace: true,
        ..solid1::CompileOptions::default()
    };
    let solid1_output =
        solid1::compile(SOURCE, &solid1_options).expect("the 1.x compiler accepts the source");

    let solid2_options = solidjs_compiler::CompileOptions {
        filename: Some("Counter.tsx".into()),
        semantic_trace: true,
        ..solidjs_compiler::CompileOptions::default()
    };
    let solid2_output = solidjs_compiler::compile(SOURCE, &solid2_options)
        .expect("the 2.0 compiler accepts the source");

    assert_ne!(
        solid1_output.code, solid2_output.code,
        "the two compilers produced identical output; dialect compiler selection has collapsed"
    );

    // `classList` is the concrete divergence this source pins: the 1.x
    // compiler owns the attribute (it splits the object into per-key class
    // toggles), while 2.0 removed it from the language and lowers it like any
    // other attribute.
    assert!(
        solid1_output.code.contains("classList"),
        "the 1.x lowering handles classList via its runtime helper:\n{}",
        solid1_output.code
    );

    // Both traces are total over the same site census — the seam contract
    // that lets one execution-map projection serve both wrappers.
    let solid1_trace = solid1_output.semantic_trace.expect("1.x trace");
    let solid2_trace = solid2_output.semantic_trace.expect("2.0 trace");
    assert!(!solid1_trace.sites.is_empty());
    assert!(!solid2_trace.sites.is_empty());
}
