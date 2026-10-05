#[path = "support/diagnostics.rs"]
mod support;

use support::{assert_rule_findings, diagnostic_fixture, findings_for_rule};

#[test]
fn write_scope_diagnostics_have_semantic_locations() {
    let Some(findings) = diagnostic_fixture("write-scope") else {
        return;
    };
    // 12 writes / 2 actions: the untrack-wrapped writes in the component body
    // and in a memo count (the rc.0 guard keys on the owner, not tracking),
    // while writes and the action inside createTrackedEffect no longer do
    // (children-forbidden leaf scopes are legal write regions). The two store
    // setters directly in the component body count too: the fixture resolves
    // no solid-js, so it is analyzed as the audited release, whose store
    // setter guard rejects a root owner (ADR 0127; rc.13 keeps rc.9's
    // answer, ADR 0194).
    assert_eq!(
        (
            findings_for_rule(&findings, "reactive-write-in-owned-scope").len(),
            findings_for_rule(&findings, "action-called-in-owned-scope").len(),
        ),
        (14, 2)
    );
    assert!(
        findings
            .iter()
            .filter(|finding| {
                matches!(
                    finding["rule"].as_str(),
                    Some("reactive-write-in-owned-scope" | "action-called-in-owned-scope")
                )
            })
            .all(|finding| {
                finding["primaryLocation"]["path"]
                    .as_str()
                    .is_some_and(|path| path.ends_with(".tsx"))
                    && finding["message"].as_str().is_some_and(|message| {
                        message.contains("owned scope") || message.contains("action")
                    })
            })
    );
}

#[test]
fn diagnostic_domains_match_the_solid_two_matrix() {
    for (fixture, rules) in [
        (
            "leaf-owner",
            &[
                // Three owner-backed inline violations, two dynamic-extent
                // violations reached through exact helpers (registerTeardown
                // and the transitive indirectTeardown), plus the
                // exported-helper proof obligation. The two closed local
                // callback adapters are now included: their exact returned
                // function values reach the leaf scope and report SC3001 at
                // registerTeardown(). Silent, each for its own reason: the
                // out-of-band (event handler) onSettled, the helper call
                // written inside the event handler, and the helper that only
                // builds a nested function.
                ("leaf-owner-forbidden-call", 17),
                // Three inline (the function-seeded createSignal, createMemo,
                // createRoot) plus the dynamic-extent trackDouble() reached
                // through its helper.
                // Two inline plus flushNow() reached through its helper from
                // a block-bodied and an expression-bodied leaf callback.
                // The six returns that used to be reported here are the
                // domain `EffectFunction`'s `(() => void) | void` return type
                // rejects, so the value-legality rules are gone; the fixture
                // keeps them as sources because the ownership rules still
                // classify them.
                ("invalid-cleanup-return", 0),
                ("cleanup-return-unresolved", 0),
                // The local callback adapters have exact closed returns and
                // are no longer dispatch obligations.
                ("reactive-dispatch-unresolved", 0),
            ][..],
        ),
        (
            "static-api",
            &[
                // The valid deprecated one-argument overload, the same absent
                // apply slot behind an exact one-element tuple spread, plus
                // five cast-hidden non-callable runtime values (including a
                // bad EffectBundle.effect field). Raw invalid apply arguments
                // are TypeScript's diagnostics and stay silent.
                ("missing-effect-function", 7),
                // Signal-family only: the store constructors never route
                // options.sync into their node, so their three sync: true
                // async derives are negative cases now.
                ("sync-computation-received-async", 3),
                // The refresh/affects target rules were removed on
                // 2026-08-17: `Refreshable<T>` brands the target in the type
                // system, so every spelling this fixture writes -- wrapper,
                // literal, zero-arg, value-form store, store child record,
                // accessor member chain, and a key on an accessor -- is a
                // TS2345. Pinned at 0 so a reintroduction fails here.
                ("invalid-refresh-target", 0),
                ("invalid-affects-target", 0),
                ("affects-keys-on-accessor", 0),
                ("reactive-write-in-owned-scope", 1),
            ],
        ),
        (
            "directive-phases",
            &[
                // Building a directive value happens while the component
                // renders; only invoking the returned directive is in the
                // compiler's directive-application phase.
                ("reactive-write-in-owned-scope", 2),
                // Owner-attaching creations only: the direct createMemo, the
                // forwarded createEffect, and the function-form createSignal.
                // The three value-form createSignal(element) calls allocate
                // plain state that needs no owner and stay silent.
                ("primitive-in-directive-application", 3),
            ],
        ),
        ("owner-presence", &[("missing-owner", 14)]),
        (
            "async-boundary",
            &[
                // Four untracked reads: two plain async sources, the
                // declared-loadingValue source (still reported — the declared
                // window ends at the first real answer, so later re-asks
                // throw; conditional wording), and the opaque-options source
                // (downgraded to uncertifiable, asserted below).
                ("pending-async-unsuspendable-read", 7),
                // The declared sources (loadingValue memo, seedLoadingValue
                // projection and store) render bare without any SC5003: their
                // first flight never trips a Loading boundary. The
                // opaque-options render keeps the informational warning.
                ("async-outside-loading-boundary", 12),
            ],
        ),
        (
            "ssr-client-boundary",
            &[
                // Only the bare ssrSource: "client" read outside Loading in a
                // server-rendering project; the bounded, loadingValue, and
                // seedLoadingValue reads stay silent.
                ("async-outside-loading-boundary", 1),
            ],
        ),
    ] {
        let Some(findings) = diagnostic_fixture(fixture) else {
            return;
        };
        for (rule, expected) in rules {
            assert_rule_findings(&findings, rule, *expected);
        }
    }
}

