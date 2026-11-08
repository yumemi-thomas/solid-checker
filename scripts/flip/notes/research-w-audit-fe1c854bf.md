# Audit of origin/main fe1c854bf (#173 merged): corpus sweep with a second path, ir-edit-fuzz, make verify

Track W grids agent on W, 2026-11-09, after the every-grid pass at a62c14ae1 (research-w-grids-rolling.md). Target `fe1c854bfc7fb3a262600dace9dfbfb213023cc9` = origin/main, which holds b7cedb9ef (code identical to a62c14ae1); `e40429f5e` is an ancestor and `if !seen.insert(span)` is in `composite_callee_walk`. Each step held the shared heavy lock alone, all 16 cores, with the 12 GB single-process guard. Nothing was touched but my own worktree and clones.

## 1. Frozen rc.21 corpus sweep: fe1c854bf against b571e6c5b

Corpus `../solid-checker-rc21-frozen/apps` with the target's `scripts/corpus/rc21-manifest.json` (23 apps at their commits, 34 projects; the installs were made on W, so totals compare with sweeps on W only). Baseline `b571e6c5b`, the last sweep on W (c2's ADR 0906 build). `--jobs 8`, macOS release builds.

| sweep | findings | uncertifiable | violation | exit errors |
|---|---:|---:|---:|---:|
| fe1c854bf, frozen root | 71400 | 71141 | 259 | 0 |
| b571e6c5b, frozen root | 74812 | 74603 | 209 | 0 |
| fe1c854bf, second root | 71400 | 71141 | 259 | 0 |

### The 50 new violations (209 -> 259, none lost)

Each was an uncertifiable at the same span in b571e6c5b and is a violation in fe1c854bf. The corpus has no runtime oracle: these are new proofs to review, not confirmed findings. SC8015 prefer-show 117 -> 161, SC1001 strict-read-untracked 74 -> 80.

