#!/usr/bin/env bun
// Documentation budget: keeps the agent-facing prose from regrowing.
//
// In 2026-10 the repository carried 6.7 MB of docs, a 24 KB AGENTS.md and
// 600 KB of fixture READMEs, most of it slice reports and review rounds for
// subsystems that no longer existed. Agents read it as current truth. The
// cut kept current invariants, live decisions and open items only; this gate
// holds that line. Slice reports belong in commit messages, a design decision
// in docs/decisions.md, an open item in docs/open-work.md.
//
//   bun scripts/doc-budget.mjs        check, exit 1 on any violation
import { readFileSync, statSync } from "node:fs";
import { execFileSync } from "node:child_process";
import { dirname, join, resolve } from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

export const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..");

/** Byte ceilings, about 1.5x the size at the 2026-10-19 cut. */
export const FILE_BUDGETS = {
  "AGENTS.md": 8_000,
  "CLAUDE.md": 1_000,
  // Raised for ADR 0383's latent-key item, and for ADR 0413.
  // Raised for ADR 0414 (the file stood 28,428 bytes before its two entries);
  // for ADR 0415 (the file stood 29,958 bytes).
  // and for ADR 0419 (the file stood 30,082 bytes).
  // Raised for ADR 0436's open cases: the file stood 6 bytes under 26_300.
  // And for ADRs 0437-0438's open cases: the file stood at 26_659 bytes against 26_700.
  // And for ADR 0439's leaf obligations: 27_971 bytes against 27_500.
  // And for ADR 0438's async and test-helper limits: 28_418 bytes against 28_000.
  // Raised again for the ADR 0381 and 0424-0441 docs series (catalog renames, ADRs 0440-0441 to come): 32_000.
  // And for ADR 0443's untracked-read misses: 32_541 bytes against 32_000: 33_000.
  // And for ADRs 0444-0445's trades (workspace boundary, exported capitalized components): 33_453 bytes against 33_000: 34_000.
  // Merging origin/main (the incremental-daemon track, ADRs 0382-0423) into the catalog-review branch (ADRs 0381, 0424-0448), with ADR 0449: the two sides' raises combine; the merged tree stood at 38,235 bytes: 39_000.
  // And for the default-value known miss of ADR 0452 (39_161 bytes against 39_000): 39_500.
  // And for the pinned SC2001 false positive and the L1 line (40_194 bytes): 40_500.
  // And for ADR 0454's SC4001/SC3001 follow-up and the moves it lists: 41,187 bytes against 40,500: 41_500.
  // And the reachability/summary-graph follow-up (41,549 bytes): 42_000.
  // And for ADR 0455's recall costs and silences (42,194 bytes against 42,000): 42_500.
  // And ADR 0455's cleanup amendment (42,543 bytes against 42,500): 43_000.
  // And its third-review amendment (43,189 bytes): 43_500.
  // And the fourth-review returned-cleanup loss (43,508 bytes): 43_750.
  // And ADR 0457's review amendment line: 43,900 bytes: 43_900.
  // And ADR 0456's handle costs and the two related fuzz shapes (44,364 bytes): 44_500.
  // With ADR 0456 (PR #123) merged into the ADR 0457 branch: 44,802 bytes: 44_900.
  // And ADR 0458's unreached-site misses and the SC3001 note (45,337 bytes): 45_400.
  // And ADR 0459's known misses (the destructured forms it does not resolve): 45,329 bytes: 45_400.
  // And its review nits (two more unresolved shapes): 45,461 bytes: 45_500.
  // Merge of main's ADR 0458 (PR #128) with ADRs 0459/0460 (renumbered from 0458/0459): 45,915 bytes: 46_000.
  // And ADR 0461's review amendment (the setter-exemption note, the assignment form's TS18048 overlap): 45,497 bytes: 45_500.
  // Merge of PR #129 (ADRs 0459/0460) with ADR 0461 (PR #130, renumbered from 0460): 46,529 bytes: 47_000.
  // And ADR 0464's extension of the ADR 0339 line (45,650 bytes): 45_700.
  // And ADR 0465's past-the-root line, replacing the root-identifier one (46,136 bytes): 46_200.
  // And the ADR 0464/0465 review residuals (46,766 bytes): 46_800.
  // Merge of main (PR #129, #130) with fix/tdz-pattern-and-store-chain (ADRs 0464/0465): 48,027 bytes: 48_100.
  // And ADR 0460's review amendment (the setter-exemption note, the assignment form's TS18048 overlap): 45,497 bytes: 45_500.
  // Merge of ADR 0458/0459 (PR #129) with ADR 0460 (PR #130): 46,075 bytes: 46_500.
  // And ADR 0463's refused alias forms and the direct-read TDZ false positive it found: 46,714 bytes: 46_800.
  // And ADR 0463 (local alias bindings; the open shapes): 47,311 bytes: 47_500.
  // And ADR 0466 (callbacks passed by name; the unresolved name forms): 47,774 bytes: 48_000.
  // And ADR 0467 (the apply read of an async source): 48,113 bytes: 48_500.
  // Merge of main (PR #135, ADRs 0464/0465) into the batch: 49,611 bytes: 50_000.
  // And ADR 0470 (the unclaimed apply-JSX code): 50,158 bytes: 50_500.
  // And ADR 0472 (the widened review-r1-owner-mix entry): 51,051 bytes: 51_500.
  // And the batch review notes (assignment callees; RC3, RC1b, SC3001 JSX left open): 51,955 bytes: 52_000.
  // And the batch review notes on the batch: 52,848 bytes: 53_000.
  // And the RC8/RC10 note (component execution boundary, left open): 53,828 bytes: 54_000.
  // And the second batch review (ADR 0475 climb, ADR 0472 fail-open shapes): 54,303 bytes: 54_500.
  // And the UNCOMMITTED_INPUT_OVERWRITTEN not-to-build line: 55,065 bytes: 55_200.
  // And the ADR 0480 hook-member line: 55_600.
  // And the ADR 0482 review notes (annotated readonly erasure, imported store alias): 55,675 bytes: 55_900.
  // And the ADR 0483 shared apply-owner premise line: 56,087 bytes: 56_300.
  // And ADR 0491 (its open-work clause, the writeOverride premise): 56,528 bytes: 56_700.
  // And ADR 0500's known miss (a hook holder that is no member alias): 57_000.
  // And ADR 0506's mount-axis known miss: 57_500.
  // And ADR 0509's two precision trades: 58_500.
  // And ADR 0511's hook-premise wording on the SC5001 line: 58,507 bytes: 58_700.
  "docs/open-work.md": 58_700,
  // Raised for ADR 0378: the file stood 2 bytes under 113_200; for ADR 0379,
  // 14 bytes under 113_800; for ADR 0380, 3 bytes under 114_700; and for the
  // catalog review's ADRs 0381 and 0424-0434, 4 bytes under 115_600; and for ADR 0435,
  // 213 bytes over 127_000; and for ADR 0436, 87 bytes under 127_300;
  // and for ADRs 0437-0438, 132_568 bytes against 129_100;
  // and for ADR 0439, 133_476 bytes against 132_700;
  // and for ADR 0438's async and test-helper extension, 135_756 bytes against 133_600.
  // Raised again for the ADR 0381 and 0424-0441 docs series (catalog renames, ADRs 0440-0441 to come): 145_000.
  // And for the Opus review's fixes to ADRs 0426, 0436, 0438 and 0442: 150_000.
  // And for ADR 0441: the file stood at 150_325 bytes against 150_000: 151_000.
  // And for ADR 0440: 152_282 bytes against 151_000: 153_000.
  // And for ADR 0443: 154_913 bytes against 153_000: 156_000.
  // And for ADRs 0428-0429's suppression comments: 156_435 bytes against 156_000: 157_000.
  // And for ADRs 0444-0445 (workspace program boundary, census with no entry): 160_120 bytes against 157_000: 161_000.
  // And for ADR 0446 (the reproduction set and the pre-release run): 162_517 bytes against 161_000: 163_500.
  // And for ADR 0447 (veto weight from a veto-off sweep) and ADR 0448 to come (verified coverage): 165_500.
  // origin/main's raises to the same file (instant-edits track): for its ADRs 0382-0400; and for ADRs 0411 and 0412; and
  // for ADR 0413 (the file stood 158,880 bytes); and for ADR 0414
  // (the file stood 160,518 bytes); and for ADR 0415 (the file stood 163,911 bytes);
  // and for ADR 0416 (the file stood 163,998 bytes before its entry);
  // and for ADR 0418 (the file stood 166,991 bytes before its entry); and for
  // ADR 0419 (the file stood 170,124 bytes before its entry and one edit); and for
  // ADR 0420 (the file stood 173,980 bytes before its entry); and for
  // ADR 0421 (the file stood 176,343 bytes before its entry); and for ADR 0422 (the file stood
  // 178,593 bytes before its entry, which removes the CI performance workflow); and for
  // ADR 0423 (the file stood 179,066 bytes before its entry, which adds the real-project gate);
  // and for ADR 0451 (the file stood 181,000 bytes of 181,000 before its entry, which gates moved answers).
  // Merging origin/main (the incremental-daemon track, ADRs 0382-0423) into the catalog-review branch (ADRs 0381, 0424-0448), with ADR 0449: the two sides' raises combine; the merged tree stood at 231,107 bytes: 232_000.
  // And for ADR 0450 (edits on a project with dispatch sites): 235,494 bytes against 232,000: 236_000.
  // And for ADR 0452 (the file stood 231,443 bytes before its entry; 233_295 with its read extension, 234_022 with the default-value case): 235_000.
  // Both together after merging the two branches (ADRs 0450 and 0452): 239,324 bytes: 240_000.
  // And for ADR 0453 (the file stood 231,598 bytes before its entry): 233_000.
  // All three (ADRs 0450, 0452, 0453): 240,224 bytes: 241_000.
  // With main's ADR 0451 (PR #119) merged in: 241,944 bytes: 242_500.
  // And for ADR 0454 (a site in a parameter list; the file stood 241,944 bytes before its entry): 246,068 bytes: 246_500.
  // And ADR 0454's owner-graph follow-up: 247,354 bytes: 247_500.
  // And ADR 0454's reachability/summary-graph follow-up (248,060 bytes): 248_500.
  // And for ADR 0455 (the file stood 248,060 bytes before its entry): 250,959 bytes: 251_000.
  // And ADR 0455's owner and untrack follow-up: 251,652 bytes: 252_000.
  // And ADR 0455's named-closure follow-up: 252,614 bytes: 253_000.
  // And ADR 0455's cleanup amendment: 253,681 bytes: 254_000.
  // And its third-review amendment: 254,425 bytes: 254_500.
  // And the fourth-review returned-cleanup note (254,712 bytes): 254_900.
  // And ADR 0456, the called onCleanup handle (257,507 bytes): 258_000.
  // And ADR 0457 (the file stood 254,712 bytes before its entry): 256,399 bytes: 256_500.
  // And ADR 0457's review amendment (a callee with other uses of the callback): 256,936 bytes: 257_000.
  // With ADR 0456 (PR #123) merged into the ADR 0457 branch: 259,937 bytes: 260_000.
  // And ADR 0458 (unreached sites): 263,202 bytes: 263_300.
  // And ADR 0459, a const destructuring of a namespace import (259,995 bytes before its entry): 261,764 bytes: 262_000.
  // And ADR 0459's review nits (the guarded path, the reassignment premise): 262,293 bytes: 262_500.
  // And ADR 0460, writes through in-place layers (render-effect apply, control-flow row): 264,170 bytes: 264_500.
  // Merge of main's ADR 0458 (PR #128) with ADRs 0459/0460: 267,371 bytes: 267_500.
  // And ADR 0461, `delete` of an optional store property (261,284 bytes): 261_500.
  // And its review amendment (parentheses-only operand, nullish links): 261,636 bytes: 261_750.
  // And ADR 0461's chain-past-the-root sentence (strictNullChecks unrecorded): 261,863 bytes: 262_000.
  // Merge of PR #129 (ADRs 0459/0460) with ADR 0461: 269,238 bytes: 269_500.
  // And ADR 0464 (a site naming a binding before its declarator): 265,215 bytes: 265_300.
  // And ADR 0465 (a store write past the root): 267,212 bytes: 267_300.
  // And its strictNullChecks amendment (Type Facts reports the option): 267,442 bytes: 267_500.
  // And the ADR 0464/0465 review sentences: 268,384 bytes: 268_400.
  // Merge of main (PR #129, #130) with ADRs 0464/0465 and the delete verdict sentence: 274,803 bytes: 274_900.
  // And ADR 0460's chain-past-the-root sentence (strictNullChecks unrecorded): 261,863 bytes: 262_000.
  // Merge of ADR 0458/0459 (PR #129) with ADR 0460 (PR #130): 266,031 bytes: 266_500.
  // And ADR 0462, boundary calls and call-attributed async reads: 268,514 bytes: 269_000.
  // And ADR 0462 (Loading/Errored as calls; call-attributed async reads): 271,721 bytes: 272_000.
  // And ADR 0461, boundary calls and call-attributed async reads: 268,514 bytes: 269_000.
  // And ADR 0463, exact `const` aliases of local reactive bindings: 271,577 bytes: 272_000.
  // And ADR 0463 (local alias bindings): 274,784 bytes: 275_000.
  // And ADR 0466 (callbacks passed by name): 277,426 bytes: 277_500.
  // And ADR 0467: 279,103 bytes: 279_500.
  // Merge of main (PR #135, ADRs 0464/0465) into the batch: 284,667 bytes: 285_000.
  // And ADR 0468 (callee spelling; unnamed callees are obligations): 287,038 bytes: 287_500.
  // And ADR 0469 (reads through in-place layers): 288,842 bytes: 289_000.
  // And ADR 0470 (a read in JSX a callback writes): 290,136 bytes: 290_500.
  // And ADR 0471 (a closure in a root's body): 292,179 bytes: 292_500.
  // And ADR 0472 (returned callables are uncertifiable): 293,882 bytes: 294_000.
  // And ADR 0474 (helper-called callbacks in the owner graph): 291,622 bytes: 292_000.
  // And ADR 0474 picked onto the batch: 295,368 bytes: 295_500.
  // And ADR 0475 (a creation the runtime refuses enters nothing): 297,044 bytes: 297_500.
  // And the ADR 0474 guard amendment: 297,529 bytes: 298_000.
  // And the second batch review amendments (ADRs 0474, 0475): 298,019 bytes: 298_500.
  // And ADR 0476 (the setter window, past-root writes): 300,074 bytes: 300_500.
  // And ADR 0477 (the await-continuation dispatch): 301,796 bytes: 302_000.
  // And ADR 0478 (flush-in-action through in-place layers): 302,714 bytes: 302_900.
  // And ADR 0479 (a named function with one call runs in place): 303,868 bytes: 304_000.
  // And ADR 0480 (literal slots as exact aliases) and the ADR 0479 review amendment: 306041 bytes: 306_800.
  // And ADR 0481 (member reads of an exclusive arrow result): 307,159 bytes: 307_400.
  // And ADR 0482 (a member alias is a root for the ported rules): 308,347 bytes: 308_600.
  // And ADR 0483 (SC6002 primitive-in-effect-callback): 309,927 bytes: 310_200.
  // And the ADR 0483 premise amendment: 310_600.
  // And ADR 0484 (the effect-phase walk crosses nested closures and helpers): 311,241 bytes: 311_500.
  // And ADR 0485 (a helper's in-place callback after an await): 312,340 bytes: 312_600.
  // And ADR 0486 (directly returned accessors are summary-eligible): 313,445 bytes: 313_700.
  // And ADR 0487 (the read-after-await rule reads member aliases): 314_600.
  // And ADR 0488 (a settled cleanup after an await is unowned): 315_500.
  // And ADR 0489 (the stored-literal walk follows a named function's one call): 316_500.
  // And ADR 0490 (an async read in a named local function is unsuspendable where its call is): 318_000.
  // And ADR 0491 (a store link its own initializer wrote is wrapped): 320_000.
  // And ADR 0492 (a leaf owner's callback runs helper callbacks in place): 321_800.
  // And ADRs 0493-0495 (layers after an await, resolve/until layers, SC6002 per layer): 325_200.
  // And the ADR 0493-0495 review amendments: 326_500.
  // And ADR 0496 (the call-attributed async walk starts from any named local function): 327_800.
  // And ADR 0497 (the store-link proof follows literal slots): 329_400.
  // And the ADR 0497 review note and ADR 0498 (the object of a written member is read): 331_800.
  // And ADR 0499 (a store link read in a closure member write): 333_400.
  // And ADR 0500 (the store-link proof follows a member-read hook holder): 334_800.
  // And the ADR 0500 review note: 335_700.
  // And ADR 0501 (the fuzzer entry renders each component by name): 336_800.
  // And ADR 0502 (schedule withholds a first run): 337_800.
  // And ADR 0503 (a render effect first run under a module-scope render): 339_800.
  // And the ADR 0503 review and ADR 0504 (an apply first read of a pending source): 343_500.
  // And ADR 0506 (reachability of function literals, call-result bindings) with its review: 345_000.
  // And ADR 0507 (a child an inert parent renders is mounted as its parent is) with its review: 347_000.
  // And ADR 0508 (<Show when={true}>, a one-row <For>) with its review: 349_500.
  // And ADR 0509 (values a caller hands over): 352_500.
  // And ADR 0510 (module-scope and parent-handed async memos, layered first-run premise): 356_000.
  // And ADR 0510's review (non-call throws, a second root, exact import): 357_000.
  // And ADR 0511's entry (props through layers): 357,695 bytes: 358_000.
  // And its hook-created memo bullet: 358,994 bytes: 359_500.
  "docs/decisions.md": 359_500,
};