#[test]
fn settled_leaf_rules_follow_call_site_ownership() {
    let Some(findings) = diagnostic_fixture("leaf-owner") else {
        return;
    };
    let cleanup = findings_for_rule(&findings, "leaf-owner-forbidden-call")
        .into_iter()
        .filter(|finding| {
            finding["message"]
                .as_str()
                .is_some_and(|message| message.starts_with("onCleanup"))
        })
        .collect::<Vec<_>>();
    // The out-of-band onSettled (event handler) must not carry any leaf-scope
    // finding: the runtime enqueues a plain callback there.
    assert!(
        findings
            .iter()
            .filter(|finding| finding["rule"] == "leaf-owner-forbidden-call")
            .all(|finding| {
                finding["message"]
                    .as_str()
                    .is_some_and(|message| !message.contains("OutOfBand"))
            }),
        "{findings:#?}"
    );
    // The exported helper's call sites are unknowable, so its onSettled leaf
    // finding is a proof obligation, not a proven violation; the owner-backed
    // component-body ones stay violations — three inline, three reached
    // through exactly-resolved dynamic-extent helpers, and two through the
    // closed local callback adapters.
    let kinds = cleanup
        .iter()
        .map(|finding| finding["kind"].as_str().unwrap_or_default())
        .collect::<Vec<_>>();
    assert_eq!(
        kinds.iter().filter(|kind| **kind == "violation").count(),
        8,
        "{cleanup:#?}"
    );
    assert_eq!(
        kinds
            .iter()
            .filter(|kind| **kind == "uncertifiable")
            .count(),
        1,
        "{cleanup:#?}"
    );
}

#[test]
fn solid2_precision_corrections_are_end_to_end() {
    let Some(findings) = diagnostic_fixture("solid2-precision") else {
        return;
    };
    let source = include_str!("../../../../fixtures/reactive-ir/solid2-precision/App.tsx");
    let start_of = |needle: &str| {
        source
            .find(needle)
            .unwrap_or_else(|| panic!("fixture landmark {needle:?}")) as u64
    };
    let starts = |rule: &str| {
        findings_for_rule(&findings, rule)
            .iter()
            .filter_map(|finding| finding["primaryLocation"]["startByte"].as_u64())
            .collect::<Vec<_>>()
    };
    let owner_starts = |message_prefix: &str| {
        findings_for_rule(&findings, "missing-owner")
            .iter()
            .filter(|finding| {
                finding["message"]
                    .as_str()
                    .is_some_and(|message| message.starts_with(message_prefix))
            })
            .filter_map(|finding| finding["primaryLocation"]["startByte"].as_u64())
            .collect::<Vec<_>>()
    };

    // Each count pins one side of a proof. SC1002: the accessor call and the
    // store member read inside the exact `Array#filter` callback, and nothing
    // for the Promise, shadowed, unresolved, or wrapper-built callbacks.
    // The cleanup-return *value* rules are gone (every illegal return is a
    // TypeScript error against the real `EffectFunction` signature), so the
    // contextual, explicit, parenthesized, `as`-cast, member, and returned-call
    // spellings this fixture still writes are pinned through the ownership
    // rules below instead of through a legality finding.
    // SC1001/SC2003: a plain store write is a write only,
    // while the compound and update forms also read their target and a
    // computed key stays a read. SC3001/SC4001/SC4001: the one owner-backed
    // settled cleanup written as a literal callback reports SC3001 without a
    // duplicate SC4001; the wrapper-built, identifier-referenced, and
    // out-of-band cleanups are SC4001 only. The returned-call block adds three
    // SC3004 (a produced `number` in both return spellings, plus the unowned
    // one), one SC9002 (`any`), and one SC4001 (a produced function is a real
    // cleanup); a produced function, `(() => void) | undefined`, and `void`
    // are legal and silent.
    for (rule, expected) in [
        ("reactive-read-after-await", 2),
        ("invalid-cleanup-return", 0),
        ("cleanup-return-unresolved", 0),
        ("strict-read-untracked", 5),
        // Still four, but not the same four: the 2026-08-17 narrowing traded the
        // three root-record writes for the nested, cast, and props writes
        // asserted by span below. A count alone cannot tell those apart.
        ("no-direct-mutation", 4),
        ("leaf-owner-forbidden-call", 1),
        // One proven returned cleanup plus four callbacks whose runtime
        // return may be a cleanup and therefore cannot be certified safe.
        ("missing-owner", 8),
    ] {
        assert_rule_findings(&findings, rule, expected);
    }

    // The leaf rules need the literal callback *and* the call's place in its
    // own synchronous extent. Past this landmark every leaf-owner call in the
    // fixture is wrapper-built, handed over as an identifier reference, or
    // has its onCleanup in a nested function the callback merely builds — no
    // leaf scope is proven at any of them, so SC3001 stops here while the
    // genuinely unowned SC4001 continues.
    let non_literal_leaf = start_of("// `wrap` may stash");
    assert!(
        starts("leaf-owner-forbidden-call")
            .iter()
            .all(|start| *start < non_literal_leaf),
        "a leaf callback that is not the literal argument proves no leaf scope"
    );
    assert_eq!(
        owner_starts("onCleanup")
            .iter()
            .filter(|start| **start > non_literal_leaf)
            .count(),
        3,
        "the wrapped, referenced, and out-of-band cleanups stay unowned"
    );

    assert!(
        !starts("strict-read-untracked").contains(&start_of("profile.name =")),
        "a plain assignment target is a write, not a read"
    );

    // 2.0's store proxy is shallowly `Readonly`, so a write to a *root*
    // property is TS2540 and no longer this checker's. The shallowness and the
    // cast escape hatch are what remain, and each is pinned by span -- the
    // count above is satisfied by four findings either way, so only these
    // assertions distinguish the narrowing from a regression that dropped the
    // wrong three.
    for root_write in ["profile.name = ", "profile.count += ", "profile.count++"] {
        assert!(
            !starts("no-direct-mutation").contains(&start_of(root_write)),
            "a root store property is TS2540, so {root_write:?} is TypeScript's to report"
        );
    }
    for kept in [
        // The readonly-ness stops at the top level.
        "profile.user.name = ",
        // A cast erases it, so TypeScript falls silent and the write is still
        // dropped. `member_root` resolves through the cast, which is why the
        // narrowing demands a bare-identifier root.
        "(profile as { user: { name: string } }).user = ",
        // A props member is not readonly at all.
        "props.n = ",
    ] {
        assert!(
            starts("no-direct-mutation").contains(&start_of(kept)),
            "{kept:?} is a write TypeScript accepts and the checker must still report"
        );
    }
    assert!(
        starts("strict-read-untracked").contains(&start_of("props.index")),
        "a computed key inside an assignment target is still a read"
    );

    // `filter(makePredicate(post => …))` hands the arrow to a wrapper, so the
    // accessor call inside it is not proven to run before the await resumes.
    let wrapped_callback = start_of("makePredicate((post)");
    assert!(
        starts("reactive-read-after-await")
            .iter()
            .all(|start| *start < wrapped_callback),
        "a wrapper-built filter callback is not a proven synchronous read"
    );

    // A returned call is still classified from its result, not from its
    // callable callee — the ownership half of that work is what survives the
    // removal of the legality rules. `return makeCount()` produces a number,
    // so no cleanup is handed over and SC4001 must not fire; the neighbouring
    // `onSettled(() => makeThunk())` produces a function and must.
    // Anchored on the module-level spellings: the returned expression is the
    // reported span, and both callees also appear inside the component above.
    let returned_call = start_of("makeCount();\n});");
    let unowned_thunk =
        start_of("onSettled(() => makeThunk())") + u64::try_from("onSettled(".len()).unwrap();
    assert!(
        !owner_starts("onSettled").contains(&returned_call),
        "a call producing a number hands the owner no cleanup to leak"
    );
    assert!(
        owner_starts("onSettled").contains(&unowned_thunk),
        "a call producing a function is an unowned returned cleanup"
    );

    // `return nothing` where `nothing: undefined` is a legal cleanup return
    // that hands the owner nothing, so it is not a returned cleanup that would
    // make these unowned callbacks SC4001.
    let cleanup_starts = owner_starts("onSettled");
    for typed_undefined in [
        start_of("return nothing;") + u64::try_from("return ".len()).unwrap(),
        start_of("=> nothing);") + u64::try_from("=> ".len()).unwrap(),
    ] {
        assert!(
            !cleanup_starts.contains(&typed_undefined),
            "a proven-`undefined` return is not an unowned cleanup"
        );
    }
}

