# rc9-dynamic-static

`@solidjs/web@2.0.0-rc.9`'s `dynamic(source, options?)` begins `if (options?.static) return staticDynamic(untrack(source))` (`dist/web.dev.js:2199`, `dist/web.js:2034`): `static: true` runs the source once, untracked, under the caller's owner with no memo. The dialect models the call form (`Primitive::DynamicStatic`): no/false static option is default (lazy tracked memo), exact `static: true` static, anything else unknown and claiming nothing; static effects report unowned (SC4001) and `StaticWrite` is silent. rc.3/rc.7: `release-triple-dynamic-rc3`, `-rc7`.
