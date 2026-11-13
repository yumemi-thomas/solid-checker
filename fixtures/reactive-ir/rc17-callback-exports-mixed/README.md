# rc17-callback-exports-mixed

solid-js and web rc.17 over signals rc.13: rc.17's callback exports stay unmodelled gaps (uncertifiable write, no SC4001 violation, SC9014). An owner operation in a function handed to an unmodelled wrapper call (`dynamicComponent(() => { createEffect(..); return Plain; })`) keeps an uncertifiable SC4001 row, never a violation: the wrapper enters it, not a render (ADR 0970, B1 of the review of #221).