#[test]
fn declared_first_paint_and_opaque_options_split_the_async_rules() {
    let Some(findings) = diagnostic_fixture("async-boundary") else {
        return;
    };
    // Fail-honest policy: SC5001 stays a proven violation when the options
    // argument is absent or readable, and downgrades to a proof obligation
    // when an unreadable options argument could declare a loadingValue.
    let untracked = findings_for_rule(&findings, "pending-async-unsuspendable-read")
        .into_iter()
        .filter(|finding| finding["severity"] == "error")
        .collect::<Vec<_>>();
    let kinds = untracked
        .iter()
        .map(|finding| finding["kind"].as_str().unwrap_or_default())
        .collect::<Vec<_>>();
    assert_eq!(
        kinds.iter().filter(|kind| **kind == "violation").count(),
        3,
        "{untracked:#?}"
    );
    assert_eq!(
        kinds
            .iter()
            .filter(|kind| **kind == "uncertifiable")
            .count(),
        1,
        "{untracked:#?}"
    );
    // The declared source keeps SC5001/SC5001 with conditional wording: the
    // first flight cannot throw, later re-asks can (probed against rc.0).
    assert!(
        untracked.iter().any(|finding| {
            finding["message"].as_str().is_some_and(|message| {
                message.contains("declares a loadingValue")
                    && message.contains("after the first real answer lands")
            })
        }),
        "{untracked:#?}"
    );
    // No declared source may carry the boundary warning: the declared first
    // paint is exactly what makes a Loading boundary unnecessary.
    assert!(
        findings_for_rule(&findings, "async-outside-loading-boundary")
            .iter()
            .all(|finding| {
                finding["message"].as_str().is_some_and(|message| {
                    !message.contains("declaredFeed")
                        && !message.contains("seededUser")
                        && !message.contains("seededStoreUser")
                })
            }),
        "{findings:#?}"
    );
}

