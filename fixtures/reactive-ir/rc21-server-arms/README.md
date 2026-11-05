# Server-only and build-constant arms (ADR 0659)

A client rule holds only where a client build runs the site. A site under a proven
`isServer` never runs there, so it is silent. A site under a build constant nothing here
decides is uncertifiable. The client side of each guard keeps its finding.
