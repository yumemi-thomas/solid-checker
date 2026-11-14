# reviewed-vitest-4-1

**Claim (ADR 0975).** The reviewed reading of vitest 4.1's `expect` and `vi` (digests in `solid-facts-backend/src/reviewed_packages.rs`, premise `vitest-4.1`) clears `expect(value).matcher(arguments)` for the matchers it names, handed a value that no foreign accessor can run in, and the `vi.fn` and `vi.stubGlobal` registrations; every other call and value into vitest keeps its SC2002 site (`package-may-call-action`).

- **Cleared** (`Primitives`, `Literals`, `Spies`, `Registrations`): a primitive by the compiler's facts (a literal; a `number`, a `string | undefined`, an enum member, a value typed by a type parameter constrained to a primitive; not a union with an object, an `any`, an `unknown` or an unconstrained type parameter's value); a flat object literal of primitives written in place (`toEqual`, `toStrictEqual`); a spy `vi.fn` made, for the matchers that read its call count (`toHaveBeenCalled`, `toHaveBeenCalledTimes`, `toHaveBeenCalledOnce`); `vi.fn()`, `vi.fn(function-written-in-place)` and `vi.stubGlobal(name, value)`.
- **A registration clears the call, not the value** (`StoredPackageFunction`): `vi.stubGlobal("stepped", step)` runs only vitest's code, but `step` is a function of `fz-step`, which the global now holds and the program may call anywhere, so handing it on is a site as before (`reaches-solid`).
- **Sites** (`Objects`, `Unknowns`, `Unnamed`, `Mocks`): objects, arrays, an unconstrained type parameter's value, a union with an object, `any`, `unknown`, a literal that holds another object, an expected value that is an object; the matchers the reading does not name (`toHaveLength`, `toHaveBeenCalledWith`, a `.not` chain); a call through a mock (`onLoad(1)`); `vi.fn(onLoad)` (an identifier argument) and `vi.spyOn`.

Module scope (`onLoad`, `text`, ...) has no owner, so nothing there is a site.

**Published bytes.** `node_modules` holds, byte for byte, the declaration files TypeScript loads for a vitest test file, vitest's entry (`dist/index.js`, whose other chunks are not copied: the package is not clear, as the whole package is not) and the runtime files the reading rests on, with each package's manifest, copied from the installed packages of the corpus project `probus-hk` (bun.lock integrity below; the digests of the runtime files are in `reviewed_packages.rs`). `node_modules/solid-js` and `@solidjs/*` are the manifests of `action-in-owned-scope-unread-packages`; `node_modules/fz-step` is **not** published bytes: it is written for `StoredPackageFunction`.

| package | integrity |
|---|---|
| `vitest@4.1.11` | `sha512-fhACrNXUidIbGSBr5FlbuBkO7VWC1ZyLl0DO4CU2DrQoAPxX84Ysxs+HeGQpii5lZWV1Q4gBZTTu49mF+A6Edw==` |
| `@vitest/expect@4.1.11` | `sha512-VX2x5vNJXET47KAFzwERI+KRMtTTCSWTfSMKsW7JsUsXV4psq++e3DvZpuTDOpHcxytiDs6p2nhVb2tVDiiUYw==` |
| `@vitest/spy@4.1.11` | `sha512-apNa/prQy2qCeywhnixOHPRCgGNhvg7T4Dapfl1GahLp/R+uhBm5cPyFoNVyqsNd2h1nJxL6BqqdIjiABL60YA==` |
| `@vitest/runner@4.1.11` | `sha512-LztvUgdwMNJMIkj3hQnnxiC2Xy1zNxq928W/xhjCLaNCzqTZOudjwbQf6v9IntZGPw132i2Lq2rgTRZHD3JHNw==` |
| `@vitest/utils@4.1.11` | `sha512-zTCVGpyFsGWBhllOyKlTw/vnr6D9qxsfSDyfbyZmTyjHw5N/VuvzHpHoQjm2ZJzn4RJgx5w4r7V0er69CmLgPQ==` |
| `chai@6.2.2` | `sha512-NUPRluOfOiTKBKvWPtSD4PhFvWCqOi0BGStNWs57X9js7XGTprSmFoz5F0tWhR4WPjNeR9jXqdC7/UpSJTnlRg==` |
| `@types/chai@5.2.3` | `sha512-Mw558oeA9fFbv65/y4mHtXDs9bPnFMZAL/jxdPFUpOHHIXX91mcgEHbS5Lahr+pwZFR8A7GQleRWeI6cGFC2UA==` |
| `@types/deep-eql@4.0.2` | `sha512-c9h9dVVMigMPc4bwTvC5dxqtqJZwQPePsWjPlpSOnojbor6pGqdk541lfA7AqFQr5pB1BRdq0juY9db81BwyFw==` |
| `assertion-error@2.0.1` | `sha512-Izi8RQcffqCeNVgFigKli1ssklIbpHnCYc6AknXGYoB6grJqyeby7jv12JUQgmTAnIDnbck1uxksT4dzN3PWBA==` |
| `@vitest/pretty-format@4.1.11` | `sha512-yiZzPbGTS9Sr/JpFl8zHrcIkAofNbFV6k21vIgQN/cY/oxZeXhJv5sc/MBJ5jFKWmWs+oJHw0UXLZjmf931+Vw==` |
| `@vitest/snapshot@4.1.11` | `sha512-pN7ikn1ON7h8ee4gIAp4AzyK+zBtJPzVbqOgu5LCEh4VaJVbPQcgYQYJIMGQPXVeJJq1fnfazis7a5pFNPahog==` |
| `@vitest/mocker@4.1.11` | `sha512-2XJVD55d1o5AZous5CCGKS74g/riOj9odEt2bQpCVZeblHyHdnMeFl4jl0XjU21stf4mbjUkew2eXQZt65g5CQ==` |
| `expect-type@1.4.0` | `sha512-KfYbmpRm0VbLjEvVa9yGwCi9GI34xvi7A/HXYWQO65CSD2u3MczUJSuwXKFIxlGsgBQizV9q5J9NHj4VG0n+pA==` |
| `tinybench@2.9.0` | `sha512-0+DUvqWMValLmha6lr4kD8iAMK1HzV0/aKnCtWb9v9641TnP/MFb7Pc2bxoxQjTXAErryXVgUOfv2YqNllqGeg==` |
| `tinyrainbow@3.1.1` | `sha512-yau8yJdTt989Mm0Bd/236QnzEiPf2xLLTqUZRUJOo/3CB078LSwzei343DgtJVmfJKJE3TMINY1u42SQsP6mXw==` |
| `@standard-schema/spec@1.1.0` | `sha512-l2aFy5jALhniG5HgqrD6jXLi/rUWrKvqN/qJx6yoJsgKhblVd+iqqU4RCXavm/jPityDo5TCvKMnpjKnOriy0w==` |
| `vite@8.2.2` | `sha512-cFKLV/PRgAUlIRm5WjMjJ86jrftzpqcgH+Us+DS8mI3CDNiH30Whrz8uHL3+MOLPAgqbMBAqWdAHAphOAM+z/Q==` |