#[test]
fn props_container_and_callback_creation_do_not_prove_nested_behavior() {
    let Some(findings) = diagnostic_fixture("callee-callback-timing") else {
        return;
    };
    let props = findings
        .iter()
        .filter(|finding| {
            finding["primaryLocation"]["path"]
                .as_str()
                .is_some_and(|path| path.ends_with("/Props.tsx"))
        })
        .cloned()
        .collect::<Vec<_>>();
    let reads = findings_for_rule(&props, "strict-read-untracked");
    assert_eq!(reads.len(), 1, "{props:#?}");
    // ADR 0199: `Handler` invokes the prop only inside a `<button>` click
    // handler, so both callback-prop literals run on dispatch and report
    // nothing. The consumers that leave such a read uncertifiable are
    // pinned by `forwarded-event-prop`.
    for line in [10, 15] {
        assert!(
            !reads
                .iter()
                .any(|read| read["primaryLocation"]["line"] == line),
            "{reads:#?}"
        );
    }
    let eager = reads
        .iter()
        .find(|read| read["primaryLocation"]["line"] == 21)
        .expect("eager component-body read");
    assert_eq!(eager["kind"], "violation", "{eager:#?}");
    let mutations = findings_for_rule(&props, "no-direct-mutation");
    assert_eq!(mutations.len(), 1, "{props:#?}");
    assert_eq!(mutations[0]["primaryLocation"]["line"], 36);
    assert_eq!(mutations[0]["kind"], "violation");
}

#[test]
fn pending_callback_reads_preserve_callee_invocation_uncertainty() {
    let Some(findings) = diagnostic_fixture("callee-callback-timing") else {
        return;
    };
    let pending = findings_for_rule(&findings, "pending-async-unsuspendable-read");
    assert_eq!(pending.len(), 7, "{pending:#?}");
    assert_eq!(
        pending
            .iter()
            .filter(|finding| finding["kind"] == "violation")
            .count(),
        4,
        "{pending:#?}"
    );
    let open = pending
        .iter()
        .filter(|finding| finding["kind"] == "uncertifiable")
        .collect::<Vec<_>>();
    assert_eq!(open.len(), 3, "{pending:#?}");
    assert!(
        open.iter().all(|finding| {
            finding["message"].as_str().is_some_and(|message| {
                (message.contains("pending handling are unproven")
                    || message.contains("strict-read window is unproven"))
                    && !message.contains("throws PENDING_ASYNC_UNTRACKED_READ")
            })
        }),
        "{open:#?}"
    );
}

#[test]
fn development_models_bind_exact_sources_and_primitive_symbols() {
    let Ok(typefacts) = std::env::var("SOLID_TYPEFACTS_BIN") else {
        return;
    };
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let fixture = root.join("fixtures/reactive-ir/callee-callback-timing");
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_solid-checker-rust"))
        .env("SOLID_TYPEFACTS_BIN", typefacts)
        .args(["--format", "json", "--feedback-facts", "--project"])
        .arg(fixture.join("tsconfig.json"))
        .output()
        .expect("run development model analysis");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let snapshot: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let models = snapshot["feedbackFacts"]
        .as_array()
        .expect("requested source models");
    let source = models
        .iter()
        .find(|model| model["path"].as_str().unwrap().ends_with("/Async.tsx"))
        .unwrap();
    let text = std::fs::read_to_string(fixture.join("Async.tsx")).unwrap();
    assert_eq!(
        source["sourceSha256"],
        solid_facts::core::SourceHash::of(&text).to_string()
    );
    let origins = source["functions"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|function| !function["derivedOrigin"].is_null())
        .collect::<Vec<_>>();
    assert_eq!(origins.len(), 8, "{origins:#?}");
    for function in origins {
        let origin = &function["derivedOrigin"];
        let call = &text
            [origin["start"].as_u64().unwrap() as usize..origin["end"].as_u64().unwrap() as usize];
        assert!(
            call.starts_with("createMemo(") || call.starts_with("Solid.createMemo("),
            "{call}"
        );
        assert_eq!(function["asynchronous"], true);
    }
    let shadowed = models
        .iter()
        .find(|model| model["path"].as_str().unwrap().ends_with("/Feedback.ts"))
        .unwrap();
    assert!(
        shadowed["functions"]
            .as_array()
            .unwrap()
            .iter()
            .all(|function| function["derivedOrigin"].is_null())
    );
    let text = std::fs::read_to_string(fixture.join("Feedback.ts")).unwrap();
    for (name, expected) in [("discarded", "open"), ("used", "return-expression")] {
        let child = shadowed["functions"]
            .as_array()
            .unwrap()
            .iter()
            .find(|function| {
                let parent = &function["parent"];
                parent["start"]
                    .as_u64()
                    .zip(parent["end"].as_u64())
                    .is_some_and(|(start, end)| {
                        text[start as usize..end as usize].starts_with(&format!("function {name}("))
                    })
            })
            .unwrap();
        assert_eq!(child["allocationRelevance"], expected);
    }
}

