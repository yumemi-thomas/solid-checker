# With no nested `render`, an apply has no owner

ADR 0644's control: `onCleanup` warns `NO_OWNER_CLEANUP` (Chrome), so SC4001 stays a violation, and a write stays legal.
