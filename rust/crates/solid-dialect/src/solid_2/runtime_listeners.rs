//! ADR 1151: what each export of the Solid 2 runtime does to the listeners a focus move delivers to ([`RuntimeListenerRole`]), and the answer
//! [`crate::Dialect::runtime_module_listener_registrations`] gives for the root and the two JSX runtimes (one file).
//!
//! Audited against the exact published `@solidjs/web@2.0.0-rc.21` (`dist/web.js`, `frames/dist/client.js`, `frames/dist/regions.js`,
//! `frames/dist/assets.js`, `server-functions/dist/client.js`) and `solid-js@2.0.0-rc.21` (`dist/solid.js`), by the three things a focus move
//! can run code through:
//!
//! - a listener registered on the element, the window or the document: in `web.js` only `addEvent` (`node.addEventListener(name, handler)`, or the
//!   delegated slot `node.$$<name> = handler` the document listener `attachDelegatedEvent` reads), `delegateEvents` and
//!   `registerDelegatedContainer`/`registerDelegatedRoot` (which attach the document-level delegated listeners), the asset elements' `load` and
//!   `error` listeners (`acquireAsset`, `warmAsset`, `useHead`), `DOMContentLoaded` for hydration, and `writeSelectValue`'s capture `change` listener
//!   on a `<select>`, which runs the runtime's own `selectChange` and no project code. `solid-js` registers `DOMContentLoaded` only
//!   (`dist/solid.js`: its hydration sweep). The frames client wires `DEFER_EVENTS` listeners and dispatches `FRAME_APPLIED_EVENT`;
//!   the server-function client's `online` and `abort` listeners are not an element's;
//! - markup written from a string: `template` (`t.innerHTML = html`) and the frames client's `template.innerHTML = html` (every export of the
//!   frame, region and server-component surface is `Opaque`);
//! - a property or an attribute written under a name the caller gives: `setProperty`, `setStatefulProperty`, `setAttribute`, `setAttributeNS`, and, under
//!   the keys of an object, `assign` and `spread` (which `Dynamic` and `dynamic` call with the props they are handed).
//!
//! Every other export of the audited surface is inert: it renders, reads, hands a value on or runs a callback the program wrote (a `ref`, an
//! effect, a component), whose body is a program file the engine reads. The table holds one row per export name of
//! [`crate::exports::solid_v2_solidjs_web::VALUES`] (`None` is an inert one), and a test holds the two to the same names, so an export added to the
//! audited surface has no row until someone reads it, which the test refuses; a second test holds the table against every name the pinned dist publishes.
//! A name only `solid-js` exports is inert (`solid-js` has its own answer).

use crate::RuntimeListenerRole;
use RuntimeListenerRole::{Event, Forwards, Markup, Named, Opaque};

