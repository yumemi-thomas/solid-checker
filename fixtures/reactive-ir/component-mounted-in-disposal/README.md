# component-mounted-in-disposal

**Claim (ADR 0782).** A component body that only a cleanup mounts runs inside that disposal, where rc.21's write guard stands down: its write is legal. One mount outside a cleanup, or a mount in an event handler, keeps the violation, and an export leaves it open.