export const GROUP_BUDGETS = [
  // Raised from 600_000 for ADRs 0382-0404 (instant-edits track), and for
  // ADRs 0411 and 0412, and for ADR 0413
  // (the group stood 628,977 bytes); for ADR 0414 (the group stood 631,357); and for
  // ADR 0415 (the group stood 635,538 bytes); and for ADR 0416
  // (the group stood 635,458 bytes before the entry, which added 2,578); and for ADR 0418
  // (the group stood 638,451 bytes before the entry and the open-work lines, which added 3,696); and for
  // ADR 0419 (the group stood 642,147 bytes before its entry, the open-work edit and the
  // protocol notes, which added 4,638); and for ADR 0420 (the group stood 646,785 bytes before
  // its entry, which added 2,363); and for ADR 0421 (the group stood 649,148 bytes before
  // its entry, which added 2,250).
  // and for ADR 0422 (the group stood 650,999 bytes before its entry, which added about 800);
  // and for ADR 0423 (the group stood 651,778 bytes before its entry, which added 1,762);
  // and for ADR 0451 (the group stood 653,800 bytes against 653,800 before its entry, which added about 1,500).
  // Raised for ADRs 0437-0438 and their rule-page mechanics: docs/ stood at 604_388 bytes.
  // And for ADR 0439 and its two rule pages: 607_028 bytes against 605_000.
  // And for ADR 0438's extension (decisions, open work, two rule pages): 610_122 bytes.
  // Raised again for the ADR 0381 and 0424-0441 docs series (catalog renames, ADRs 0440-0441 to come): 650_000.
  // And for ADRs 0440-0443 and their rule pages: 650_087 bytes against 650_000: 656_000.
  // And for ADRs 0428-0429 (suppression section, SC9016's page): 658_058 bytes against 656_000: 660_000.
  // And for ADRs 0444-0445 (decisions, open work, fixture READMEs): 662_349 bytes against 660_000: 664_000.
  // And for ADR 0446 (the reproduction set and the pre-release run): 665_446 bytes against 664_000: 667_000.
  // And for ADRs 0447-0448 (see above): 669_000.
  // Merging origin/main (the incremental-daemon track, ADRs 0382-0423) into the catalog-review branch (ADRs 0381, 0424-0448), with ADR 0449: the two sides' raises combine; the merged tree stood at 739,773 bytes: 741_000.
  // And for ADR 0450: 744,869 bytes against 741,000: 745_500.
  // And for ADR 0452 (its entry and rule-page paragraphs): 743_600 bytes against 741_000: 744_500.
  // Both together (ADRs 0450 and 0452): 751,131 bytes: 752_000.
  // And for ADR 0453 (the group stood 740,735 bytes before its entry): 742_000.
  // All three (ADRs 0450, 0452, 0453): 752,031 bytes: 753_000.
  // With main's ADR 0451 (PR #119) merged in: 753,780 bytes: 754_500.
  // And for ADR 0454 (its entry, the open-work lines, a rule-page paragraph): 758,425 bytes against 754,500: 759_000; its owner-graph follow-up: 760,511 bytes: 761_000; its reachability follow-up: 761,579 bytes: 762_500.
  // And for ADR 0455 (its entry, one open-work line, a rule-page paragraph): 765,586 bytes against 762,500: 766_000.
  { name: "docs/", test: (path) => path.startsWith("docs/"), bytes: 911_500 }, // ADR 0511 hook-created memos (its bullet, a rule-page clause, open-work wording): 910,833 bytes. // ADR 0511 (its entry, a rule-page clause, the open-work line): 909,383 bytes. // ADR 0510 review (its paragraph). // ADR 0510 (its entry, a rule-page paragraph, two open-work lines). // ADR 0509. // ADR 0508. // ADRs 0505-0507 (entries, rule-page and open-work lines). // ADR 0503 review, ADR 0504. // ADR 0503 (entry, rule-page paragraph). // ADR 0502. // ADR 0501. // ADR 0500 review. // ADR 0500. // ADR 0499. // ADR 0497 review, ADR 0498. // ADR 0497 (its entry). // ADR 0496 (its entry). // ADR 0493-0495 review amendments. // ADRs 0493-0495 (entries, three rule-page clauses). // ADR 0492 (its entry, a rule-page clause). // ADR 0491 (its entry and review, a rule-page clause, the open-work line). // ADR 0490 (its entry, a rule-page paragraph). // ADR 0489 (its entry). // ADR 0488 (its entry). // ADR 0487 (its entry). // ADR 0486 (its entry, a rule-page clause). // ADR 0485 (its entry, a rule-page sentence): 857,477 bytes. // ADR 0484 (its entry, a rule-page clause): 856,113 bytes. // ADR 0483 premise lines (open-work, rule pages, ADR). // ADR 0483 (its entry, the SC6002 rule page, a README row): 853,675 bytes. // ADR 0482 review notes in open-work: 849,875 bytes. // ADR 0482 (its entry, a rule-page paragraph, the open-work line): 849,509 bytes. // ADR 0481 (its entry, the open-work line): 848,204 bytes. // ADRs 0479 amendment and 0480 (entries, one open-work line). // ADR 0479 (its entry): 844,490 bytes. // ADR 0478 (its entry, a rule-page paragraph): 843,336 bytes. // The UNCOMMITTED_INPUT_OVERWRITTEN not-to-build line: 842,068 bytes. // ADR 0477 (its entry, a rule-page sentence): 841,485 bytes. // ADR 0476 store-alias amendment (an ADR clause, a rule-page clause): 839,881 bytes. // ADR 0476 review (its sentence, the residual false-positive line): 839,649 bytes. // ADR 0476 (its entry, two rule-page paragraphs, two open-work lines): 839,107 bytes. // Second batch review: 836,062 bytes. // ADR 0474 guard amendment (its sentence, a rule-page sentence): 835,097 bytes. // RC8/RC10 open-work note: 834,391 bytes. // ADR 0475 (its entry, a rule-page sentence, one open-work line): 833,477 bytes. // Batch review notes on the batch: 831,469 bytes. // ADR 0474 on the batch: 829,668 bytes. // ADR 0472 (its entry, a rule-page paragraph, open-work): 827,781 bytes. // ADR 0471 (its entry, a rule-page sentence, one open-work line): 825,165 bytes. // ADR 0470 (its entry, a rule-page bullet, one open-work line): 822,681 bytes. // ADR 0469 (its entry, a rule-page bullet): 820,739 bytes. // ADR 0468 (its entry, one open-work line): 818,373 bytes. // Merge of main (PR #135) into the batch: 815,691 bytes. // ADR 0467 (its entry, two rule-page paragraphs, one open-work line): 807,212 bytes. // ADR 0466 (its entry, one open-work line, a rule-page paragraph): 804,218 bytes. // ADR 0463 (its entry, open-work lines, rule-page sentences): 800,445 bytes. // ADR 0462 (its entry, two rule-page paragraphs, one open-work line): 796,067 bytes. // Merge of PR #129 with ADR 0461 (PR #130): 792,496 bytes. // ADR 0461 review amendment (ADR sentences, open-work lines, rule-page sentences): 782,004 bytes. // ADR 0461 (its entry, the open-work line, a rule-page paragraph): 780,658 bytes. // Merge of ADR 0456 (PR #123) with ADR 0457: 778,757 bytes. // ADR 0456 (its entry, open-work lines, a rule-page paragraph): 775,714 bytes. // ADR 0457 review amendment (its sentences, one open-work line): 774,199 bytes against 773,500. // ADR 0457 (its entry, a rule-page sentence): 773,270 bytes against 771,500. // ADR 0455 fourth-review amendment: 771,408 bytes. // ADR 0455 third-review amendment: 770,819 bytes. // ADR 0455 cleanup amendment: 769,241 bytes. // ADR 0455 named-closure follow-up: 767,101 bytes. // ADR 0455 follow-up: 766,106 bytes against 766,000.
  {
    name: "fixture READMEs",
    test: (path) => path.startsWith("fixtures/") && path.endsWith("/README.md"),
    // Raised from 250 KB for the context-read fixtures (ADR 0352): the group was
    // 249.8 KB before they landed, and each case README is a claim the reader needs.
    // Raised again for ADR 0378's fixture: the group stood 1 byte under 256 KB.
    // And for ADR 0435's nine dispatch fixtures: the group stood 455 bytes under 257 KB.
    // And for ADR 0436's four spread fixtures: four READMEs took the group 2_395 bytes past 261 KB.
    // And for ADRs 0437-0438's fixtures (five handler READMEs, three test-runner projects): 271_432 bytes.
    // And for ADR 0439's fixture: 271_811 bytes against 271_500.
    // And for ADR 0438's async and helper variants and the vitest 4 project: 275_151 bytes.
    // Raised again for the ADR 0381 and 0424-0441 docs series (catalog renames, ADRs 0440-0441 to come): 290_000.
    // And for the Opus review's fixes (suite bodies per runner, after-await and sync-dispatch variants): 295_000.
    // And for ADRs 0428-0429's fixtures (group key, suppression comments): 297_071 bytes against 295_000: 298_000.
    // And for ADRs 0444-0445's fixtures (workspace boundary, census with no entry): 298_292 bytes against 298_000: 299_500.
    // And for ADR 0452's two fixtures: 301_497 bytes against 299_500: 302_000.
    // And ADR 0452's default-value cases: 302_229 bytes against 302_000: 303_000.
    // And the pinned false positive's fixture: 303_850 bytes against 303_000: 304_500.
    // And ADR 0454's fixture: 305,605 bytes against 304,500: 306_000.
    // And the owner-graph cases of the same fixture: 306,455 bytes: 307_000.
    // And ADR 0455's two fixtures: 309,348 bytes against 307,000: 309_500.
    // And ADR 0455's follow-up cases: 309,906 bytes: 310_000.
    // And its named-closure cases: 310,410 bytes: 310_500.
    // And the cleanup-disposal fixture (ADR 0455 amendment): 311,225 bytes: 311_500.
    // And the cleanup-disposal release and await cases (ADR 0455 amendment): 312,992 bytes: 313_000.
    // And the third review's cleanup cases (ADR 0455 amendment): 313,739 bytes: 314_000.
    // And the fourth review's returned-cleanup cases: 314,169 bytes: 314_500.
    // And ADR 0456's handle fixture and the called-handle cases: 316,292 bytes: 316_500.
    // With ADR 0456 (PR #123) merged into the ADR 0457 branch: 316,797 bytes: 317_000.
    // And ADR 0458's dead-code-sites fixture: 318,662 bytes: 318_800.
    // And ADR 0459's fixture (namespace-destructured-api) and flush-fixture lines: 318,351 bytes: 318_500.
    // And its review nits (the fixture's added cases): 318,612 bytes: 318_700.
    // And ADR 0460's fixture (write-render-apply-row-layers): 320,112 bytes: 320_500.
    // And its review's await cases: 320,562 bytes: 321_000.
    // Merge with main's ADR 0458 (dead-code-sites): 322,225 bytes: 322_500.
    // And ADR 0461's fixture (store-delete): 318,360 bytes: 318_500.
    // And its review amendment (Rejected.tsx, nullish links): 318,816 bytes: 319_000.
    // And the store-delete README's uncertifiable nested paths: 319,109 bytes: 319_500.
    // Merge of PR #129 with ADR 0461 (store-delete): 324,335 bytes: 324_500.
    // And ADR 0464's destructured-binding-tdz fixture: 320,170 bytes: 320_300.
    // And ADR 0465's store-write-chain-links fixtures and the moved READMEs: 323,215 bytes: 323_300.
    // Merge with fix/tdz-pattern-and-store-chain (destructured-binding-tdz, store-write-chain-links, store-delete): 328,710 bytes: 328_800.
    // Merge of PR #129 with PR #130: 322,672 bytes: 323_000.
    // And ADR 0462's two fixtures (boundary-call-owner, async-boundary-call-attributed): 324,397 bytes: 324_500.
    // And ADR 0462's fixtures (boundary-call-owner, async-boundary-call-attributed): 326,060 bytes: 326_500.
    // And ADR 0461's two fixtures (boundary-call-owner, async-boundary-call-attributed): 324,397 bytes: 324_500.
    // And ADR 0463's fixture (local-alias-bindings) and two README sentences: 326,391 bytes: 326_500.
    // And ADR 0463's fixture (local-alias-bindings): 328,102 bytes: 328_500.
    // And ADR 0466's fixture (write-callback-by-name) and two README updates: 330,275 bytes: 330_500.
    // And ADR 0467's fixture (strict-read-async-apply): 331,534 bytes: 332_000.
    // Merge of main (PR #135) into the batch: 335,905 bytes: 336_500.
    // And ADR 0468's fixture (callee-spelling): 337,833 bytes: 338_000.
    // And ADR 0469's fixture (read-in-place-layers): 339,576 bytes: 340_000.
    // And ADR 0470's fixture (read-jsx-in-callbacks): 340,841 bytes: 341_000.
    // And ADR 0471's fixture (write-named-closure-in-root): 342,615 bytes: 343_000.
    // And ADR 0472's fixture (owner-returned-callable): 343,860 bytes: 344_000.
    // And ADR 0474's fixture (owner-helper-callback): 342,136 bytes: 342_500.
    // And ADR 0474's fixture on the batch: 345,155 bytes: 345_500.
    // And ADR 0475's fixture (write-in-refused-creation): 346,275 bytes: 346_500.
    // And the ADR 0474 guard amendment (owner-helper-callback): 346,892 bytes: 347_000.
    // And ADR 0476's store-setter-extent fixture, with its review cases: 347,798 bytes: 347_900.
    // And ADR 0477's four event-dispatch-await fixtures: 349,709 bytes: 349_900.
    // And ADR 0478's rc9-flush-in-action-layers fixture: 350,331 bytes: 350_500.
    // And ADRs 0479 and 0480's fixtures (write-named-function-one-call, local-alias-literal-slots): 352_300.
    // And ADR 0481's local-alias-member-reads fixture: 352,734 bytes: 352_900.
    // And ADR 0482's cases in local-alias-member-reads: 353,012 bytes: 353_200.
    // And ADR 0483's primitive-in-effect-callback fixture: 353,690 bytes: 353_900.
    // And ADR 0484's effect-phase-layers fixture: 354,315 bytes: 354_500.
    // And ADR 0485's after-await-helper-callback fixture: 354,904 bytes: 355_100.
    // And ADR 0488's settled-cleanup-after-await fixture: 355_600.
    // And ADR 0490's async-unsuspendable-call-attributed fixture: 356,247 bytes: 356_500.
    // And ADR 0491's store-write-literal-links fixture: 357_500.
    // And ADR 0492's leaf-scope-helper-callback fixture: 358_800.
    // And ADR 0493's owner-after-await-layers fixture, 0494/0495 README edits: 360_500.
    // And ADRs 0496/0497 README rows: 361_200.
    // And ADR 0499's local-alias-member-reads paragraph: 361_800.
    // And ADR 0500's store-write-literal-links row and review: 362_500.
    // And ADR 0503's render-apply-first-run fixture: 363_700.
    // And ADR 0503's review rows and ADR 0504's strict-read-async-apply paragraph: 365_200.
    // And ADR 0505's effect-apply-first-run fixture and strict-read-async-apply sentence: 366_000.
    // And ADR 0506's private-component-layers and call-result-binding fixtures and two moved-row notes: 368_500.
    // And ADR 0507's child-mount-first-run fixture: 369_500.
    // And ADR 0508's child-mount-first-run rows: 370_500.
    // And ADR 0509's three fixtures: 372_500.
    // And ADR 0510's pending-source-first-run fixture: 374_500.
    // And ADR 0510's review variants (body-member, apply-member, second-root): 375_000.
    // And ADR 0511's pending-source-layers fixture: 375,529 bytes: 376_000.
    bytes: 376_000,
  },
  { name: "skills", test: (path) => path.startsWith(".claude/skills/"), bytes: 25_000 },
];