/// The role of every `@solidjs/web` export name (a name is the same function whichever subpath exports it), sorted by name; `None` is inert.
#[rustfmt::skip]
pub(super) static WEB_ROLES: &[(&str, Option<RuntimeListenerRole>)] = &[
    ("ChildProperties", None),
    ("ChunkReader", None),
    ("DEFAULT_WEB_PLUGINS", None),
    ("DOMElements", None),
    ("DOMWithState", None),
    ("DelegatedEvents", None),
    ("Dynamic", Some(Forwards)),
    ("ERROR_HEADER", None),
    ("Errored", None),
    ("FLASH_COOKIE", None),
    ("FRAME_APPLIED_EVENT", None),
    ("FRAME_STREAM_HEADER", None),
    ("FUNCTION_HEADER", None),
    ("For", None),
    ("GENERIC_SERVER_ERROR_MESSAGE", None),
    ("GET", None),
    ("HREF", None),
    ("Hydration", Some(Opaque)),
    ("HydrationScript", Some(Opaque)),
    ("INSTANCE_HEADER", None),
    ("Loading", None),
    ("Match", None),
    ("MathMLElements", None),
    ("Namespaces", None),
    ("NoHydration", Some(Opaque)),
    ("OpaqueReference", None),
    ("Portal", None),
    ("REVALIDATE_HEADER", None),
    ("RawTextElements", None),
    ("Repeat", None),
    ("RequestContext", None),
    ("ResponseEnvelope", None),
    ("Reveal", None),
    ("SAFE_ERROR", None),
    ("SERVER_COMPONENT_BOOTSTRAP", None),
    ("SINGLE_FLIGHT_HEADER", None),
    ("SVGElements", None),
    ("ServerComponentPlugin", None),
    ("Show", None),
    ("Switch", None),
    ("VoidElements", None),
    ("acquireAsset", Some(Opaque)),
    ("addEvent", Some(Event { argument: 1 })),
    ("applyFrameResponse", Some(Opaque)),
    ("applyRef", None),
    ("assign", Some(Opaque)),
    ("asyncArg", None),
    ("claimElement", Some(Opaque)),
    ("claimElementTree", Some(Opaque)),
    ("className", None),
    ("clearFlashCookie", None),
    ("clientOnly", None),
    ("commitEventResponse", Some(Opaque)),
    ("composeMiddleware", None),
    ("configureServerFunctionsClient", None),
    ("configureServerFunctionsServer", None),
    ("createChunk", None),
    ("createComponent", None),
    ("createFrame", Some(Opaque)),
    ("createFrameElement", Some(Opaque)),
    ("createFrameHost", Some(Opaque)),
    ("createFrameSink", None),
    ("createHydrationSerializer", None),
    ("createJSONDataTable", None),
    ("createJSONDeserializer", None),
    ("createJSONSerializer", None),
    ("createNoJSHandler", None),
    ("createPlugin", None),
    ("createRequestEvent", None),
    ("createResponseStub", None),
    ("createSSRResponse", None),
    ("createSerializer", None),
    ("createServerComponentHandler", Some(Opaque)),
    ("createServerReference", None),
    ("decodeErrorHeaderValue", None),
    ("decodeFlashCookie", None),
    ("decodeResponse", None),
    ("decodeResponsePayload", None),
    ("delegateEvents", Some(Opaque)),
    ("deserializeStream", None),
    ("dispatchAsInteraction", Some(Opaque)),
    ("dynamic", Some(Forwards)),
    ("dynamicComponent", Some(Forwards)),
    ("dynamicProperty", None),
    ("effect", None),
    ("enableRichArguments", None),
    ("encodeErrorHeaderValue", None),
    ("encodeFlashCookie", None),
    ("escape", None),
    ("foldSetCookies", None),
    ("frameAddress", None),
    ("frameTransformDirectResult", None),
    ("frameTransformFlightResult", None),
    ("frameTransformResult", None),
    ("generateHydrationScript", Some(Opaque)),
    ("getDelegatedRoot", None),
    ("getEventServerFunctionInvocation", None),
    ("getExpectedRedirectStatus", None),
    ("getFlightDataConsumer", None),
    ("getFrameHost", Some(Opaque)),
    ("getHydrationKey", Some(Opaque)),
    ("getLocalHeaderScript", None),
    ("getNextElement", Some(Opaque)),
    ("getNextMarker", Some(Opaque)),
    ("getNextMatch", Some(Opaque)),
    ("getOwner", None),
    ("getRequestEvent", None),
    ("getServerFunction", None),
    ("getServerFunctionInvocation", None),
    ("getServerFunctionMetadata", None),
    ("getServerFunctionRPC", None),
    ("getServerFunctionsCodec", None),
    ("handleServerFunctionRequest", None),
    ("hasFlashCookie", None),
    ("hasServerLinkState", None),
    ("httpHeader", None),
    ("httpStatus", None),
    ("hydrate", Some(Opaque)),
    ("insert", None),
    ("installServerComponentHydration", Some(Opaque)),
    ("installServerComponents", Some(Opaque)),
    ("isDev", None),
    ("isFrameStreamResponse", None),
    ("isHref", None),
    ("isResponseEnvelope", None),
    ("isSafeError", None),
    ("isServer", None),
    ("isServerFunction", None),
    ("markSafeError", None),
    ("memo", None),
    ("mergeProps", None),
    ("parseCookieHeader", None),
    ("provideRequestEvent", None),
    ("redirect", None),
    ("ref", None),
    ("registerDelegatedContainer", Some(Opaque)),
    ("registerDelegatedRoot", Some(Opaque)),
    ("registerElementClaim", Some(Opaque)),
    ("registerServerFunction", None),
    ("registerServerReference", None),
    ("reload", None),
    ("render", None),
    ("renderServerComponent", None),
    ("renderToFrameStream", None),
    ("renderToStream", None),
    ("renderToString", None),
    ("resolveSSRNode", None),
    ("resolveSerializerPlugins", None),
    ("respond", None),
    ("runHydrationEvents", Some(Opaque)),
    ("sanitizeServerError", None),
    ("scope", None),
    ("serializeCookie", None),
    ("serializeJSON", None),
    ("serializeString", None),
    ("serverComponentResponse", None),
    ("setAttribute", Some(Named { argument: 1 })),
    ("setAttributeNS", Some(Named { argument: 2 })),
    ("setLinkClaim", Some(Opaque)),
    ("setProperty", Some(Named { argument: 1 })),
    ("setServerFunctionsDev", None),
    ("setStatefulProperty", Some(Named { argument: 1 })),
    ("setStyleProperty", None),
    ("spread", Some(Opaque)),
    ("ssr", None),
    ("ssrAttribute", None),
    ("ssrClassName", None),
    ("ssrElement", None),
    ("ssrGroup", None),
    ("ssrHydrationKey", None),
    ("ssrStyle", None),
    ("ssrStyleProperty", None),
    ("style", None),
    ("subscribeFlightData", None),
    ("template", Some(Markup { argument: 0 })),
    ("unregisterDelegatedContainer", None),
    ("unregisterDelegatedRoot", None),
    ("untrack", None),
    ("useHead", Some(Opaque)),
    ("warmAsset", Some(Opaque)),
    ("withMeta", None),
];

/// The exports of the root module that register a listener or write markup, with their roles: every row of [`WEB_ROLES`] that is not inert and that the
/// root exports (the frames tier's own exports are `Any` under its subpaths).
pub(super) fn root_registrars() -> &'static [(&'static str, RuntimeListenerRole)] {
    static REGISTRARS: std::sync::OnceLock<Vec<(&'static str, RuntimeListenerRole)>> =
        std::sync::OnceLock::new();
    REGISTRARS
        .get_or_init(|| {
            WEB_ROLES
                .iter()
                .filter_map(|(name, role)| role.map(|role| (*name, role)))
                .filter(|(name, _)| {
                    crate::exports::solid_v2_solidjs_web::VALUES
                        .binary_search_by(|(row, _)| (*row).cmp(name))
                        .is_ok_and(|index| {
                            crate::exports::solid_v2_solidjs_web::VALUES[index]
                                .1
                                .contains(&"@solidjs/web")
                        })
                })
                .collect()
        })
        .as_slice()
}
