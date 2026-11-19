# disabled-handler-owner-enabled

ADR 1134 negative: the program writes a `.disabled` member (`other.disabled = false` through a ref), so a control disabled from mount
may be enabled; the SC4001 `missing-owner` violation in its handler stays.
