use super::*;

#[test]
fn return_cover_requires_a_candidate_on_every_normal_completion() {
    for (body, expected) in [
        ("return state;", Some(true)),
        ("if(flag) return state; else return state;", Some(true)),
        ("if(flag) return state; return state;", Some(true)),
        ("if(flag) throw 0; return state;", Some(true)),
        ("if(flag) return state;", Some(false)),
        ("if(flag) return; return state;", Some(false)),
        ("if(flag) return other; return state;", Some(false)),
        ("try { return state; } finally { return other; }", None),
        ("try { throw 0; } catch { return state; }", None),
        ("while(flag) { return state; }", None),
        ("switch(flag) { default: return state; }", None),
    ] {
        let source = format!("function test(flag) {{ {body} }}");
        let candidates = source
            .match_indices("return state;")
            .map(|(at, text)| {
                Span::new((at + "return ".len()) as u32, (at + text.len() - 1) as u32)
            })
            .collect::<Vec<_>>();
        assert_eq!(
            completion_return_cover(
                Path::new("test.js"),
                &source,
                Span::new(source.find('{').unwrap() as u32, source.len() as u32),
                &candidates,
            ),
            expected,
            "{body}"
        );
    }
}

fn cover(body: &str, targets: &[&str], dead: &[&str]) -> Option<bool> {
    let source = format!("function test(flag) {{ {body} }}");
    let spans = |needles: &[&str]| {
        needles
            .iter()
            .flat_map(|needle| {
                source
                    .match_indices(needle)
                    .map(|(start, _)| Span::new(start as u32, (start + needle.len()) as u32))
            })
            .collect::<Vec<_>>()
    };
    completion_call_cover(
        Path::new("test.js"),
        &source,
        Span::new(source.find('{').unwrap() as u32, source.len() as u32),
        &spans(targets),
        &spans(dead),
    )
}

#[test]
fn completion_cover_joins_both_branches_but_not_missing_or_early_paths() {
    assert_eq!(
        cover("if(flag) a(); else b();", &["a()", "b()"], &[]),
        Some(true)
    );
    assert_eq!(cover("if(flag) a();", &["a()"], &[]), Some(false));
    assert_eq!(
        cover("if(flag) a(); else other();", &["a()"], &[]),
        Some(false)
    );
    assert_eq!(
        cover(
            "if(flag) return; if(flag) a(); else b();",
            &["a()", "b()"],
            &[]
        ),
        Some(false)
    );
    assert_eq!(
        cover("if(flag) { a(); return; } else b();", &["a()", "b()"], &[]),
        Some(true)
    );
    assert_eq!(cover("if(flag) throw 0; a();", &["a()"], &[]), Some(true));
    assert_eq!(cover("throw 0; a();", &["a()"], &[]), Some(false));
}

#[test]
fn completion_cover_refuses_unmodeled_flow_and_nested_or_optional_calls() {
    for body in [
        "try { a(); } catch {}",
        "for(;;) a();",
        "switch(flag) { default: a(); }",
    ] {
        assert_eq!(cover(body, &["a()"], &[]), None);
    }
    for body in [
        "const later = () => a();",
        "flag && a();",
        "a?.();",
        "obj?.a();",
    ] {
        assert_eq!(cover(body, &["a()", "a?.()", "obj?.a()"], &[]), Some(false));
    }
}

#[test]
fn completion_cover_counts_a_whole_initializer_or_returned_call() {
    assert_eq!(cover("const x = a();", &["a()"], &[]), Some(true));
    assert_eq!(cover("const y = 1, x = a();", &["a()"], &[]), Some(true));
    assert_eq!(cover("return a();", &["a()"], &[]), Some(true));
    assert_eq!(
        cover("if(flag) return a(); b();", &["a()", "b()"], &[]),
        Some(true)
    );
    assert_eq!(
        cover("if(flag) return 1; return a();", &["a()"], &[]),
        Some(false)
    );
    assert_eq!(
        cover("const x = a(); for (const k in x) b(k);", &["a()"], &[]),
        Some(true)
    );
    assert_eq!(
        cover("for (const k in flag) b(k); const x = a();", &["a()"], &[]),
        None
    );
    for body in [
        "const x = flag && a();",
        "const x = () => a();",
        "return flag ? a() : 0;",
    ] {
        assert_eq!(cover(body, &["a()"], &[]), Some(false), "{body}");
    }
}

#[test]
fn completion_cover_reads_an_expression_bodied_arrow_as_its_one_statement() {
    let source = "const f = flag => a();";
    let call = source.find("a()").unwrap() as u32;
    let body = Span::new(call, call + 3);
    assert_eq!(
        completion_call_cover(Path::new("test.js"), source, body, &[body], &[]),
        Some(true)
    );
    let source = "const f = flag => flag && a();";
    let call = source.find("a()").unwrap() as u32;
    let start = source.find("flag &&").unwrap() as u32;
    assert_eq!(
        completion_call_cover(
            Path::new("test.js"),
            source,
            Span::new(start, call + 3),
            &[Span::new(call, call + 3)],
            &[]
        ),
        Some(false)
    );
}

#[test]
fn completion_cover_uses_dead_returns_only_as_extra_fallthrough() {
    assert_eq!(
        cover("if(flag) return; a();", &["a()"], &["return;"]),
        Some(true)
    );
    assert_eq!(
        cover("if(flag) return; else a();", &["a()"], &["return;"]),
        Some(false)
    );
}