| # | file:line:col | code | rule | blocker before |
|---:|---|---|---|---|
| 1 | `dashiboard-ui/dashiboard-ui/src/components/SelectorField.tsx:336:52` | SC8015 | prefer-show | uncertifiable:preference-read-unproven |
| 2 | `donegeon-client/web/apps/client/src/components/auth/WaitlistCard.tsx:109:10` | SC8015 | prefer-show | uncertifiable:preference-read-unproven |
| 3 | `donegeon-client/web/apps/client/src/components/auth/WaitlistCard.tsx:110:10` | SC8015 | prefer-show | uncertifiable:preference-read-unproven |
| 4 | `donegeon-client/web/apps/client/src/components/auth/WaitlistCard.tsx:111:10` | SC8015 | prefer-show | uncertifiable:preference-read-unproven |
| 5 | `donegeon-client/web/apps/client/src/routes/LoginRoute.tsx:184:14` | SC8015 | prefer-show | uncertifiable:preference-read-unproven |
| 6 | `donegeon-client/web/apps/client/src/routes/LoginRoute.tsx:188:18` | SC8015 | prefer-show | uncertifiable:tuple-iteration-written |
| 7 | `donegeon-client/web/apps/client/src/routes/LoginRoute.tsx:210:18` | SC8015 | prefer-show | uncertifiable:preference-read-unproven |
| 8 | `donegeon-client/web/apps/client/src/routes/LoginRoute.tsx:245:18` | SC8015 | prefer-show | uncertifiable:preference-read-unproven |
| 9 | `donegeon-client/web/apps/client/src/routes/LoginRoute.tsx:262:18` | SC8015 | prefer-show | uncertifiable:preference-read-unproven |
| 10 | `donegeon-client/web/apps/client/src/routes/LoginRoute.tsx:277:14` | SC8015 | prefer-show | uncertifiable:preference-read-unproven |
| 11 | `donegeon-client/web/apps/client/src/routes/OnboardingRoute.tsx:127:10` | SC8015 | prefer-show | uncertifiable:preference-read-unproven |
| 12 | `donegeon-client/web/apps/client/src/routes/OnboardingRoute.tsx:134:10` | SC8015 | prefer-show | uncertifiable:preference-read-unproven |
| 13 | `donegeon-client/web/apps/client/src/routes/OnboardingRoute.tsx:143:14` | SC8015 | prefer-show | uncertifiable:preference-read-unproven |
| 14 | `donegeon-client/web/apps/client/src/routes/OnboardingRoute.tsx:197:10` | SC8015 | prefer-show | uncertifiable:preference-read-unproven |
| 15 | `donegeon-client/web/apps/client/src/routes/OnboardingRoute.tsx:219:10` | SC8015 | prefer-show | uncertifiable:preference-read-unproven |
| 16 | `jandibat-web/apps/web/src/pages/AuthPage.tsx:138:14` | SC8015 | prefer-show | uncertifiable:preference-read-unproven |
| 17 | `jandibat-web/apps/web/src/pages/CustomPage.tsx:206:14` | SC8015 | prefer-show | uncertifiable:preference-read-unproven |
| 18 | `jandibat-web/apps/web/src/providers/ProviderCardsRelay.tsx:114:8` | SC8015 | prefer-show | uncertifiable:preference-read-unproven |
| 19 | `jandibat-web/apps/web/src/subjects/OwnerGate.tsx:170:12` | SC8015 | prefer-show | uncertifiable:preference-read-unproven |
| 20 | `kui/frontend/packages/kernel/src/components/charts/BarChart.tsx:214:24` | SC8015 | prefer-show | uncertifiable:preference-read-unproven |
| 21 | `kui/frontend/packages/kernel/src/components/charts/LineChart.tsx:240:24` | SC8015 | prefer-show | uncertifiable:preference-read-unproven |
| 22 | `kui/frontend/packages/kernel/src/components/charts/BarChart.tsx:214:24` | SC8015 | prefer-show | uncertifiable:preference-read-unproven |
| 23 | `kui/frontend/packages/kernel/src/components/charts/LineChart.tsx:240:24` | SC8015 | prefer-show | uncertifiable:preference-read-unproven |
| 24 | `kui/frontend/packages/kernel/src/components/charts/BarChart.tsx:214:24` | SC8015 | prefer-show | uncertifiable:preference-read-unproven |
| 25 | `kui/frontend/packages/kernel/src/components/charts/LineChart.tsx:240:24` | SC8015 | prefer-show | uncertifiable:preference-read-unproven |
| 26 | `kui/frontend/packages/kernel/src/components/charts/BarChart.tsx:214:24` | SC8015 | prefer-show | uncertifiable:preference-read-unproven |
| 27 | `kui/frontend/packages/kernel/src/components/charts/LineChart.tsx:240:24` | SC8015 | prefer-show | uncertifiable:preference-read-unproven |
| 28 | `kui/frontend/packages/kernel/src/components/charts/BarChart.tsx:214:24` | SC8015 | prefer-show | uncertifiable:preference-read-unproven |
| 29 | `kui/frontend/packages/kernel/src/components/charts/LineChart.tsx:240:24` | SC8015 | prefer-show | uncertifiable:preference-read-unproven |
| 30 | `kui/frontend/packages/feature-topics/src/TopicListPage.tsx:298:42` | SC1001 | strict-read-untracked | uncertifiable:read-backing-unproven:TopicListPage |
| 31 | `kui/frontend/packages/feature-topics/src/TopicListPage.tsx:314:22` | SC1001 | strict-read-untracked | uncertifiable:read-backing-unproven:createEffect apply callback |
| 32 | `kui/frontend/packages/feature-topics/src/TopicsRoute.tsx:522:28` | SC1001 | strict-read-untracked | uncertifiable:read-backing-unproven:createEffect apply callback |
| 33 | `kui/frontend/packages/feature-topics/src/TopicsRoute.tsx:522:62` | SC1001 | strict-read-untracked | uncertifiable:read-backing-unproven:createEffect apply callback |
| 34 | `kui/frontend/packages/kernel/src/components/charts/BarChart.tsx:214:24` | SC8015 | prefer-show | uncertifiable:preference-read-unproven |
| 35 | `kui/frontend/packages/kernel/src/components/charts/LineChart.tsx:240:24` | SC8015 | prefer-show | uncertifiable:preference-read-unproven |
| 36 | `kui/frontend/packages/kernel/src/components/Drawer.stories.tsx:127:18` | SC8015 | prefer-show | uncertifiable:preference-read-unproven |
| 37 | `kui/frontend/packages/kernel/src/components/charts/BarChart.tsx:214:24` | SC8015 | prefer-show | uncertifiable:preference-read-unproven |
| 38 | `kui/frontend/packages/kernel/src/components/charts/LineChart.tsx:240:24` | SC8015 | prefer-show | uncertifiable:preference-read-unproven |
| 39 | `sensor-sim/packages/frontend/src/components/LoginForm.tsx:38:8` | SC8015 | prefer-show | uncertifiable:preference-read-unproven |
| 40 | `sensor-sim/packages/frontend/src/components/users/UserAddForm.tsx:30:12` | SC8015 | prefer-show | uncertifiable:preference-read-unproven |
| 41 | `viviana-ui-web/apps/web/src/components/ThemeCreator.tsx:63:42` | SC1001 | strict-read-untracked | uncertifiable:read-backing-unproven:ColorControl |
| 42 | `viviana-ui-web/apps/web/src/components/theme/ColorKnob.tsx:34:42` | SC1001 | strict-read-untracked | uncertifiable:read-backing-unproven:ColorKnob |
| 43 | `viviana-ui-web/apps/web/src/routes/showcase/overlays.tsx:242:14` | SC8015 | prefer-show | uncertifiable:preference-read-unproven |
| 44 | `viviana-ui-web/apps/web/src/routes/solid-spectrum/docs/components/alertdialog.tsx:55:12` | SC8015 | prefer-show | uncertifiable:preference-read-unproven |
| 45 | `viviana-ui-web/apps/web/src/routes/solid-spectrum/docs/components/link.tsx:53:12` | SC8015 | prefer-show | uncertifiable:preference-read-unproven |
| 46 | `viviana-ui-web/apps/web/src/routes/solid-spectrum/docs/components/progressbar.tsx:62:14` | SC8015 | prefer-show | uncertifiable:preference-read-unproven |
| 47 | `viviana-ui-web/apps/web/src/routes/solid-spectrum/docs/components/taggroup.tsx:87:12` | SC8015 | prefer-show | uncertifiable:preference-read-unproven |
| 48 | `viviana-ui-web/apps/web/src/routes/solid-spectrum/docs/components/taggroup.tsx:96:14` | SC8015 | prefer-show | uncertifiable:preference-read-unproven |
| 49 | `viviana-ui-web/apps/web/src/routes/solid-spectrum/docs/hooks/create-press.tsx:306:10` | SC8015 | prefer-show | uncertifiable:preference-read-unproven |
| 50 | `viviana-ui-web/apps/web/src/routes/viviana-ui/docs/hooks/create-press.tsx:306:10` | SC8015 | prefer-show | uncertifiable:preference-read-unproven |