#[test]
fn ssr_client_hole_distinguishes_proven_and_unresolved_server_rendering() {
    let Some(findings) = diagnostic_fixture("ssr-client-boundary") else {
        return;
    };
    let holes = findings_for_rule(&findings, "async-outside-loading-boundary");
    assert_eq!(holes.len(), 1, "{findings:#?}");
    // The visible server entry proves the application server-renders, but no
    // render chain is traced from a mount root to `BadWidget` (server
    // renderers are not modelled as mount roots), so the missing boundary
    // above it is not proven: uncertifiable, never a violation.
    assert_eq!(holes[0]["kind"], "uncertifiable", "{holes:#?}");
    // The server throw is unconditional, so the rule mirrors it as an error.
    assert_eq!(holes[0]["severity"], "error", "{holes:#?}");
    assert!(
        holes[0]["message"]
            .as_str()
            .is_some_and(|message| message.contains("ssrSource: \"client\"")),
        "{holes:#?}"
    );
    // With no visible server entry, the same source is not certified safe:
    // the entry may live in another tsconfig/package.
    let Some(csr_findings) = diagnostic_fixture("ssr-client-boundary-csr") else {
        return;
    };
    let unresolved = findings_for_rule(&csr_findings, "async-outside-loading-boundary");
    assert_eq!(unresolved.len(), 1, "{csr_findings:#?}");
    assert_eq!(unresolved[0]["kind"], "uncertifiable", "{unresolved:#?}");
}

/// The wave-6 server-surface and resolve rules, pinned at their probed
/// gates: SC7005's server-render + Loading-children dominance, SC7006's
/// module-directive export shapes, SC7007's enableRichArguments silence, and
/// SC2004's and SC2005's observer-keyed scope split.
#[test]
fn server_surface_and_resolve_rules_pin_their_probed_gates() {
    if let Some(findings) = diagnostic_fixture("http-response-flush") {
        let drops = findings_for_rule(&findings, "http-response-after-flush");
        // The two component-body calls below the Loading boundary plus the
        // lexical call in its children; the shell, fallback, and
        // event-handler calls stay silent.
        assert_eq!(drops.len(), 3, "{findings:#?}");
        assert!(
            drops.iter().all(|finding| {
                finding["severity"] == "warning" && finding["kind"] == "uncertifiable"
            }),
            "the post-flush race must remain explicitly uncertifiable: {drops:#?}"
        );
        // With no visible server entry, the rendering mode is unresolved;
        // absence of the import is not proof that the app is CSR-only.
        if let Some(csr) = diagnostic_fixture("http-response-flush-csr") {
            let unresolved = findings_for_rule(&csr, "http-response-after-flush");
            assert_eq!(unresolved.len(), 2, "{csr:#?}");
            assert!(
                unresolved
                    .iter()
                    .all(|finding| finding["kind"] == "uncertifiable"),
                "{unresolved:#?}"
            );
        }
    }
    if let Some(findings) = diagnostic_fixture("server-function-directive") {
        // Two wrapped exports, one named re-export, one star re-export, one
        // wrapped default export; the direct function exports stay silent.
        assert_rule_findings(&findings, "server-function-module-directive", 5);
        assert!(
            findings.iter().all(|finding| {
                !finding["primaryLocation"]["path"]
                    .as_str()
                    .is_some_and(|path| path.ends_with("plain.ts"))
            }),
            "a module without the directive puts nothing at risk: {findings:#?}"
        );
    }
    if let Some(findings) = diagnostic_fixture("server-function-rich-args") {
        // Fifteen proven rich/non-JSON primitive values -- thirteen at the
        // top level plus two witnessed inside a closed object literal, since
        // JSON.stringify reaches nested values -- and four explicit
        // obligations. Lone/trailing Uint8Array, compiler-proven JSON-safe
        // primitive domains, and object graphs closed against spreads,
        // computed and duplicate keys, and accessors are all certified.
        //
        // The four obligations are each irreducible for a different reason,
        // and every one is pinned: a broad `number` may be non-finite; a
        // getter's body runs on access, so no written value proves what the
        // property yields; a binding referenced twice could be mutated before
        // the call; and a spread could overwrite the witness.
        assert_rule_findings(&findings, "server-function-rich-argument", 19);
        assert_eq!(
            findings_for_rule(&findings, "server-function-rich-argument")
                .iter()
                .filter(|finding| finding["kind"] == "uncertifiable")
                .count(),
            4,
            "{findings:#?}"
        );
        if let Some(enabled) = diagnostic_fixture("server-function-rich-args-enabled") {
            assert!(
                enabled.is_empty(),
                "enableRichArguments installs the codec and removes the throw everywhere: {enabled:#?}"
            );
        }
    }
    if let Some(findings) = diagnostic_fixture("resolve-scope") {
        // Memo compute, effect compute, createTrackedEffect, tracked JSX;
        // untrack, component body, event handler, apply, and module scope
        // are observer-free and stay silent (probed, rc.0).
        assert_rule_findings(&findings, "resolve-in-tracked-scope", 4);
        assert!(
            findings
                .iter()
                .all(|finding| finding["rule"] == "resolve-in-tracked-scope"),
            "{findings:#?}"
        );
    }
    if let Some(findings) = diagnostic_fixture("rc9-until-scope") {
        // rc.9's `until` carries resolve's observer guard, so the same four
        // tracked scopes throw and the same observer-free ones -- plus the
        // action step rc.9 documents -- stay silent.
        assert_rule_findings(&findings, "until-in-tracked-scope", 4);
        // rc.9 is older than the audited rc.13 (ADR 0194): one SC9014 notice
        // beside them.
        assert_eq!(
            findings_for_rule(&findings, "unaudited-solid-release").len(),
            1,
            "{findings:#?}"
        );
        assert!(
            findings.iter().all(|finding| {
                (finding["rule"] == "until-in-tracked-scope" && finding["kind"] == "violation")
                    || finding["rule"] == "unaudited-solid-release"
            }),
            "{findings:#?}"
        );
    }
    if let Some(findings) = diagnostic_fixture("rc9-static-dynamic-async") {
        // rc.9's static dynamic() form throws in dev on a thenable source and
        // renders nothing in production. Four inline async sources, the
        // standard library's Promise.resolve and Promise construct signature,
        // and two identifiers resolved to same-file async functions; the
        // synchronous, default-form, unknown-form, shadowed, parameter, `let`
        // and block-bodied sources stay silent.
        assert_rule_findings(&findings, "static-dynamic-async-source", 8);
        assert!(
            findings.iter().all(|finding| {
                (finding["rule"] == "static-dynamic-async-source" && finding["kind"] == "violation")
                    || finding["rule"] == "unaudited-solid-release"
            }),
            "{findings:#?}"
        );
        // rc.3 has no static form: the same calls are the memo's async
        // compute, which settles the Promise.
        if let Some(rc3) = diagnostic_fixture("release-triple-static-dynamic-async-rc3") {
            // Only the older-release notice (ADR 0127).
            assert!(
                rc3.iter()
                    .all(|finding| finding["rule"] == "unaudited-solid-release"),
                "{rc3:#?}"
            );
            assert_eq!(rc3.len(), 1, "{rc3:#?}");
        }
    }
    // SC2006 is keyed on the resolved @solidjs/signals: the FLUSH_IN_ACTION
    // guard ships from rc.8, so the same positives report on rc.8 and rc.9
    // and nothing reports on rc.3, where flush drains inside a
    // step as anywhere else.
    let line = |finding: &serde_json::Value| finding["primaryLocation"]["line"].as_u64();
    for (fixture, lines, notice) in [
        ("rc9-flush-in-action", &[14_u64, 20, 27, 32, 39, 44][..], 1),
        ("release-triple-flush-rc8", &[9, 15][..], 1),
        ("release-triple-flush-rc3", &[][..], 1),
    ] {
        let Some(findings) = diagnostic_fixture(fixture) else {
            continue;
        };
        let flushes = findings_for_rule(&findings, "flush-in-action");
        assert_eq!(
            flushes
                .iter()
                .map(|finding| line(finding))
                .collect::<Vec<_>>(),
            lines.iter().copied().map(Some).collect::<Vec<_>>(),
            "{fixture}: {findings:#?}"
        );
        assert!(
            flushes
                .iter()
                .all(|finding| finding["id"] == "SC2006" && finding["kind"] == "violation"),
            "{fixture}: {findings:#?}"
        );
        // Beside them only the release notice, which every one of these
        // triples gets: all are older than the audited rc.13 (ADR 0194).
        assert_eq!(
            findings_for_rule(&findings, "unaudited-solid-release").len(),
            notice,
            "{fixture}: {findings:#?}"
        );
        assert_eq!(
            findings.len(),
            lines.len() + notice,
            "{fixture}: {findings:#?}"
        );
    }
    if let Some(findings) = diagnostic_fixture("uncalled-accessor-v2") {
        // The positions TypeScript permits: a string-concatenation operand, a
        // logical-not operand, the two unary numeric coercions (`-count` and
        // `~count`, both clean against the published typings), and a template
        // interpolation. The typed positions the 2026-08-17 narrowing dropped
        // -- a class object value, a native attribute, and a computed key --
        // are each a diagnostic of TypeScript's own, and the children
        // attribute and the called and passed-on accessors were already
        // silent. Binary arithmetic and bitwise operands stay silent because
        // TypeScript rejects a function there (TS2365/TS2362).
        assert_rule_findings(&findings, "uncalled-accessor", 5);
        for position in [
            "string concatenation",
            "logical-not operator",
            "numeric coercion",
            "template literal",
        ] {
            assert!(
                findings.iter().any(|finding| {
                    finding["message"]
                        .as_str()
                        .is_some_and(|message| message.contains(position))
                }),
                "{position} is a position TypeScript permits and must stay reported: {findings:#?}"
            );
        }
        for typed in [
            "class object value",
            "native JSX attribute",
            "computed property access",
        ] {
            assert!(
                findings.iter().all(|finding| {
                    !finding["message"]
                        .as_str()
                        .is_some_and(|message| message.contains(typed))
                }),
                "{typed} is TypeScript's; reporting it duplicates a diagnostic: {findings:#?}"
            );
        }
    }
}

