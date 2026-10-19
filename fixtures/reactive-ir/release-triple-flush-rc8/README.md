# release-triple-flush-rc8

SC2006 `flush-in-action` on `2.0.0-rc.8`, the first throwing release: `@solidjs/signals@2.0.0-rc.8` added the `flush` guard (`dist/dev-shared.js:1904-1913`) and `action` step brackets (`dist/dev.js:1682-1690`); rc.7 has neither. The sync- and async-generator heads report SC2006, the after-`await` call is silent, plus SC9014. `flush`/`action` are transcribed byte-faithfully from rc.8.