/** One fixture README is a claim, not a report. */
export const FIXTURE_README_MAX = 8_000;

/**
 * What docs/ may hold. A new top-level document is how the append logs came
 * back last time, so adding one is a deliberate edit here.
 */
export const DOCS_ALLOWED = [
  "docs/README.md",
  "docs/adding-a-dialect.md",
  "docs/compiler-facts.md",
  "docs/decisions.md",
  "docs/development-feedback.md",
  "docs/monorepo.md",
  "docs/open-work.md",
  "docs/typefacts.md",
  "docs/rules/",
  "docs/typefacts/",
  "docs/release-audits/",
];

/** Every violation for a `{ path: bytes }` map, as sentences. */
export function violations(sizes) {
  const found = [];
  for (const [path, limit] of Object.entries(FILE_BUDGETS)) {
    if ((sizes[path] ?? 0) > limit) found.push(`${path} is ${sizes[path]} bytes (budget ${limit})`);
  }
  for (const group of GROUP_BUDGETS) {
    const total = Object.entries(sizes)
      .filter(([path]) => group.test(path))
      .reduce((sum, [, bytes]) => sum + bytes, 0);
    if (total > group.bytes) found.push(`${group.name} total ${total} bytes (budget ${group.bytes})`);
  }
  for (const [path, bytes] of Object.entries(sizes)) {
    if (path.startsWith("fixtures/") && path.endsWith("/README.md") && bytes > FIXTURE_README_MAX) {
      found.push(`${path} is ${bytes} bytes (a fixture README states its claim; budget ${FIXTURE_README_MAX})`);
    }
    if (
      path.startsWith("docs/") &&
      !DOCS_ALLOWED.some((allowed) => (allowed.endsWith("/") ? path.startsWith(allowed) : path === allowed))
    ) {
      found.push(`${path} is not an allowed docs/ location`);
    }
  }
  return found;
}