### Moved rows: 6690

| change (code rule before -> after) | rows |
|---|---:|
| gone SC9012 reactive-dispatch-unresolved uncertifiable:tuple-iteration-written | 4386 |
| new SC9012 reactive-dispatch-unresolved uncertifiable:listener-reach | 258 |
| gone SC5003 async-outside-loading-boundary uncertifiable:async-read-unaccounted | 167 |
| moved SC9012 reactive-dispatch-unresolved uncertifiable:tuple-iteration-written -> uncertifiable:value-forwarded | 126 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:owned scope | 104 |
| moved SC9012 reactive-dispatch-unresolved uncertifiable:tuple-iteration-written -> uncertifiable:value-held-in-binding | 96 |
| new SC9012 reactive-dispatch-unresolved uncertifiable:function-run-unplaced | 94 |
| gone SC5001 pending-async-unsuspendable-read uncertifiable:async-read-unaccounted | 78 |
| moved SC9012 reactive-dispatch-unresolved uncertifiable:tuple-iteration-written -> uncertifiable:value-to-unresolved-callee | 70 |
| gone SC5003 async-outside-loading-boundary uncertifiable:async-provenance-unproven | 67 |
| moved SC9012 reactive-dispatch-unresolved uncertifiable:tuple-iteration-written -> uncertifiable:value-in-carrier | 65 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:TeamSettingsRoute | 58 |
| gone SC5001 pending-async-unsuspendable-read uncertifiable:source-pending-unproven | 48 |
| moved SC8015 prefer-show uncertifiable:preference-read-unproven -> violation:null | 43 |
| new SC5003 async-outside-loading-boundary uncertifiable:async-provenance-unproven | 38 |
| gone SC9012 reactive-dispatch-unresolved uncertifiable:handed-member-read | 33 |
| moved SC9012 reactive-dispatch-unresolved uncertifiable:handed-member-dispatch -> uncertifiable:function-run-unplaced | 32 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:HomeTaskCreateModal | 31 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:HomeDesktopSidebar | 30 |
| new SC9012 reactive-dispatch-unresolved uncertifiable:value-forwarded | 30 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:RecordRow | 28 |
| new SC5003 async-outside-loading-boundary uncertifiable:async-read-unaccounted | 26 |
| new SC5001 pending-async-unsuspendable-read uncertifiable:async-read-unaccounted | 23 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:VirtualizedTable | 21 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:KsqlScreen | 18 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:HeaderChip | 14 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:JsonNode | 14 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:BarChart | 14 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:Histogram | 14 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:LineChart | 14 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:Playground | 14 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:HomeLabelsManager | 13 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:HomeSectionManager | 13 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:OnboardingRoute | 13 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:EmbedPage | 13 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:LoginRoute | 12 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:ProjectPicker | 11 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:LabelPicker | 10 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:ProfileRoute | 10 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:select | 10 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:TopicsScreen | 10 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:TopicScreen | 10 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:SectionPicker | 9 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:WaitlistCard | 8 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:start | 8 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:ConnectionsContent | 8 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:ManageScreen | 8 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:SpecimenDeck | 8 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:Cards | 7 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:Pagination | 7 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:settle | 7 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:listener | 7 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:Registry | 7 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:Documents | 6 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:TagGroupPage | 6 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:DocsLayout | 6 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:SelectorField | 5 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:AuthPage | 5 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:CustomContent | 5 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:OwnerGate | 5 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:ThemeCreator | 5 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:Page | 5 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:AlertDialogPage | 5 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:MenuPage | 5 |
| gone SC9012 reactive-dispatch-unresolved uncertifiable:prop-pending-unproven | 5 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:Presets | 4 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:HelpButton | 4 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:Loader | 4 |
| moved SC9012 reactive-dispatch-unresolved uncertifiable:tuple-iteration-written+uncertifiable:value-use-unclassified:budget -> uncertifiable:value-in-carrier+uncertifiable:value-use-unclassified:budget | 4 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:ToastProvider | 4 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:retry | 4 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:measure | 4 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:SignedInCard | 4 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:Autocomplete | 4 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:ConnectScreen | 4 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:SubjectList | 4 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:TopicListPage | 4 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:RoadmapPanel | 4 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:TasksPanel | 4 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:ActionGroupDemo | 4 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:SettingsScreen | 4 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:BreadcrumbsPage | 4 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:DisclosurePage | 4 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:GridListPage | 4 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:CreatePressPage | 4 |
| gone SC9012 reactive-dispatch-unresolved uncertifiable:closed-parameter-member-dispatch | 4 |
| moved SC9012 reactive-dispatch-unresolved uncertifiable:value-in-carrier -> uncertifiable:listener-reach+uncertifiable:value-in-carrier | 3 |
| new SC9012 reactive-dispatch-unresolved uncertifiable:value-held-in-binding | 3 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:FilePicker | 3 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:render callback | 3 |
| new SC9012 reactive-dispatch-unresolved uncertifiable:parameter-member-target-unresolved | 3 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:HelpTip | 3 |
| moved SC9012 reactive-dispatch-unresolved uncertifiable:handed-member-dispatch+uncertifiable:standard-member-impostor:a key no fact names may be `filter`: <apps>/dashiboard-ui/dashiboard-ui/node_modules/.pnpm/@solidjs+router@2.0.0-next.41_@solidjs+web@2.0.0-rc.21_solid-js@2.0.0-rc.21__filesystem_c500c3151898f2e9f2afcc8d83533632/node_modules/@solidjs/router/dist/data/action.js:215 -> uncertifiable:function-run-unplaced+uncertifiable:standard-member-impostor:a key no fact names may be `filter`: <apps>/dashiboard-ui/dashiboard-ui/node_modules/.pnpm/@solidjs+router@2.0.0-next.41_@solidjs+web@2.0.0-rc.21_solid-js@2.0.0-rc.21__filesystem_c500c3151898f2e9f2afcc8d83533632/node_modules/@solidjs/router/dist/data/action.js:215 | 3 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:SidebarAccountCard | 3 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:ActionMenu | 3 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:Picker | 3 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:advance | 3 |
| new SC4001 missing-owner uncertifiable:owner-callers-unenumerated | 3 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:App | 3 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:Heatmap | 3 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:CustomWorkspace | 3 |
| moved SC9012 reactive-dispatch-unresolved uncertifiable:handed-member-dispatch+uncertifiable:standard-member-impostor:a key no fact names may be `filter`: <apps>/kui/frontend/node_modules/.pnpm/@solidjs+router@2.0.0-next.41_@solidjs+web@2.0.0-rc.21_solid-js@2.0.0-rc.21__solid-js@2.0.0-rc.21/node_modules/@solidjs/router/dist/data/action.js:215 -> uncertifiable:function-run-unplaced+uncertifiable:standard-member-impostor:a key no fact names may be `filter`: <apps>/kui/frontend/node_modules/.pnpm/@solidjs+router@2.0.0-next.41_@solidjs+web@2.0.0-rc.21_solid-js@2.0.0-rc.21__solid-js@2.0.0-rc.21/node_modules/@solidjs/router/dist/data/action.js:215 | 3 |
| moved SC1001 strict-read-untracked uncertifiable:read-backing-unproven:createEffect apply callback -> violation:createEffect apply callback | 3 |
| moved SC9012 reactive-dispatch-unresolved uncertifiable:handed-member-dispatch+uncertifiable:standard-member-impostor:the program may put a function at `map`: <apps>/solid-groove/node_modules/@solidjs/router/dist/data/liveQuery.js:153 -> uncertifiable:function-run-unplaced+uncertifiable:standard-member-impostor:the program may put a function at `map`: <apps>/solid-groove/node_modules/@solidjs/router/dist/data/liveQuery.js:153 | 3 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:handleScroll | 3 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:StyledComboBoxDemo | 3 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:ActionBarDemo | 3 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:LinkPage | 3 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:ProgressBarPage | 3 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:SearchFieldPage | 3 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:TabsPage | 3 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:TreePage | 3 |
| gone SC1001 strict-read-untracked uncertifiable:function-run-unplaced | 3 |
| gone SC5001 pending-async-unsuspendable-read uncertifiable:async-provenance-unproven | 3 |
| moved SC9012 reactive-dispatch-unresolved uncertifiable:handed-member-dispatch+uncertifiable:standard-member-impostor:the program may put a function at `map`: <apps>/app-game/node_modules/.pnpm/@solid-primitives+utils@7.0.0-next.4_@solidjs+web@2.0.0-rc.21_solid-js@2.0.0-rc.21__solid-js@2.0.0-rc.21/node_modules/@solid-primitives/utils/dist/immutable/array.js:133 -> uncertifiable:function-run-unplaced+uncertifiable:standard-member-impostor:the program may put a function at `map`: <apps>/app-game/node_modules/.pnpm/@solid-primitives+utils@7.0.0-next.4_@solidjs+web@2.0.0-rc.21_solid-js@2.0.0-rc.21__solid-js@2.0.0-rc.21/node_modules/@solid-primitives/utils/dist/immutable/array.js:133 | 2 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:GroupsEditor | 2 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:Results | 2 |
| moved SC9012 reactive-dispatch-unresolved uncertifiable:handed-member-dispatch+uncertifiable:standard-member-impostor:the program may put a function at `map`: <apps>/dashiboard-ui/dashiboard-ui/node_modules/.pnpm/@solidjs+router@2.0.0-next.41_@solidjs+web@2.0.0-rc.21_solid-js@2.0.0-rc.21__filesystem_c500c3151898f2e9f2afcc8d83533632/node_modules/@solidjs/router/dist/data/liveQuery.js:153 -> uncertifiable:function-run-unplaced+uncertifiable:standard-member-impostor:the program may put a function at `map`: <apps>/dashiboard-ui/dashiboard-ui/node_modules/.pnpm/@solidjs+router@2.0.0-next.41_@solidjs+web@2.0.0-rc.21_solid-js@2.0.0-rc.21__filesystem_c500c3151898f2e9f2afcc8d83533632/node_modules/@solidjs/router/dist/data/liveQuery.js:153 | 2 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:AppShell | 2 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:ThemeProvider | 2 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:resetComposerPreview | 2 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:resetDetailPreview | 2 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:cancel | 2 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:animateTo | 2 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:home | 2 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:end | 2 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:seekFraction | 2 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:AutoplayAlgorithmWalkthrough | 2 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:NavBar | 2 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:Home | 2 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:createEffect apply callback | 2 |
| moved SC9012 reactive-dispatch-unresolved uncertifiable:returned-callable-outside-source-universe -> uncertifiable:handed-member-read | 2 |
| moved SC9012 reactive-dispatch-unresolved uncertifiable:returned-callable-outside-source-universe -> uncertifiable:handed-member-dispatch | 2 |
| new SC5001 pending-async-unsuspendable-read uncertifiable:async-provenance-unproven | 2 |
| new SC5003 async-outside-loading-boundary uncertifiable:root-mount-pass-unproven | 2 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:AlertsScreen | 2 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:ConfigurationTab | 2 |
| moved SC9012 reactive-dispatch-unresolved uncertifiable:handed-member-dispatch+uncertifiable:standard-member-impostor:the program may put a function at `map`: <apps>/kui/frontend/node_modules/.pnpm/@solidjs+router@2.0.0-next.41_@solidjs+web@2.0.0-rc.21_solid-js@2.0.0-rc.21__solid-js@2.0.0-rc.21/node_modules/@solidjs/router/dist/data/liveQuery.js:153 -> uncertifiable:function-run-unplaced+uncertifiable:standard-member-impostor:the program may put a function at `map`: <apps>/kui/frontend/node_modules/.pnpm/@solidjs+router@2.0.0-next.41_@solidjs+web@2.0.0-rc.21_solid-js@2.0.0-rc.21__solid-js@2.0.0-rc.21/node_modules/@solidjs/router/dist/data/liveQuery.js:153 | 2 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:TopicSettings | 2 |
| moved SC9012 reactive-dispatch-unresolved uncertifiable:tuple-iteration-written -> uncertifiable:value-through-logical | 2 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:ControlledList | 2 |
| new SC9012 reactive-dispatch-unresolved uncertifiable:value-to-unresolved-callee | 2 |
| moved SC9012 reactive-dispatch-unresolved uncertifiable:tuple-iteration-written+uncertifiable:value-use-unclassified:budget -> uncertifiable:value-forwarded+uncertifiable:value-use-unclassified:budget | 2 |
| moved SC9012 reactive-dispatch-unresolved uncertifiable:handed-member-dispatch+uncertifiable:standard-member-impostor:the program may put a function at `slice`: <apps>/kui/frontend/packages/kernel/src/components/index.ts:105 -> uncertifiable:function-run-unplaced+uncertifiable:standard-member-impostor:the program may put a function at `slice`: <apps>/kui/frontend/packages/kernel/src/components/index.ts:105 | 2 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:LoginForm | 2 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:UserAddForm | 2 |
| moved SC9012 reactive-dispatch-unresolved uncertifiable:handed-member-dispatch+uncertifiable:standard-member-impostor:a key no fact names may be `find`: <apps>/solid-groove/docs/design/support.js:5 -> uncertifiable:function-run-unplaced+uncertifiable:standard-member-impostor:a key no fact names may be `find`: <apps>/solid-groove/docs/design/support.js:5 | 2 |
| moved SC9012 reactive-dispatch-unresolved uncertifiable:handed-member-dispatch+uncertifiable:standard-member-impostor:a key no fact names may be `slice`: <apps>/solid-groove/docs/design/support.js:5 -> uncertifiable:function-run-unplaced+uncertifiable:standard-member-impostor:a key no fact names may be `slice`: <apps>/solid-groove/docs/design/support.js:5 | 2 |
| moved SC9012 reactive-dispatch-unresolved uncertifiable:leaf-callback-opaque+uncertifiable:closed-parameter-member-dispatch -> uncertifiable:leaf-callback-opaque | 2 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:TreeDemo | 2 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:TagGroupSelectionDemo | 2 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:ApiDocsLayout | 2 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:ButtonPage | 2 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:FileTriggerPage | 2 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:MeterPage | 2 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:TablePage | 2 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:CreateButtonPage | 2 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:PressStateDemo | 2 |
| gone SC1001 strict-read-untracked uncertifiable:callee-callback-timing | 2 |
| gone SC1001 strict-read-untracked uncertifiable:callee-callback-timing:write | 2 |
| gone SC9012 reactive-dispatch-unresolved uncertifiable:returned-callable-outside-source-universe | 2 |
| moved SC9012 reactive-dispatch-unresolved uncertifiable:handed-member-dispatch+uncertifiable:standard-member-impostor:the program may put a function at `filter`: <apps>/app-game/node_modules/.pnpm/@solid-primitives+utils@7.0.0-next.4_@solidjs+web@2.0.0-rc.21_solid-js@2.0.0-rc.21__solid-js@2.0.0-rc.21/node_modules/@solid-primitives/utils/dist/immutable/array.js:133 -> uncertifiable:function-run-unplaced+uncertifiable:standard-member-impostor:the program may put a function at `filter`: <apps>/app-game/node_modules/.pnpm/@solid-primitives+utils@7.0.0-next.4_@solidjs+web@2.0.0-rc.21_solid-js@2.0.0-rc.21__solid-js@2.0.0-rc.21/node_modules/@solid-primitives/utils/dist/immutable/array.js:133 | 1 |
| moved SC9012 reactive-dispatch-unresolved uncertifiable:handed-member-dispatch+uncertifiable:standard-member-impostor:a key no fact names may be `flatMap`: <apps>/app-game/apps/abr-viewer/src/features/brush-detail/BrushDetailEditable.tsx:185 -> uncertifiable:function-run-unplaced+uncertifiable:standard-member-impostor:a key no fact names may be `flatMap`: <apps>/app-game/apps/abr-viewer/src/features/brush-detail/BrushDetailEditable.tsx:185 | 1 |
| moved SC9012 reactive-dispatch-unresolved uncertifiable:handed-member-dispatch+uncertifiable:standard-member-impostor:the program may put a function at `every`: <apps>/app-game/packages/math-examples/geometric-algebra/src/element.ts:8 -> uncertifiable:function-run-unplaced+uncertifiable:standard-member-impostor:the program may put a function at `every`: <apps>/app-game/packages/math-examples/geometric-algebra/src/element.ts:8 | 1 |
| moved SC9012 reactive-dispatch-unresolved uncertifiable:handed-member-dispatch+uncertifiable:standard-member-impostor:the program may put a function at `some`: <apps>/app-game/packages/ecsy/utils/some.ts:1 -> uncertifiable:function-run-unplaced+uncertifiable:standard-member-impostor:the program may put a function at `some`: <apps>/app-game/packages/ecsy/utils/some.ts:1 | 1 |
| moved SC9012 reactive-dispatch-unresolved uncertifiable:handed-member-dispatch+uncertifiable:standard-member-impostor:the program may put a function at `then`: <apps>/app-game/packages/math-examples/bitecs-ecs/index.tsx:35 -> uncertifiable:standard-member-impostor:the program may put a function at `then`: <apps>/app-game/packages/math-examples/bitecs-ecs/index.tsx:35 | 1 |
| new SC9012 reactive-dispatch-unresolved uncertifiable:parameter-member-path-unresolved | 1 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:Toggler | 1 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:Counter | 1 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:Filters | 1 |
| new SC1001 strict-read-untracked uncertifiable:function-run-unplaced | 1 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:pruneFilters | 1 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:rememberDescribed | 1 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:toggleTheme | 1 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:HomeTaskContent | 1 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:HomeTaskDetailModal | 1 |
| moved SC1001 strict-read-untracked uncertifiable:callee-callback-timing+uncertifiable:callee-callback-timing -> uncertifiable:callee-callback-timing | 1 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:TaskQuickAddComposer | 1 |
| moved SC9012 reactive-dispatch-unresolved uncertifiable:tuple-iteration-written+uncertifiable:value-use-unclassified:budget -> uncertifiable:value-held-in-binding+uncertifiable:value-use-unclassified:budget | 1 |
| moved SC9012 reactive-dispatch-unresolved uncertifiable:leaf-callback-opaque+uncertifiable:tuple-iteration-written -> uncertifiable:leaf-callback-opaque | 1 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:refreshData | 1 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:persistOrder | 1 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:reorderTasks | 1 |
| moved SC8015 prefer-show uncertifiable:tuple-iteration-written -> violation:null | 1 |
| moved SC1001 strict-read-untracked uncertifiable:callee-callback-timing -> uncertifiable:callee-callback-timing+uncertifiable:callee-callback-timing | 1 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:WaitlistRoute | 1 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:ExplorerPage | 1 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:clearSelection | 1 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:replace | 1 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:move | 1 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:clearDrag | 1 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:handlePointerMove | 1 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:play | 1 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:CardGridPage | 1 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:cancelJump | 1 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:navigate | 1 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:selectOwnerSubject | 1 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:clearOwnerSubject | 1 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:setExploreSubject | 1 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:showToast | 1 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:ConnectionsWorkspace | 1 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:ActivityResult | 1 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:ProviderConnectForm | 1 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:rotate | 1 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:TestApp | 1 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:cleanup | 1 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:commit | 1 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:setIsLoadingMore | 1 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:disposeFetch | 1 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:startFetch | 1 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:completeFetch | 1 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:BadgesDialog | 1 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:DependencyNode | 1 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:Services | 1 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:PackagePage | 1 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:set | 1 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:rememberViewed | 1 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:setTheme | 1 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:BrokersScreen | 1 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:ClusterAdmin | 1 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:CompatibilityCheck | 1 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:RegisterSchemaDialog | 1 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:AddPartitionsDialog | 1 |
| moved SC1001 strict-read-untracked uncertifiable:read-backing-unproven:TopicListPage -> violation:TopicListPage | 1 |
| new SC9012 reactive-dispatch-unresolved uncertifiable:parameter-use-unmodeled | 1 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:toggle | 1 |
| moved SC9012 reactive-dispatch-unresolved uncertifiable:handed-member-dispatch+uncertifiable:standard-member-impostor:a key no fact names may be `reduce`: <apps>/kui/frontend/node_modules/.pnpm/@solidjs+router@2.0.0-next.41_@solidjs+web@2.0.0-rc.21_solid-js@2.0.0-rc.21__solid-js@2.0.0-rc.21/node_modules/@solidjs/router/dist/data/action.js:215 -> uncertifiable:function-run-unplaced+uncertifiable:standard-member-impostor:a key no fact names may be `reduce`: <apps>/kui/frontend/node_modules/.pnpm/@solidjs+router@2.0.0-next.41_@solidjs+web@2.0.0-rc.21_solid-js@2.0.0-rc.21__solid-js@2.0.0-rc.21/node_modules/@solidjs/router/dist/data/action.js:215 | 1 |
| moved SC9012 reactive-dispatch-unresolved uncertifiable:handed-member-read+uncertifiable:standard-member-impostor:the program may put a function at `map`: <apps>/kui/frontend/node_modules/.pnpm/@solidjs+router@2.0.0-next.41_@solidjs+web@2.0.0-rc.21_solid-js@2.0.0-rc.21__solid-js@2.0.0-rc.21/node_modules/@solidjs/router/dist/data/liveQuery.js:153 -> uncertifiable:function-run-unplaced+uncertifiable:standard-member-impostor:the program may put a function at `map`: <apps>/kui/frontend/node_modules/.pnpm/@solidjs+router@2.0.0-next.41_@solidjs+web@2.0.0-rc.21_solid-js@2.0.0-rc.21__solid-js@2.0.0-rc.21/node_modules/@solidjs/router/dist/data/liveQuery.js:153 | 1 |
| moved SC9012 reactive-dispatch-unresolved uncertifiable:handed-member-dispatch+uncertifiable:standard-member-impostor:the program may put a function at `map`: <apps>/kui/frontend/node_modules/.pnpm/@solidjs+router@2.0.0-next.41_@solidjs+web@2.0.0-rc.21_solid-js@2.0.0-rc.21__solid-js@2.0.0-rc.21/node_modules/@solidjs/router/dist/data/liveQuery.js:153 -> uncertifiable:standard-member-impostor:the program may put a function at `map`: <apps>/kui/frontend/node_modules/.pnpm/@solidjs+router@2.0.0-next.41_@solidjs+web@2.0.0-rc.21_solid-js@2.0.0-rc.21__solid-js@2.0.0-rc.21/node_modules/@solidjs/router/dist/data/liveQuery.js:153 | 1 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:login | 1 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:logout | 1 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:SimList | 1 |
| moved SC9012 reactive-dispatch-unresolved uncertifiable:handed-member-dispatch+uncertifiable:standard-member-impostor:the program may put a function at `map`: <apps>/skyjtx-website/node_modules/.bun/@solidjs+router@2.0.0-next.41+53b5d6c669304543/node_modules/@solidjs/router/dist/data/liveQuery.js:153 -> uncertifiable:function-run-unplaced+uncertifiable:standard-member-impostor:the program may put a function at `map`: <apps>/skyjtx-website/node_modules/.bun/@solidjs+router@2.0.0-next.41+53b5d6c669304543/node_modules/@solidjs/router/dist/data/liveQuery.js:153 | 1 |
| moved SC1001 strict-read-untracked uncertifiable:read-backing-unproven:ColorControl -> violation:ColorControl | 1 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:ColorControl | 1 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:TableDemo | 1 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:GridListDemo | 1 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:SelectDemo | 1 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:StyledSelectDemo | 1 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:StyledTabsDemo | 1 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:StyledNumberFieldDemo | 1 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:StyledSearchFieldDemo | 1 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:StyledSliderDemo | 1 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:TagGroupDemo | 1 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:RangeSliderDemo | 1 |
| moved SC1001 strict-read-untracked uncertifiable:read-backing-unproven:ColorKnob -> violation:ColorKnob | 1 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:ColorKnob | 1 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:SpectrumPreviewGallery | 1 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:ThemePreviewGallery | 1 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:ThemeStudio | 1 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:TheaterScreen | 1 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:ButtonsPanel | 1 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:AccordionPage | 1 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:ActionBarPage | 1 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:DialogPage | 1 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:DropZonePage | 1 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:SelectPage | 1 |
| new SC2001 no-write-in-pure-computation uncertifiable:function-run-unplaced:swap | 1 |
| gone SC1001 strict-read-untracked uncertifiable:function-run-unplaced:handleChange while evaluating a condition | 1 |
| gone SC1001 strict-read-untracked uncertifiable:function-run-unplaced:handleChange | 1 |
| gone SC1001 strict-read-untracked uncertifiable:callee-callback-timing:say | 1 |
| gone SC1001 strict-read-untracked uncertifiable:read-backing-unproven:createEffect apply callback | 1 |
| gone SC9012 reactive-dispatch-unresolved uncertifiable:handed-member-dispatch | 1 |

