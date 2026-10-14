# Package integrity review: fresh-channels

ADR 0300 review round 1: visible receiver transport through fresh-channels.
The final location.search must remain an SC1001 violation.
Fresh-channels discharges every unrelated target; all other cases veto.
Unknown or escaping inputs never count as unrelated targets.
The unchanged Solid declarations retain the claim-bearing published
2.0.0-rc.13 signatures; the router models its context/location getter shape.
TypeScript --noEmit must be clean.