/// rc.9's two new callback forms whose runtime the call shape selects.
///
/// `dynamic(source, { static: true })` is `untrack(source)` at the call, so it
/// owns and writes exactly as `untrack` does; an unproven options value claims
/// nothing. `omit(props, hidden)`'s predicate runs on reads of the returned
/// view, so nothing written inside it is placed in the component body, and a
/// predicate not proven inert is uncertifiable at the predicate argument.
#[test]
fn rc9_call_forms_follow_the_runtime_they_select() {
    let line = |finding: &serde_json::Value| finding["primaryLocation"]["line"].as_u64();
    if let Some(findings) = diagnostic_fixture("rc9-dynamic-static") {
        let owners = findings_for_rule(&findings, "missing-owner");
        // StaticEffect, NamespaceStaticEffect, and the `untrack` reference.
        assert_eq!(
            owners
                .iter()
                .map(|finding| line(finding))
                .collect::<Vec<_>>(),
            [Some(20), Some(27), Some(33)],
            "{findings:#?}"
        );
        let writes = findings_for_rule(&findings, "reactive-write-in-owned-scope");
        // DefaultWrite, FalseWrite, DeferStreamWrite, BodyStaticWrite; the
        // module-scope static write and both unknown-form writes are silent.
        assert_eq!(
            writes
                .iter()
                .map(|finding| line(finding))
                .collect::<Vec<_>>(),
            [Some(50), Some(57), Some(62), Some(71)],
            "{findings:#?}"
        );
        // Seven findings, and the SC9014 notice: rc.9 is older than the
        // audited rc.13 (ADR 0194).
        assert_eq!(findings.len(), 8, "{findings:#?}");
        assert_eq!(
            findings_for_rule(&findings, "unaudited-solid-release").len(),
            1,
            "{findings:#?}"
        );
    }
    if let Some(findings) = diagnostic_fixture("rc9-omit-predicate") {
        // The only violation is the control: the body read beside a
        // predicate. No predicate body is placed in a component.
        let reads = findings_for_rule(&findings, "strict-read-untracked");
        assert_eq!(
            reads
                .iter()
                .map(|finding| line(finding))
                .collect::<Vec<_>>(),
            [Some(48)],
            "{findings:#?}"
        );
        // Every predicate not proven inert, each at its argument and each
        // uncertifiable, with the reason it is not proven: a reactive
        // operation (inline read, inline write, named, const arrow, props,
        // forwarded through `hideBy`), a call out of the predicate, or no
        // inspectable body. The inert literal, the standard-library-only
        // literal, the key list and the forwarded parameter itself are silent.
        let predicates = findings_for_rule(&findings, "reactive-dispatch-unresolved");
        fn context(finding: &serde_json::Value) -> Option<&str> {
            finding["analysisContext"]
                .as_str()
                .map(|context| context.trim_start_matches("result-access-callback-"))
        }
        assert_eq!(
            predicates
                .iter()
                .map(|finding| (line(finding), context(finding)))
                .collect::<Vec<_>>(),
            [
                (Some(20), Some("reactive-operation")),
                (Some(27), Some("reactive-operation")),
                (Some(39), Some("reactive-operation")),
                (Some(67), Some("reactive-operation")),
                (Some(73), Some("reactive-operation")),
                (Some(84), Some("opaque-call")),
                (Some(91), Some("body-unresolved")),
                (Some(107), Some("reactive-operation")),
            ],
            "{findings:#?}"
        );
        assert!(
            predicates
                .iter()
                .all(|finding| finding["kind"] == "uncertifiable"),
            "{findings:#?}"
        );
        // The exported wrapper's own open callback (its callers may be outside
        // the project), the control and the eight predicates, and the SC9014
        // notice: rc.9 is older than the audited rc.13 (ADR 0194).
        assert_eq!(
            findings_for_rule(&findings, "package-contract-incomplete")
                .iter()
                .map(|finding| line(finding))
                .collect::<Vec<_>>(),
            [Some(100)],
            "{findings:#?}"
        );
        assert_eq!(findings.len(), 11, "{findings:#?}");
        assert_eq!(
            findings_for_rule(&findings, "unaudited-solid-release").len(),
            1,
            "{findings:#?}"
        );
    }
}