/** Tracked and untracked-but-not-ignored files under the budgeted roots. */
export function measuredSizes(root = ROOT) {
  const listed = execFileSync(
    "git",
    ["ls-files", "-z", "--cached", "--others", "--exclude-standard", "--", "AGENTS.md", "CLAUDE.md", "docs", "fixtures", ".claude/skills"],
    // Package-source fixtures ship whole published packages.
    { cwd: root, encoding: "utf8", maxBuffer: 256 * 1024 * 1024 },
  );
  const sizes = {};
  for (const path of listed.split("\0").filter(Boolean)) {
    if (path.startsWith("fixtures/") && !path.endsWith("/README.md")) continue;
    try {
      sizes[path] = statSync(resolve(root, path)).size;
    } catch {
      // Deleted in the working tree but still in the index: it costs nothing.
    }
  }
  return sizes;
}

if (process.argv[1] && resolve(process.argv[1]) === resolve(fileURLToPath(import.meta.url))) {
  const found = violations(measuredSizes());
  if (found.length > 0) {
    console.error(
      `doc-budget: ${found.length} violation(s):\n  ${found.join("\n  ")}\n` +
        "Slice reports go in the commit message, a design decision in docs/decisions.md, " +
        "an open item in docs/open-work.md. Raise a budget only as a deliberate edit to scripts/doc-budget.mjs.",
    );
    process.exit(1);
  }
  console.log("doc-budget: within budget");
}
