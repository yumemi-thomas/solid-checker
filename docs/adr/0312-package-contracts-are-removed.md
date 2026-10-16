# ADR 0312: Package contracts are removed

Status: accepted, 2026-10-16.

Ordinary analysis uses project and selected dependency source plus the selected
Solid dialect’s reviewed runtime premises. It consumes no catalog, authored
tier, bundled contract or receipt. The owner requested immediate deletion after
P7, without a release wait. P7 (210396ee1) remains the isolated switch-off
revert point; reverting it alone after deletion requires restoring the deleted
implementation commits as well.

The package boundary derives from `package_requirements`. A required behavior
unavailable from source is explicitly uncertifiable (SC9005); it never becomes
safe by absence or a guessed violation. No new host-free inference replaces a
contract view. The WASM checkSync path keeps bare runtime package imports open;
the owned check can analyze files through the shared source pipeline.

Source selection assumes Vite with its Solid plugin and an explicit runtime
target/build. Standard runtime configuration, package integrity (ADR 0300), and
Window-realm premises for browser models remain conditions of certification.
Visible counterevidence and unresolved targets withhold proof.

Consumption, emission, certification, probes, negative authority, historical
measurement infrastructure and the package-contract schemas are deleted.
Type Facts protocol 78 removes invocation/export-value certification reads;
ordinary facts and module/session operations remain. Removed CLI and ESLint
settings refuse with migration messages; no-package-contracts remains a no-op.

The P7 default-mode sweep covered 49 projects: 537 violations before and after,
none added or removed; uncertifiable rows rose from 3,705 to 3,711. This is not
complete recall. The previously recorded kui TopicsRoute, spotify-desk-thing
PlayerControlIcon/PlayerControls contract-only rows and kui context getter stay
open until exact source analysis proves them. No recall work was performed.

This ADR supersedes contract-only decisions in ADR 0001, 0004–0026,
0028–0109, 0112–0126, 0128–0135, 0137, 0139–0148, 0150–0161,
0163–0167, 0169–0175, 0177–0189, 0191, 0198, 0207–0208,
0223–0265 and 0267. Their historical records remain. It supersedes the
contract portions of 0027, 0110 §4, 0149, 0162, 0168, 0268–0270,
0273, 0287–0290, 0284, 0291, 0295–0297, and retires the contract
workflows in 0111, 0197, 0271–0272 and 0274. Independent runtime and
source decisions survive; compiler and runtime audit records remain archival
evidence, not accepted package authority.