#[test]
fn broadened_rule_surfaces_pin_distinct_semantic_branches() {
    let Some(async_findings) = diagnostic_fixture("async-boundary") else {
        return;
    };
    assert!(
        async_findings.iter().all(|finding| {
            !finding["primaryLocation"]["path"]
                .as_str()
                .is_some_and(|path| path.ends_with("Good.tsx"))
        }),
        "nested Loading and inline pending observers must stay clean: {async_findings:#?}"
    );

    let Some(reactivity_findings) = diagnostic_fixture("shared-reactivity-v2") else {
        return;
    };
    for (rule, expected) in [
        // Two template interpolations. The third was a native JSX attribute,
        // which TypeScript rejects on its own (TS2322) and the 2026-08-17
        // narrowing dropped.
        ("uncalled-accessor", 2),
        // The reactive `props.onSave` read, plus the two hyphenated attributes
        // TypeScript deliberately declines to check: one proven invalid and
        // one callable/non-callable proof obligation. Ordinary `onClick`
        // non-callable values remain TS2322-owned.
        ("reactive-handler-frozen", 3),
    ] {
        assert_rule_findings(&reactivity_findings, rule, expected);
    }

    let Some(unresolved_findings) = diagnostic_fixture("static-api-unresolved") else {
        return;
    };
    // These six obligations asked whether the target carries the source
    // brand, and the brand is a type (`Refreshable<T>`), so TypeScript answers
    // the question outright: an unbranded target is TS2345, a branded one type
    // checks. The obligations were removed on 2026-08-17 and are pinned at 0;
    // the fixture keeps its unresolved targets so a reintroduction is caught
    // by the same cases that used to justify them.
    assert_rule_findings(&unresolved_findings, "refresh-target-unresolved", 0);
    assert_rule_findings(&unresolved_findings, "affects-target-unresolved", 0);
}

#[test]
fn static_violation_evidence_describes_the_actual_proof() {
    let Some(static_api) = diagnostic_fixture("static-api") else {
        return;
    };
    assert!(
        findings_for_rule(&static_api, "sync-computation-received-async")
            .into_iter()
            .all(|finding| {
                finding["evidence"][0]["message"]
                    .as_str()
                    .is_some_and(|message| message.contains("sync computation"))
            })
    );
}

