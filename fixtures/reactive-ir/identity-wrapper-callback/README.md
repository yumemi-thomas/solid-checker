A call-valued callback names the literal an identity wrapper hands back. A
project function whose every completion returns one parameter unchanged, and
never reads that parameter any other way, makes `wrap(() => body)` evaluate to
the literal, so the literal is the callback (ADR 0346). The declared, arrow,
imported (another file) and event-handler forms are violations: the callback
runs with no reactive owner, as a leaked effect or an unowned cleanup. The
rejected forms are fixture `identity-wrapper-rejections`.
