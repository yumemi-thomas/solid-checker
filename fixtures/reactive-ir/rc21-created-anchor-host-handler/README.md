# A created anchor appended elsewhere keeps the veto

ADR 1099's guard. The same anchor is appended to an element `querySelector` returns, which may be inside a JSX element, so the
click may reach a JSX handler on the render's stack, under an owner and inside its strict-read window. Both handlers stay open:
the context read's SC4001 and SC4002, and the counter's SC2001 and SC1001, are uncertifiable.