#[test]
fn create_effect_owner_findings_require_runtime_allocation() {
    let Some(static_api) = diagnostic_fixture("static-api") else {
        return;
    };
    let lines = findings_for_rule(&static_api, "missing-owner")
        .into_iter()
        .map(|finding| finding["primaryLocation"]["line"].as_u64().unwrap())
        .collect::<Vec<_>>();
    // Solid 2 throws before allocating an effect node for an absent apply slot
    // (including the exact one-element tuple spread), undefined, or null apply
    // argument (including cast-hidden null). Other non-callable values
    // allocate first and still create an owner leak.
    assert_eq!(lines, [12, 13, 17, 19, 20, 21, 27, 40]);
}

#[test]
fn interprocedural_diagnostics_point_to_the_calling_component() {
    for (fixture, expected_count, message) in [
        ("interprocedural", 1, "readCount"),
        ("callback-forwarding", 1, "invoke"),
        ("polymorphic", 2, "readGeneric"),
        ("recursive", 1, "readA"),
        ("returned-closure", 1, "readCount"),
        ("store-flow", 1, "\"state.count\""),
        // Five: class, object, generic-function, the exact object passed to
        // `invoke`, and the conditional whose candidates have equivalent
        // reactive summaries. Divergent and computed dispatch are SC9012.
        ("interprocedural-methods-v2", 5, "count"),
    ] {
        let Some(findings) = diagnostic_fixture(fixture) else {
            return;
        };
        let strict = findings_for_rule(&findings, "strict-read-untracked");
        assert_eq!(
            strict.len(),
            expected_count,
            "fixture {fixture}: {findings:#?}"
        );
        assert!(strict.iter().any(|finding| {
            finding["message"]
                .as_str()
                .is_some_and(|text| text.contains(message))
        }));
        assert!(strict.iter().all(|finding| {
            finding["primaryLocation"]["path"]
                .as_str()
                .is_some_and(|path| path.ends_with("App.tsx"))
        }));
    }
}

#[test]
fn unknown_callback_diagnostic_contains_actionable_open_claim_context() {
    let Some(findings) = diagnostic_fixture("package-unknown-callback-producer") else {
        return;
    };
    let callback_findings = findings_for_rule(&findings, "package-contract-incomplete");
    assert_eq!(callback_findings.len(), 1, "{findings:#?}");
    let finding = &callback_findings[0];
    let message = finding["message"].as_str().unwrap_or_default();
    let hint = finding["hint"].as_str().unwrap_or_default();
    assert!(message.contains("current project.:schedule"), "{message}");
    assert!(message.contains("parameter 0 (() => void)"), "{message}");
    assert!(hint.contains("solid-checker-open-contract-claim"), "{hint}");
    assert!(hint.contains("semanticModelVersion\":1"), "{hint}");
    assert!(!hint.contains("schemaVersion\":1"), "{hint}");
    assert!(hint.contains("choose exactly one audited mode"), "{hint}");
    assert!(hint.contains("solid-checker contract generate"), "{hint}");
}

#[test]
fn control_flow_and_effect_phases_classify_strict_reads() {
    // control-flow: the two frozen Show-callback reads, plus the two frozen
    // reads under <For keyed={byId}> — a named key function proven callable
    // through type facts keeps the custom-key accessor claims. The dynamic
    // boolean `keyed={flag()}` functions contribute nothing: the callback
    // shape is ambiguous, so neither parameter is claimed as an accessor.
    for (fixture, expected) in [("control-flow", 4), ("execution-phases", 1)] {
        let Some(findings) = diagnostic_fixture(fixture) else {
            return;
        };
        assert_eq!(
            findings_for_rule(&findings, "strict-read-untracked").len(),
            expected,
            "fixture {fixture}: {findings:#?}"
        );
    }
}

/// ADR 0202: the default output lists findings about the user's code in full
/// and folds the analysis-coverage gaps into one grouped section; a closed
/// program reports no contract-generation obligation for its own exports.
#[test]
fn default_output_groups_coverage_gaps_after_findings_to_review() {
    let Ok(typefacts) = std::env::var("SOLID_TYPEFACTS_BIN") else {
        return;
    };
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let run = |fixture: &str, format: &str| {
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_solid-checker-rust"))
            .env("SOLID_TYPEFACTS_BIN", &typefacts)
            .env("SOLID_CHECKER_DAEMON", "0")
            .args(["--format", format, "--project"])
            .arg(
                root.join("fixtures/reactive-ir")
                    .join(fixture)
                    .join("tsconfig.json"),
            )
            .output()
            .expect("run the checker");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap()
    };

    let open = run("feedback-tiers-open", "default");
    let review = open.find("Needs review (1)").expect("review heading");
    let coverage = open
        .find("Analysis coverage: 2 sites in 2 groups")
        .expect("coverage heading");
    assert!(review < coverage, "{open}");
    // A coverage gap is one line with its count and first site, not a frame.
    assert!(
        open.contains("[SC9012] ageOf invokes .getTime on a caller-supplied value"),
        "{open}"
    );
    assert!(open.contains("1 site, first at App.tsx:18:21"), "{open}");
    assert!(!open[coverage..].contains(",-["), "{open}");
    // `--format full` keeps every finding in place.
    let full = run("feedback-tiers-open", "full");
    assert!(!full.contains("Analysis coverage"), "{full}");
    assert!(full.contains("[SC9005] callback parameter 0"), "{full}");

    // The same source in a closed program: no contract-generation obligation.
    let closed = run("feedback-tiers", "default");
    assert!(
        closed.contains("Analysis coverage: 1 site in 1 group"),
        "{closed}"
    );
    assert!(!closed.contains("SC9005"), "{closed}");
}