Every moved row (code, rule, kind, blocker, path, line, column, span): `research-w-audit-fe1c854bf-sweep.json` beside this file, `moved.rows`.

## 2. Second path

fe1c854bf swept again on an APFS clone of the apps at `/Users/thomas.guehenneux/EDF/tmp/flipw/rolling/req/second-root-sweep-x/rc21-apps-relocated-copy` (another name and length; deleted after): **0 path-dependent differences**. Every finding was compared whole after replacing the root (code, rule, kind, blocker, span, message, hint, evidence), and every project's exit code, status and premises.

## 3. ir-edit-fuzz on the corpus, long budget

`bun scripts/ir-edit-fuzz.mjs --project <tsconfig> --steps 30 --seed 7 --keep-going`, the target's release build, daemon with inline shadow against a fresh one-shot. The script edits in place, so it ran on an APFS clone of the three projects it names (deleted after); the frozen corpus was not touched.

| project | result | median daemon / one-shot |
|---|---|---|
| app-game/apps/web | 30 step(s), 0 mismatch(es) | 18066 ms / 14862 ms |
| solid-groove | 30 step(s), 0 mismatch(es) | 5294 ms / 3803 ms |
| compass-ui/apps/ui | 30 step(s), 0 mismatch(es) | 4504 ms / 3212 ms |

**0 mismatches in 90 steps.** The daemon answered slower than the one-shot on all three (a latency note, not a mismatch).

## 4. make verify: not green on W, one timing test

- Run 1 (02:46-02:58, host busy): `rc=2`, `FAILED during step builds-and-suites`, lane fuzz-tests: 1391 passed, 3 failed, all in `scripts/fuzz/exec.test.mjs` ("the executor's deadlines": `the executor was not ready within 0.8 s`).
- Run 2 (06:16-06:23, host load ~4): `rc=2`, same step and lane: 1392 passed, 2 failed, same file and message.
- The file alone (`bun scripts/fuzz/test-lane.mjs scripts/fuzz/exec.test.mjs`, under the lock, load 4.2): 4 of 4 pass.
- Beside verify's parallel build lanes the 0.8 s deadline fails on this machine, both times; alone it passes. The same family failed on e57cc0dc5 here. verify stops at its first failed step, so its later steps did not run in this audit. No limit was changed.
