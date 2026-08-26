//! Rust client model for the TypeFacts v3 lifecycle protocol.
//!
//! This package contains checker-derived facts only. Structural discovery is
//! owned by `solid-ast-facts`; no regex or TypeScript AST shape is reproduced.

use serde::{Deserialize, Serialize, de::DeserializeOwned};
use sha2::{Digest, Sha256};
use std::fmt;
use std::io::{Read, Write};
use std::num::NonZeroU32;
use std::sync::Arc;
use thiserror::Error;

mod retained_table;
mod session;
mod shared_transition_arena;
pub mod v3;

pub use retained_table::{FactTable, Symbol};
pub use session::{
    AnalysisDemand, Cancellation, DemandGroup, ExchangeTimings, Producer, Session, SessionError,
    TableChanges, UpdateTimings,
};

pub const MAX_MESSAGE_BYTES: usize = 64 << 20;
pub const MAX_NESTING_DEPTH: usize = 32;
pub const MAX_COLLECTION_LENGTH: usize = 1_000_000;
pub const SHA256_PREFIX: &str = "sha256:";

// Fact rows keep heap data behind `Arc`, so persistent retained-table leaves
// can share unchanged values between generations. The wire shape is unchanged;
// `Arc<str>` and `Arc<[T]>` serialize exactly as the string and list they hold.

/// serde `skip_serializing_if` helper for `Arc<[T]>` fields.
fn is_empty_slice<T>(values: &[T]) -> bool {
    values.is_empty()
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SourceHash(Arc<str>);

impl SourceHash {
    #[must_use]
    pub fn of(source: &str) -> Self {
        Self(format!("{SHA256_PREFIX}{:x}", Sha256::digest(source.as_bytes())).into())
    }

    pub fn parse(value: impl Into<String>) -> Result<Self, TypeFactsError> {
        let value = value.into();
        let digest = value
            .strip_prefix(SHA256_PREFIX)
            .ok_or_else(|| TypeFactsError::SourceHash(value.clone()))?;
        if digest.len() != 64
            || !digest
                .as_bytes()
                .iter()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        {
            return Err(TypeFactsError::SourceHash(value));
        }
        Ok(Self(value.into()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SourceHash {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Clone, Debug, Default, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Location {
    pub path: Arc<str>,
    pub end_byte: u64,
    pub start_byte: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Declaration {
    pub name: Arc<str>,
    pub kind: Arc<str>,
    pub location: Location,
}

/// The compiler's call-signature classification for one demanded expression's
/// type.
///
/// [`Callability::UntypedCallable`] is the signature-less function-supertype
/// family: a type the compiler permits calling even though it exposes no call
/// signature to read. lib.es5.d.ts's `Function` interface declares
/// `apply`/`call`/`bind` and no signature of its own, and `CallableFunction`,
/// `NewableFunction`, an alias or interface reaching them, and an intersection
/// containing one all inherit that shape; the compiler resolves such a call
/// through its TS 1.0 §4.12 untyped-call rule and gives it `anySignature`. For
/// a single, non-union type the value is exact: it is a *positive* proof that
/// the type is callable, paired with the absence of any signature, arity, or
/// parameter type to read from it. It is not [`Callability::Unknown`]: a
/// domain was closed. It is not [`Callability::Callable`]: nothing about the
/// call can be checked. At a union the promise is weaker — see Aggregation
/// below.
///
/// It never reaches `object`, `{}`, `Record<string, unknown>`, or an interface
/// that merely declares a `bind` method — none is assignable to `Function` and
/// the compiler refuses to call them, so those stay
/// [`Callability::NonCallable`]. Note the deliberate asymmetry with
/// [`Constructability`]: `new` on this family *is* a compile error
/// (`resolveNewExpression` has no untyped fallback), so the family answers
/// `NonConstructable` there and that answer is the compiler's own.
///
/// Aggregation places it below `Callable` and above `Mixed`: constituents that
/// are all callable in either sense answer the weaker of the two, and any
/// non-callable constituent beside a callable one still answers `Mixed`. That
/// promise is per constituent, not a claim about the union's own call:
/// `Function | (() => void)` still carries one readable, arity-enforced call
/// signature tsc itself enforces (a wrong argument count is TS2554), while
/// `Function | Merged` (two constituents each individually in this family,
/// such as a merged `declare class C {}` and `interface C extends Function
/// {}`) has tsc refuse the call outright (TS2349), because the untyped-call
/// rule's fallback explicitly excludes unions. Either way a consumer reading
/// `UntypedCallable` as "callable, signature unread" only under-checks what it
/// could have proven; it never claims the union's call type-checks.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Callability {
    Callable,
    UntypedCallable,
    NonCallable,
    Mixed,
    Unknown,
}

/// The compiler's construct-signature classification for one demanded
/// expression's type: `new X()` where [`Callability`] answers `X()`.
///
/// It exists because a class's *value* type is otherwise unanswerable. The
/// type system reads a construct signature as *not* a call signature, so
/// `typeof C` for `class C {}` is [`Callability::NonCallable`] while
/// `typeof C === "function"` holds at runtime. This fact is the missing half:
/// that same type is [`Constructability::Constructable`].
///
/// `Unknown` is `any`, `unknown`, `never` or an error type — the checker
/// closed no domain, so this is the *absence* of an answer and not a negative
/// one. `Mixed` is a union holding both a constructable and a
/// non-constructable constituent: proven, and proves neither side.
///
/// The producer answers it over the same constituent partition of the same
/// type as [`Callability`], but the two aggregate *independently*, so a
/// `Mixed` verdict on either does not compose with the other into a
/// per-constituent proof. `(() => void) | number | (new () => X)` answers
/// `Mixed` twice over and still holds a constituent that is neither callable
/// nor constructable.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Constructability {
    Constructable,
    NonConstructable,
    Mixed,
    Unknown,
}

/// Checker-derived runtime value classes for one demanded expression.
///
/// `unknown` means the checker could not provide a closed domain. In that
/// case the three `may_be_*` fields conservatively describe all categories
/// that remain possible. The all-false value is the known empty `never`
/// domain, so absence is represented by `Option<RuntimeValueDomain>` on an
/// entity rather than by this struct's zero value.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[repr(transparent)]
pub struct RuntimeValueDomain(u8);

impl RuntimeValueDomain {
    pub const fn new(
        may_be_callable: bool,
        may_be_undefined: bool,
        may_be_other: bool,
        unknown: bool,
    ) -> Self {
        Self(
            may_be_callable as u8
                | (may_be_undefined as u8) << 1
                | (may_be_other as u8) << 2
                | (unknown as u8) << 3,
        )
    }

    #[must_use]
    pub const fn may_be_callable(self) -> bool {
        self.0 & 1 != 0
    }
    #[must_use]
    pub const fn may_be_undefined(self) -> bool {
        self.0 & 2 != 0
    }
    #[must_use]
    pub const fn may_be_other(self) -> bool {
        self.0 & 4 != 0
    }
    #[must_use]
    pub const fn unknown(self) -> bool {
        self.0 & 8 != 0
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RuntimeValueDomainSerde {
    #[serde(default, skip_serializing_if = "is_false")]
    may_be_callable: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    may_be_undefined: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    may_be_other: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    unknown: bool,
}

impl Serialize for RuntimeValueDomain {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        RuntimeValueDomainSerde {
            may_be_callable: self.may_be_callable(),
            may_be_undefined: self.may_be_undefined(),
            may_be_other: self.may_be_other(),
            unknown: self.unknown(),
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for RuntimeValueDomain {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = RuntimeValueDomainSerde::deserialize(deserializer)?;
        Ok(Self::new(
            value.may_be_callable,
            value.may_be_undefined,
            value.may_be_other,
            value.unknown,
        ))
    }
}

/// Compiler-proven JavaScript primitive possibilities at exactly one demanded
/// expression span. Null and undefined remain distinct so consumers can apply
/// their own runtime policy. Absence on [`EntityFact`] means the fact was not
/// demanded.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[repr(transparent)]
pub struct PrimitiveValueDomain(u16);

impl PrimitiveValueDomain {
    const STRING: u16 = 1;
    const NUMBER: u16 = 1 << 1;
    const BOOLEAN: u16 = 1 << 2;
    const BIG_INT: u16 = 1 << 3;
    const SYMBOL: u16 = 1 << 4;
    const NULL: u16 = 1 << 5;
    const UNDEFINED: u16 = 1 << 6;
    const OBJECT: u16 = 1 << 7;
    const NUMBERS_FINITE: u16 = 1 << 8;
    // Zero is the absent entity-row sentinel. The next bit represents a
    // demanded empty `never` domain; unknown is the all-possibilities value.
    const EMPTY: u16 = 1 << 9;
    const UNKNOWN: u16 = u16::MAX;

    #[allow(clippy::fn_params_excessive_bools, clippy::too_many_arguments)]
    pub const fn new(
        may_be_string: bool,
        may_be_number: bool,
        may_be_boolean: bool,
        may_be_big_int: bool,
        may_be_symbol: bool,
        may_be_null: bool,
        may_be_undefined: bool,
        may_be_object: bool,
        numbers_are_finite: bool,
        unknown: bool,
    ) -> Self {
        if unknown {
            return Self(Self::UNKNOWN);
        }
        let bits = Self::selected(may_be_string, Self::STRING)
            | Self::selected(may_be_number, Self::NUMBER)
            | Self::selected(may_be_boolean, Self::BOOLEAN)
            | Self::selected(may_be_big_int, Self::BIG_INT)
            | Self::selected(may_be_symbol, Self::SYMBOL)
            | Self::selected(may_be_null, Self::NULL)
            | Self::selected(may_be_undefined, Self::UNDEFINED)
            | Self::selected(may_be_object, Self::OBJECT)
            | Self::selected(may_be_number && numbers_are_finite, Self::NUMBERS_FINITE);
        if bits == 0 {
            Self(Self::EMPTY)
        } else {
            Self(bits)
        }
    }

    const fn selected(value: bool, bit: u16) -> u16 {
        if value { bit } else { 0 }
    }

    /// The demanded `never` domain: present, but with no possible value kind.
    #[must_use]
    pub const fn empty() -> Self {
        Self(Self::EMPTY)
    }

    /// Whether this is a demanded fact rather than the compact absent sentinel
    /// stored in an entity row.
    #[must_use]
    pub const fn is_present(self) -> bool {
        self.0 != 0
    }

    /// Recover an optional-fact view without making every retained entity row
    /// carry an enum discriminant.
    #[must_use]
    pub const fn present(self) -> Option<Self> {
        if self.is_present() { Some(self) } else { None }
    }

    #[must_use]
    pub const fn may_be_string(self) -> bool {
        self.0 & Self::STRING != 0
    }
    #[must_use]
    pub const fn may_be_number(self) -> bool {
        self.0 & Self::NUMBER != 0
    }
    #[must_use]
    pub const fn may_be_boolean(self) -> bool {
        self.0 & Self::BOOLEAN != 0
    }
    #[must_use]
    pub const fn may_be_big_int(self) -> bool {
        self.0 & Self::BIG_INT != 0
    }
    #[must_use]
    pub const fn may_be_symbol(self) -> bool {
        self.0 & Self::SYMBOL != 0
    }
    #[must_use]
    pub const fn may_be_null(self) -> bool {
        self.0 & Self::NULL != 0
    }
    #[must_use]
    pub const fn may_be_undefined(self) -> bool {
        self.0 & Self::UNDEFINED != 0
    }
    #[must_use]
    pub const fn may_be_object(self) -> bool {
        self.0 & Self::OBJECT != 0
    }
    #[must_use]
    pub const fn numbers_are_finite(self) -> bool {
        self.may_be_number() && self.0 & Self::NUMBERS_FINITE != 0
    }
    #[must_use]
    pub const fn unknown(self) -> bool {
        self.0 == Self::UNKNOWN
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PrimitiveValueDomainSerde {
    #[serde(default, skip_serializing_if = "is_false")]
    may_be_string: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    may_be_number: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    may_be_boolean: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    may_be_big_int: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    may_be_symbol: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    may_be_null: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    may_be_undefined: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    may_be_object: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    numbers_are_finite: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    unknown: bool,
}

impl Serialize for PrimitiveValueDomain {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        PrimitiveValueDomainSerde {
            may_be_string: self.may_be_string(),
            may_be_number: self.may_be_number(),
            may_be_boolean: self.may_be_boolean(),
            may_be_big_int: self.may_be_big_int(),
            may_be_symbol: self.may_be_symbol(),
            may_be_null: self.may_be_null(),
            may_be_undefined: self.may_be_undefined(),
            may_be_object: self.may_be_object(),
            numbers_are_finite: self.numbers_are_finite(),
            unknown: self.unknown(),
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for PrimitiveValueDomain {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = PrimitiveValueDomainSerde::deserialize(deserializer)?;
        Ok(Self::new(
            value.may_be_string,
            value.may_be_number,
            value.may_be_boolean,
            value.may_be_big_int,
            value.may_be_symbol,
            value.may_be_null,
            value.may_be_undefined,
            value.may_be_object,
            value.numbers_are_finite,
            value.unknown,
        ))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ConstantValueKind {
    String,
    Number,
}

/// A compiler-proven primitive value for exactly the demanded expression span.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConstantValue {
    pub kind: ConstantValueKind,
    #[serde(default, skip_serializing_if = "str::is_empty")]
    pub string: Arc<str>,
    #[serde(default, skip_serializing_if = "is_zero_f64")]
    pub number: f64,
}

// The producer rejects NaN, so equality remains reflexive.
impl Eq for ConstantValue {}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PrimitiveLiteralKind {
    String,
    Number,
    Boolean,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PrimitiveLiteralCandidate {
    pub kind: PrimitiveLiteralKind,
    #[serde(default, skip_serializing_if = "str::is_empty")]
    pub string: Arc<str>,
    #[serde(default, skip_serializing_if = "is_zero_f64")]
    pub number: f64,
    #[serde(default, skip_serializing_if = "is_false")]
    pub boolean: bool,
}

impl Eq for PrimitiveLiteralCandidate {}

/// The checker's array/tuple classification for exactly the demanded expression
/// span, derived from its own `isArrayOrTupleType` predicate over the real union
/// constituents. Rendered type text never participates, so an aliased tuple
/// classifies as the tuple it names.
///
/// `Array` is narrower than "array-like": a type merely assignable to
/// `ReadonlyArray<any>` — an interface extending `Array`, or any other
/// purpose-built wrapper — is `NotArray`, because its author chose that type
/// over an array.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ArrayShape {
    /// Every constituent is an array or tuple type.
    Array,
    /// No constituent is an array or tuple type. A positive claim, so a
    /// consumer may rely on the negative.
    NotArray,
    /// A union that genuinely mixes the two. Proven, but proves neither side.
    Mixed,
    /// `any`, `unknown`, `never`, an error type, or an instantiable type the
    /// predicate cannot settle. Not proven in either direction.
    Unknown,
}

impl ArrayShape {
    /// Whether this classification proves an array or tuple. `Mixed` and
    /// `Unknown` prove nothing and answer `false`, so a caller reading only
    /// this cannot mistake an unproven shape for a negative — ask for
    /// [`ArrayShape::NotArray`] explicitly when the negative must be proven.
    #[must_use]
    pub const fn is_array_or_tuple(self) -> bool {
        matches!(self, Self::Array)
    }
}

/// The tuple at exactly the demanded expression span: how many fixed element
/// slots it has, whether a rest or variadic tail follows, and whether the first
/// slot holds a callable value.
///
/// Emitted when the type at that span resolves to a tuple: itself a tuple, or a
/// union whose every value-carrying constituent is one. Never for the global
/// `Array`/`ReadonlyArray` types, which carry a number index signature instead of
/// fixed slots. Absence means "not proven a tuple", never "not a tuple".
///
/// For a union the fields are the constituents' meet — the slots they all have,
/// callable only if all are, and the largest argument requirement among them — so
/// what it reports holds whichever constituent the value turns out to be. Nullish
/// constituents are skipped, so an optional tuple still describes the tuple it is
/// when present; pair it with `runtime_value_domain` if presence also matters.
///
/// This is the structural detail [`ArrayShape`] deliberately collapses. Ask for
/// it when a value has to satisfy an interface with *numbered* members; ask
/// [`ArrayShape`] when the question is only "is this iterable as an array".
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "TupleShapeSerde", into = "TupleShapeSerde")]
pub struct TupleShape {
    // Lower 28 bits: fixed length. Bit 28: rest. Bits 29..31: optional
    // Callability tag. Keeping the four scalar facts in three u32 words makes
    // Option<TupleShape> retain its previous footprint in every EntityFact.
    // The packed value is biased by one so NonZeroU32 gives
    // Option<TupleShape> an outer absence niche for free.
    packed_shape_plus_one: NonZeroU32,
    element_zero_min_parameters: u32,
    // exactLength + 1; zero means absent and preserves exact length zero.
    exact_length_plus_one: u32,
}

impl TupleShape {
    const FIXED_LENGTH_MASK: u32 = (1 << 28) - 1;
    const HAS_REST: u32 = 1 << 28;
    const ELEMENT_ZERO_SHIFT: u32 = 29;

    pub(crate) fn try_new(
        fixed_length: u32,
        has_rest: bool,
        element_zero: Option<Callability>,
        element_zero_min_parameters: u32,
        exact_length: Option<u32>,
    ) -> Result<Self, String> {
        if fixed_length > Self::FIXED_LENGTH_MASK {
            return Err("tuple fixed length exceeds compact representation".into());
        }
        let callability = match element_zero {
            None => 0,
            Some(Callability::Callable) => 1,
            Some(Callability::NonCallable) => 2,
            Some(Callability::Mixed) => 3,
            Some(Callability::Unknown) => 4,
            Some(Callability::UntypedCallable) => 5,
        };
        let exact_length_plus_one = exact_length
            .map(|length| {
                length
                    .checked_add(1)
                    .ok_or_else(|| "tuple exact length overflows compact representation".to_owned())
            })
            .transpose()?
            .unwrap_or_default();
        let rest = if has_rest { Self::HAS_REST } else { 0 };
        let packed_shape = fixed_length | rest | (callability << Self::ELEMENT_ZERO_SHIFT);
        Ok(Self {
            packed_shape_plus_one: NonZeroU32::new(packed_shape + 1)
                .expect("packed tuple shape plus one is nonzero"),
            element_zero_min_parameters,
            exact_length_plus_one,
        })
    }

    /// Initial required-or-optional slots, matching the compiler's own
    /// `fixedLength`.
    #[must_use]
    pub const fn fixed_length(self) -> u32 {
        (self.packed_shape_plus_one.get() - 1) & Self::FIXED_LENGTH_MASK
    }

    /// Whether a rest or variadic tail follows the fixed slots.
    #[must_use]
    pub const fn has_rest(self) -> bool {
        (self.packed_shape_plus_one.get() - 1) & Self::HAS_REST != 0
    }

    /// Callability of the first slot's type.
    #[must_use]
    pub const fn element_zero(self) -> Option<Callability> {
        match (self.packed_shape_plus_one.get() - 1) >> Self::ELEMENT_ZERO_SHIFT {
            0 => None,
            1 => Some(Callability::Callable),
            2 => Some(Callability::NonCallable),
            3 => Some(Callability::Mixed),
            4 => Some(Callability::Unknown),
            5 => Some(Callability::UntypedCallable),
            _ => None,
        }
    }

    /// Fewest arguments required by a call signature in the first slot.
    #[must_use]
    pub const fn element_zero_min_parameters(self) -> u32 {
        self.element_zero_min_parameters
    }

    /// Whether the tuple has a value at index `index` — from a fixed slot, or
    /// from the rest tail when one follows.
    #[must_use]
    pub const fn has_slot(self, index: u32) -> bool {
        index < self.fixed_length() || (self.has_rest() && index <= self.fixed_length())
    }

    /// The exact runtime element count, when compiler tuple structure proves it.
    #[must_use]
    pub const fn exact_length(self) -> Option<u32> {
        self.exact_length_plus_one.checked_sub(1)
    }

    /// Whether the first slot is callable with `arguments` arguments — callable
    /// at all, and not requiring more than that many.
    ///
    /// [`Callability::UntypedCallable`] is deliberately false here: that slot is
    /// callable, but it carries no signature, so no argument count can be
    /// checked against it and `element_zero_min_parameters` is zero for the
    /// absence of a requirement rather than for a proven one. Ask
    /// [`Self::element_zero`] when "is it callable at all" is the question.
    #[must_use]
    pub fn element_zero_accepts(self, arguments: u32) -> bool {
        self.element_zero() == Some(Callability::Callable)
            && self.element_zero_min_parameters <= arguments
    }
}

impl Default for TupleShape {
    fn default() -> Self {
        Self::try_new(0, false, None, 0, None).expect("empty tuple shape is representable")
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct TupleShapeSerde {
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    fixed_length: u32,
    #[serde(default, skip_serializing_if = "is_false")]
    has_rest: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    element_zero: Option<Callability>,
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    element_zero_min_parameters: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    exact_length: Option<u32>,
}

impl TryFrom<TupleShapeSerde> for TupleShape {
    type Error = String;

    fn try_from(value: TupleShapeSerde) -> Result<Self, Self::Error> {
        Self::try_new(
            value.fixed_length,
            value.has_rest,
            value.element_zero,
            value.element_zero_min_parameters,
            value.exact_length,
        )
    }
}

impl From<TupleShape> for TupleShapeSerde {
    fn from(value: TupleShape) -> Self {
        Self {
            fixed_length: value.fixed_length(),
            has_rest: value.has_rest(),
            element_zero: value.element_zero(),
            element_zero_min_parameters: value.element_zero_min_parameters(),
            exact_length: value.exact_length(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ReferenceSpace {
    Value,
    Type,
    Both,
    Neither,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ResolvedCallValidity {
    Valid,
    Recovery,
    Unresolved,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CallKind {
    Unknown,
    Call,
    Construct,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ArgumentMappingStatus {
    Resolved,
    Unresolved,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ArgumentMappingReason {
    CallUnresolved,
    RecoverySignature,
    CompositeSignature,
    SpreadArgument,
    ParameterUnavailable,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DeclarationOwner {
    #[serde(default, skip_serializing_if = "str::is_empty")]
    pub symbol: Arc<str>,
    #[serde(default, skip_serializing_if = "str::is_empty")]
    pub name: Arc<str>,
    pub kind: Arc<str>,
    pub location: Location,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResolvedDeclaration {
    #[serde(default, skip_serializing_if = "str::is_empty")]
    pub symbol: Arc<str>,
    #[serde(default, skip_serializing_if = "str::is_empty")]
    pub name: Arc<str>,
    pub kind: Arc<str>,
    pub location: Location,
    #[serde(default, skip_serializing_if = "is_empty_slice")]
    pub owners: Arc<[DeclarationOwner]>,
    #[serde(default, skip_serializing_if = "str::is_empty")]
    pub qualified_name: Arc<str>,
    #[serde(default, skip_serializing_if = "str::is_empty")]
    pub origin_module: Arc<str>,
    #[serde(default, skip_serializing_if = "str::is_empty")]
    pub source_file: Arc<str>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub standard_library: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ParameterFact {
    pub index: u64,
    #[serde(default, skip_serializing_if = "str::is_empty")]
    pub symbol: Arc<str>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub declaration: Option<Declaration>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub rest: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub optional: bool,
    pub callability: Callability,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub type_descriptor: Option<TypeDescriptor>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object_shape: Option<ObjectConstructionShape>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ConstructionWitness {
    Unknown,
    EmptyArray,
    EmptyObject,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ObjectConstructionProperty {
    pub name: Arc<str>,
    pub witness: ConstructionWitness,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ObjectConstructionShape {
    #[serde(default, skip_serializing_if = "is_empty_slice")]
    pub required_properties: Arc<[ObjectConstructionProperty]>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ArgumentMapping {
    pub argument_index: u64,
    pub status: ArgumentMappingStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unresolved: Option<ArgumentMappingReason>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parameter: Option<ParameterFact>,
}

/// A finite set of exact callable declarations for one composite call.
///
/// `exhaustive` is an explicit compiler proof that `candidates` cover every
/// call signature of the callee's apparent type. A set without that proof
/// must never be treated as the complete runtime dispatch set; the producer
/// only emits proven sets, and consumers must still check the bit.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CallTargetSet {
    #[serde(default, skip_serializing_if = "is_false")]
    pub exhaustive: bool,
    #[serde(default, skip_serializing_if = "is_empty_slice")]
    pub candidates: Arc<[ResolvedDeclaration]>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResolvedCall {
    #[serde(default, skip_serializing_if = "str::is_empty")]
    pub target: Arc<str>,
    #[serde(default, skip_serializing_if = "str::is_empty")]
    pub return_type_text: Arc<str>,
    pub validity: ResolvedCallValidity,
    pub kind: CallKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub declaration: Option<ResolvedDeclaration>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub targets: Option<CallTargetSet>,
    #[serde(default, skip_serializing_if = "is_empty_slice")]
    pub arguments: Arc<[ArgumentMapping]>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TypeDescriptor {
    #[serde(default, skip_serializing_if = "str::is_empty")]
    pub text: Arc<str>,
    #[serde(default, skip_serializing_if = "str::is_empty")]
    pub origin_module: Arc<str>,
    #[serde(default, skip_serializing_if = "is_empty_slice")]
    pub alias_declarations: Arc<[Declaration]>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EntityFact {
    pub location: Location,
    #[serde(default, skip_serializing_if = "str::is_empty")]
    pub symbol: Arc<str>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub symbol_unresolved: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub type_descriptor: Option<Arc<TypeDescriptor>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolved_call: Option<Arc<ResolvedCall>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub callability: Option<Callability>,
    /// Whether the type at this span has construct signatures. Read it beside
    /// `callability`: a class is `NonCallable` and `Constructable`, and only
    /// the two together decide whether an export is a runtime function.
    ///
    /// `None` means the fact was not demanded here, or the span carried no
    /// expression to classify. It is never evidence of a non-constructable
    /// type — and neither is `Some(Constructability::Unknown)`, which is the
    /// checker declining to close a domain.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub constructability: Option<Constructability>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime_value_domain: Option<RuntimeValueDomain>,
    #[serde(default, skip_serializing_if = "primitive_value_domain_is_absent")]
    pub primitive_value_domain: PrimitiveValueDomain,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub primitive_literal_candidates: Option<Arc<Vec<PrimitiveLiteralCandidate>>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub call_result_domain: Option<RuntimeValueDomain>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub constant_value: Option<ConstantValue>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub array_shape: Option<ArrayShape>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tuple_shape: Option<TupleShape>,
    /// Standard-library type names the type at this span is built from at its
    /// top level: itself, its union and intersection constituents, and one
    /// array-element unwrap. Sorted and deduplicated, so it is a set.
    ///
    /// It answers "is this value one of these well-known runtime types" without
    /// depending on how the type was spelled — an alias, an import, and the
    /// built-in written directly all resolve to the same name — and only
    /// declarations in default-library files count, so a user-defined `Map` is
    /// not the global one. Empty means nothing at the top level came from the
    /// standard library; it never means the type is unresolved.
    ///
    /// Held behind a thin `Arc` like the other large evidence on this row: it is
    /// demanded at few spans, and a fat slice pointer would cost every retained
    /// row 16 bytes to carry an absence.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub library_types: Option<Arc<Vec<Arc<str>>>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference_space: Option<ReferenceSpace>,
    #[serde(default, skip_serializing_if = "str::is_empty")]
    pub runtime_identity: Arc<str>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SymbolFact {
    pub id: Arc<str>,
    #[serde(default, skip_serializing_if = "str::is_empty")]
    pub alias_target: Arc<str>,
    #[serde(default, skip_serializing_if = "is_empty_slice")]
    pub declarations: Arc<[Declaration]>,
    #[serde(default, skip_serializing_if = "is_empty_slice")]
    pub references: Arc<[Location]>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SourceCall {
    pub location: Location,
    pub callee: Location,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub arguments: Vec<Location>,
    #[serde(default, skip_serializing_if = "str::is_empty")]
    pub target: Arc<str>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SourceBinding {
    #[serde(default, skip_serializing_if = "is_false")]
    pub array: bool,
    pub names: Vec<Location>,
    pub initializer: SourceCall,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SourceFunction {
    pub name: Location,
    pub body: Location,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parameters: Vec<Location>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub exported: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub r#async: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub arrow: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AsyncFunctionFact {
    pub expression: Location,
    #[serde(default, skip_serializing_if = "str::is_empty")]
    pub symbol: Arc<str>,
    #[serde(default, skip_serializing_if = "str::is_empty")]
    pub target: Arc<str>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub can_return_async: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub calls_after_await: Vec<Location>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FileFact {
    pub path: Arc<str>,
    #[serde(default, skip_serializing_if = "is_empty_slice")]
    pub calls: Arc<[SourceCall]>,
    #[serde(default, skip_serializing_if = "is_empty_slice")]
    pub bindings: Arc<[SourceBinding]>,
    #[serde(default, skip_serializing_if = "is_empty_slice")]
    pub functions: Arc<[SourceFunction]>,
    #[serde(default, skip_serializing_if = "is_empty_slice")]
    pub async_functions: Arc<[AsyncFunctionFact]>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SourceDigest {
    pub path: Arc<str>,
    pub sha256: SourceHash,
}

#[derive(Debug, Error)]
pub enum TypeFactsError {
    #[error("message is {actual} bytes, limit is {limit}")]
    MessageLimit { actual: usize, limit: usize },
    #[error("CBOR codec error: {0}")]
    Codec(String),
    #[error("invalid deterministic CBOR: {0}")]
    DeterministicCbor(String),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("source hash is not canonical sha256: {0:?}")]
    SourceHash(String),
}

pub fn encode<T: Serialize>(value: &T) -> Result<Vec<u8>, TypeFactsError> {
    let mut intermediate = Vec::new();
    ciborium::into_writer(value, &mut intermediate)
        .map_err(|error| TypeFactsError::Codec(error.to_string()))?;
    let mut value: ciborium::Value = ciborium::from_reader(intermediate.as_slice())
        .map_err(|error| TypeFactsError::Codec(error.to_string()))?;
    canonicalize(&mut value)?;
    let mut encoded = Vec::new();
    ciborium::into_writer(&value, &mut encoded)
        .map_err(|error| TypeFactsError::Codec(error.to_string()))?;
    enforce_limit(encoded.len())?;
    Ok(encoded)
}

/// Encodes a request for the already authenticated local v3 sidecar.
///
/// The v3 request fields are declared in deterministic CBOR key order, so
/// serializing the struct directly preserves the wire contract without the
/// generic value round trip used by [`encode`].
pub fn encode_sidecar_request(value: &v3::Request) -> Result<Vec<u8>, TypeFactsError> {
    let mut encoded = Vec::new();
    ciborium::into_writer(value, &mut encoded)
        .map_err(|error| TypeFactsError::Codec(error.to_string()))?;
    enforce_limit(encoded.len())?;
    Ok(encoded)
}

pub fn decode<T: DeserializeOwned>(encoded: &[u8]) -> Result<T, TypeFactsError> {
    enforce_limit(encoded.len())?;
    validate_deterministic_cbor(encoded)?;
    ciborium::from_reader(encoded).map_err(|error| TypeFactsError::Codec(error.to_string()))
}

/// Decodes a frame from the already authenticated local v3 sidecar.
///
/// Frozen protocol fixtures and untrusted inputs must continue to use
/// [`decode`], which verifies deterministic CBOR before deserializing.
pub fn decode_trusted<T: DeserializeOwned>(encoded: &[u8]) -> Result<T, TypeFactsError> {
    enforce_limit(encoded.len())?;
    ciborium::from_reader(encoded).map_err(|error| TypeFactsError::Codec(error.to_string()))
}

/// Write one length-prefixed payload using the TypeFacts u32-LE frame codec.
pub fn write_frame(writer: &mut impl Write, payload: &[u8]) -> Result<(), TypeFactsError> {
    enforce_limit(payload.len())?;
    let length = u32::try_from(payload.len()).map_err(|_| TypeFactsError::MessageLimit {
        actual: payload.len(),
        limit: u32::MAX as usize,
    })?;
    writer.write_all(&length.to_le_bytes())?;
    writer.write_all(payload)?;
    writer.flush()?;
    Ok(())
}

/// Read one length-prefixed payload using the TypeFacts u32-LE frame codec.
pub fn read_frame(reader: &mut impl Read) -> Result<Vec<u8>, TypeFactsError> {
    let mut prefix = [0_u8; 4];
    reader.read_exact(&mut prefix)?;
    let length = u32::from_le_bytes(prefix) as usize;
    enforce_limit(length)?;
    let mut payload = vec![0_u8; length];
    reader.read_exact(&mut payload)?;
    Ok(payload)
}

fn canonicalize(value: &mut ciborium::Value) -> Result<(), TypeFactsError> {
    match value {
        ciborium::Value::Array(values) => {
            for value in values {
                canonicalize(value)?;
            }
        }
        ciborium::Value::Map(entries) => {
            for (key, value) in entries.iter_mut() {
                canonicalize(key)?;
                canonicalize(value)?;
            }
            let mut keyed = entries
                .drain(..)
                .map(|entry| {
                    let mut encoded_key = Vec::new();
                    ciborium::into_writer(&entry.0, &mut encoded_key)
                        .map_err(|error| TypeFactsError::Codec(error.to_string()))?;
                    Ok((encoded_key, entry))
                })
                .collect::<Result<Vec<_>, TypeFactsError>>()?;
            keyed.sort_by(|left, right| {
                left.0
                    .len()
                    .cmp(&right.0.len())
                    .then_with(|| left.0.cmp(&right.0))
            });
            entries.extend(keyed.into_iter().map(|(_, entry)| entry));
        }
        ciborium::Value::Tag(_, value) => canonicalize(value)?,
        _ => {}
    }
    Ok(())
}

fn enforce_limit(length: usize) -> Result<(), TypeFactsError> {
    if length > MAX_MESSAGE_BYTES {
        return Err(TypeFactsError::MessageLimit {
            actual: length,
            limit: MAX_MESSAGE_BYTES,
        });
    }
    Ok(())
}

fn validate_deterministic_cbor(encoded: &[u8]) -> Result<(), TypeFactsError> {
    let end = validate_cbor_item(encoded, 0, 1)?;
    if end != encoded.len() {
        return Err(TypeFactsError::DeterministicCbor(
            "trailing bytes after top-level item".into(),
        ));
    }
    Ok(())
}

fn validate_cbor_item(encoded: &[u8], start: usize, depth: usize) -> Result<usize, TypeFactsError> {
    if depth > MAX_NESTING_DEPTH {
        return Err(TypeFactsError::DeterministicCbor(format!(
            "nesting depth exceeds {MAX_NESTING_DEPTH}"
        )));
    }
    let initial = *encoded
        .get(start)
        .ok_or_else(|| TypeFactsError::DeterministicCbor("truncated item".into()))?;
    let major = initial >> 5;
    let additional = initial & 0x1f;
    let (argument, mut cursor) = decode_cbor_argument(encoded, start + 1, additional)?;
    match major {
        0 | 1 => Ok(cursor),
        2 | 3 => {
            let length = usize::try_from(argument).map_err(|_| {
                TypeFactsError::DeterministicCbor("string length overflows usize".into())
            })?;
            let end = cursor.checked_add(length).ok_or_else(|| {
                TypeFactsError::DeterministicCbor("string length overflow".into())
            })?;
            let bytes = encoded
                .get(cursor..end)
                .ok_or_else(|| TypeFactsError::DeterministicCbor("truncated string".into()))?;
            if major == 3 {
                std::str::from_utf8(bytes).map_err(|error| {
                    TypeFactsError::DeterministicCbor(format!(
                        "text string at byte {cursor} (length {length}) is not UTF-8: {error}"
                    ))
                })?;
            }
            Ok(end)
        }
        4 => {
            let length = collection_length(argument)?;
            for _ in 0..length {
                cursor = validate_cbor_item(encoded, cursor, depth + 1)?;
            }
            Ok(cursor)
        }
        5 => {
            let length = collection_length(argument)?;
            let mut previous_key: Option<&[u8]> = None;
            for _ in 0..length {
                let key_start = cursor;
                cursor = validate_cbor_item(encoded, cursor, depth + 1)?;
                let key = &encoded[key_start..cursor];
                if let Some(previous) = previous_key {
                    let ordering = previous
                        .len()
                        .cmp(&key.len())
                        .then_with(|| previous.cmp(key));
                    if !ordering.is_lt() {
                        return Err(TypeFactsError::DeterministicCbor(
                            if previous == key {
                                "duplicate map key"
                            } else {
                                "map keys are not in core deterministic order"
                            }
                            .into(),
                        ));
                    }
                }
                previous_key = Some(key);
                cursor = validate_cbor_item(encoded, cursor, depth + 1)?;
            }
            Ok(cursor)
        }
        6 => Err(TypeFactsError::DeterministicCbor(
            "CBOR tags are forbidden".into(),
        )),
        7 if matches!(additional, 20 | 21) => Ok(cursor),
        7 => Err(TypeFactsError::DeterministicCbor(
            "only boolean simple values are permitted".into(),
        )),
        _ => Err(TypeFactsError::DeterministicCbor(format!(
            "unsupported CBOR major type {major}"
        ))),
    }
}

fn decode_cbor_argument(
    encoded: &[u8],
    cursor: usize,
    additional: u8,
) -> Result<(u64, usize), TypeFactsError> {
    let (argument, width) = match additional {
        value @ 0..=23 => (u64::from(value), 0),
        24 => (
            u64::from(*encoded.get(cursor).ok_or_else(|| {
                TypeFactsError::DeterministicCbor("truncated uint8 argument".into())
            })?),
            1,
        ),
        25 => (
            u64::from(u16::from_be_bytes(read_cbor_bytes(encoded, cursor)?)),
            2,
        ),
        26 => (
            u64::from(u32::from_be_bytes(read_cbor_bytes(encoded, cursor)?)),
            4,
        ),
        27 => (u64::from_be_bytes(read_cbor_bytes(encoded, cursor)?), 8),
        31 => {
            return Err(TypeFactsError::DeterministicCbor(
                "indefinite-length items are forbidden".into(),
            ));
        }
        value => {
            return Err(TypeFactsError::DeterministicCbor(format!(
                "reserved additional information {value}"
            )));
        }
    };
    let shortest = match width {
        0 => true,
        1 => argument >= 24,
        2 => argument > u64::from(u8::MAX),
        4 => argument > u64::from(u16::MAX),
        8 => argument > u64::from(u32::MAX),
        _ => unreachable!(),
    };
    if !shortest {
        return Err(TypeFactsError::DeterministicCbor(
            "integer or length is not shortest-form encoded".into(),
        ));
    }
    Ok((argument, cursor + width))
}

fn read_cbor_bytes<const N: usize>(
    encoded: &[u8],
    cursor: usize,
) -> Result<[u8; N], TypeFactsError> {
    encoded
        .get(cursor..cursor + N)
        .ok_or_else(|| TypeFactsError::DeterministicCbor("truncated argument".into()))?
        .try_into()
        .map_err(|_| TypeFactsError::DeterministicCbor("invalid argument width".into()))
}

fn collection_length(argument: u64) -> Result<usize, TypeFactsError> {
    let length = usize::try_from(argument).map_err(|_| {
        TypeFactsError::DeterministicCbor("collection length overflows usize".into())
    })?;
    if length > MAX_COLLECTION_LENGTH {
        return Err(TypeFactsError::DeterministicCbor(format!(
            "collection length {length} exceeds {MAX_COLLECTION_LENGTH}"
        )));
    }
    Ok(length)
}

/// The compiler's emit module format for one included file, as
/// `GetEmitModuleFormatOfFile` computes it: the file's implied node format
/// where the configured module kind defers to it, and the configured kind
/// otherwise.
///
/// Only formats that describe a real runtime shape have a variant. The legacy
/// AMD, UMD, and System kinds, and a program with no module emit at all,
/// answer [`ModuleFormat::Unknown`] — a refusal to characterize the file, not a
/// claim about it.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ModuleFormat {
    Commonjs,
    Esm,
    /// `module: preserve`: import and export syntax is emitted as written.
    Preserve,
    /// The field was absent, which is how the producer reports a format this
    /// vocabulary does not name. Producer and client ship in build-id
    /// lockstep, so a *present* value outside this set is a protocol violation
    /// and is rejected at decode rather than folded in here.
    #[default]
    Unknown,
}

/// The compiler's own pairing of one input file with the declaration file
/// emitted from it.
///
/// It exists only where a configured `references` entry covers the file, and it
/// is the **only** declaration-to-implementation pairing TypeScript maintains.
/// In particular it is never available for the shape almost every published
/// package has — a shipped `channel.d.ts` beside a `channel.js` — because
/// resolution selects the declaration file, never opens the implementation, and
/// records nothing joining the two. A consumer that needs that edge does not
/// have it, and must not reconstruct it by matching file names.
///
/// Both fields are always populated; which one equals the module's own path
/// says whether the program holds the input or the output.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectReferenceMapping {
    pub source: Arc<str>,
    pub output_dts: Arc<str>,
}

/// One file the TypeScript program actually resolved and included.
///
/// The complete list of these is the program's own file list, so a consumer
/// recording which bytes an analysis read holds an attestation rather than a
/// reconstruction of one.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModuleFact {
    /// The cleaned absolute path the program holds the file under. For a module
    /// reached through a symlink this is the realpath, matching
    /// [`ModuleImportFact::resolved_path`].
    pub path: Arc<str>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub declaration_file: bool,
    #[serde(default, skip_serializing_if = "module_format_is_unknown")]
    pub format: ModuleFormat,
    /// The compiler's input-to-declaration-output pairing, and `None` whenever
    /// no configured project reference covers this file — which is almost
    /// always. See [`ProjectReferenceMapping`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_reference: Option<ProjectReferenceMapping>,
    /// Other paths the program resolved to this same file because they are the
    /// same `name@version` installed in more than one place. The compiler's own
    /// duplicate-install record, never a path similarity.
    #[serde(default, skip_serializing_if = "is_empty_slice")]
    pub redirect_targets: Arc<[Arc<str>]>,
}

/// What the compiler's resolver recorded about the shape of one resolution.
/// Every variant is read off `module.ResolvedModule`; none is inferred from a
/// path.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ModuleResolution {
    /// The program holds no resolution for this specifier. The only variant
    /// with an empty [`ModuleImportFact::resolved_path`].
    #[default]
    Unresolved,
    /// A specifier the resolver treated as relative or rooted, so no package
    /// lookup participated.
    Relative,
    /// `IsExternalLibraryImport`: the resolver landed inside a `node_modules`
    /// tree.
    NodeModules,
    /// A bare specifier that resolved outside every `node_modules` tree. A
    /// tsconfig `paths` or `baseUrl` mapping, a package self-name, a
    /// project-reference redirect, and an ambient module declaration all land
    /// here, and `ResolvedModule` does not record which, so this variant never
    /// claims one. [`ModuleImportFact::paths_pattern`] answers the `paths` half
    /// on its own terms.
    NonRelative,
}

/// The owning package of a resolved file: the nearest enclosing `package.json`
/// found by the compiler's own package-scope lookup, and that manifest's own
/// name and version.
///
/// An empty `name` or `version` is a fact about the manifest, not a lookup
/// failure — `manifest_path` is populated in that case too. The package
/// directory is the manifest path's parent and is not repeated here.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PackageIdentity {
    pub manifest_path: Arc<str>,
    #[serde(default, skip_serializing_if = "str::is_empty")]
    pub name: Arc<str>,
    #[serde(default, skip_serializing_if = "str::is_empty")]
    pub version: Arc<str>,
}

/// The package identity the *resolver* itself recorded while resolving one
/// specifier.
///
/// This is a different fact from [`PackageIdentity`] and the two can disagree:
/// this one names the package whose manifest the resolution consulted, which
/// for a subpath export or a nested workspace install is not always the nearest
/// manifest above the file that was selected. A consumer comparing a contract
/// against a package must say which of the two it means.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResolverPackageId {
    #[serde(default, skip_serializing_if = "str::is_empty")]
    pub name: Arc<str>,
    /// The selected file's path relative to the package directory, as the
    /// resolver recorded it — the file it landed on, not the `exports` key that
    /// led there. Empty when the package root's own entry was selected.
    #[serde(default, skip_serializing_if = "str::is_empty")]
    pub subpath: Arc<str>,
    #[serde(default, skip_serializing_if = "str::is_empty")]
    pub version: Arc<str>,
    #[serde(default, skip_serializing_if = "str::is_empty")]
    pub peer_dependencies: Arc<str>,
}

/// The compiler's own answer for one module specifier: the file the program
/// included for it, and what the resolver recorded on the way.
///
/// One fact is produced per specifier occurrence in the file's import list —
/// import declarations, export-from declarations, `import(...)` types, and
/// require calls alike — so a consumer joins these rows to its own syntax facts
/// by exact span rather than by matching specifier text.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModuleImportFact {
    /// The string-literal span, with `path` naming the importing file.
    pub specifier: Location,
    /// The specifier as written, after string-literal unescaping.
    pub text: Arc<str>,
    pub resolution: ModuleResolution,
    /// The file the resolver selected. When resolution walked a symlink this is
    /// the realpath. Empty exactly when `resolution` is
    /// [`ModuleResolution::Unresolved`].
    #[serde(default, skip_serializing_if = "str::is_empty")]
    pub resolved_path: Arc<str>,
    /// The file the program actually parses in place of `resolved_path`,
    /// populated only when the two differ.
    ///
    /// This is the compiler's own redirect record, and the only mechanism by
    /// which a specifier that resolved to a declaration file is joined to an
    /// implementation: a configured project reference's declaration output is
    /// redirected to the input it was emitted from, and a symlinked equivalent
    /// of the same. Nothing redirects an ordinary shipped `.d.ts` to the `.js`
    /// beside it, so an empty value is the usual and honest answer.
    #[serde(default, skip_serializing_if = "str::is_empty")]
    pub included_path: Arc<str>,
    /// The path the resolver had reached before taking its realpath.
    ///
    /// TypeScript populates it only when the two differ and only for a
    /// non-relative resolution under `node_modules` with `preserveSymlinks`
    /// off — exactly the pnpm and workspace-link shape — so an empty value
    /// means the resolver saw no divergence, not that none was looked for.
    #[serde(default, skip_serializing_if = "str::is_empty")]
    pub symlink_path: Arc<str>,
    /// The extension the resolver selected (`.ts`, `.d.ts`, `.js`, `.json`, …).
    /// How a consumer sees that a specifier landed on a declaration file.
    #[serde(default, skip_serializing_if = "str::is_empty")]
    pub extension: Arc<str>,
    /// The specifier named a TypeScript extension outright rather than having
    /// one substituted.
    #[serde(default, skip_serializing_if = "is_false")]
    pub ts_extension: bool,
    /// The configured `paths` key the compiler's own pattern matcher selects
    /// for `text`, under the compiler's eligibility rule (`paths` is non-empty
    /// and the specifier is not relative) and its longest-prefix tie-break.
    ///
    /// It says the mapping *matched the specifier*, which is a fact about the
    /// configuration and the text. It does not say the resolution came through
    /// the mapping: TypeScript tries `paths` first and falls through to
    /// ordinary resolution when the mapped candidate does not exist, and
    /// `ResolvedModule` records no trace of which happened. Read with
    /// `resolution` it is nonetheless decisive for the case it serves — a bare
    /// specifier that a `paths` key matched and that did *not* land in
    /// `node_modules` is not the installed package of that name.
    #[serde(default, skip_serializing_if = "str::is_empty")]
    pub paths_pattern: Arc<str>,
    /// The owning package of `resolved_path`, present only when the request
    /// asked for package identities.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub package: Option<PackageIdentity>,
    /// The identity the resolver itself recorded, present only when the request
    /// asked for package identities and the resolver read a manifest during
    /// this resolution.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolver_package: Option<ResolverPackageId>,
}

/// Selects how much of the resolved module graph one request answers. The
/// module inventory itself is unconditional: it is the operation's reason to
/// exist.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ModuleGraphDemand {
    /// Ask for resolved import provenance. With no `import_paths` this covers
    /// every file the program included.
    pub imports: bool,
    /// Scope `imports` to these importing files.
    pub import_paths: Vec<String>,
    /// Add `package` and `resolver_package` to every import fact.
    pub packages: bool,
}

impl ModuleGraphDemand {
    /// The inventory alone: every file the program included, and no import rows.
    #[must_use]
    pub fn inventory() -> Self {
        Self::default()
    }

    /// The inventory plus every file's resolved import provenance.
    #[must_use]
    pub fn with_all_imports() -> Self {
        Self {
            imports: true,
            ..Self::default()
        }
    }

    /// Scope import provenance to these importing files. Setting it implies
    /// [`Self::imports`].
    #[must_use]
    pub fn import_paths<I>(mut self, paths: I) -> Self
    where
        I: IntoIterator,
        I::Item: Into<String>,
    {
        self.imports = true;
        self.import_paths = paths.into_iter().map(Into::into).collect();
        self
    }

    /// Also answer package identities on every import fact.
    #[must_use]
    pub const fn with_packages(mut self) -> Self {
        self.packages = true;
        self
    }
}

/// One generation's resolved module graph, as the compiler holds it.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ModuleGraph {
    /// Every file the program included, ordered by path.
    pub modules: Vec<ModuleFact>,
    /// The requested files' specifier facts, ordered by importing path and then
    /// by specifier start byte.
    pub imports: Vec<ModuleImportFact>,
    /// Requested import paths the program does not hold, ordered by path.
    ///
    /// They are reported rather than dropped, so a consumer can tell "this file
    /// imports nothing" from "this file was never analyzed". A non-empty value
    /// means the answer is scoped to less than what was asked for.
    pub unknown_import_paths: Vec<Arc<str>>,
}

impl ModuleGraph {
    /// The module fact for an exact path, if the program included it.
    #[must_use]
    pub fn module(&self, path: &str) -> Option<&ModuleFact> {
        self.modules
            .binary_search_by(|module| (*module.path).cmp(path))
            .ok()
            .map(|index| &self.modules[index])
    }

    /// Every import fact declared by one importing file, in source order.
    pub fn imports_from<'a>(
        &'a self,
        path: &'a str,
    ) -> impl Iterator<Item = &'a ModuleImportFact> + 'a {
        self.imports
            .iter()
            .filter(move |fact| &*fact.specifier.path == path)
    }

    /// Whether every requested import path was answered. A `false` here is the
    /// signal to fail closed: the graph describes fewer files than were asked
    /// about.
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.unknown_import_paths.is_empty()
    }
}

const fn module_format_is_unknown(value: &ModuleFormat) -> bool {
    matches!(value, ModuleFormat::Unknown)
}

const fn is_false(value: &bool) -> bool {
    !*value
}

const fn primitive_value_domain_is_absent(value: &PrimitiveValueDomain) -> bool {
    !value.is_present()
}

fn is_zero_f64(value: &f64) -> bool {
    *value == 0.0
}

fn is_zero_u32(value: &u32) -> bool {
    *value == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    // Every retained row pays for every inline optional field, so the budget
    // rises only after the field has been made as small as it can be. `TupleShape`
    // cost 16 bytes with a `usize` length and 8 with a `u32` one — a tuple cannot
    // have more slots than the source has bytes — and the `Option` rides the
    // callability enum's niche. Boxing it would buy the other 8 bytes back at the
    // price of an allocation per fact; it is three scalars, so it stays inline.
    //
    // `library_types` is the opposite call and shows the rule working: as an
    // `Arc<[Arc<str>]>` it cost 16 bytes on every row to carry an absence, because
    // a slice pointer is fat. Behind a thin `Arc`, like `resolved_call` and
    // `type_descriptor` before it, it costs 8. Primitive literal candidates
    // use the same thin-Arc representation; the one new pointer raises the
    // deliberate retained-row ceiling by exactly 8 bytes.
    #[test]
    fn retained_entity_rows_keep_optional_evidence_bounded() {
        assert!(
            std::mem::size_of::<EntityFact>() <= 152,
            "EntityFact is {} bytes; optional evidence exceeded the bounded row budget",
            std::mem::size_of::<EntityFact>()
        );
    }

    #[test]
    fn sidecar_request_fast_path_preserves_canonical_cbor() {
        let location = Location {
            path: "a.ts".into(),
            start_byte: 1,
            end_byte: 2,
        };
        let request = v3::Request {
            schema: v3::TYPE_FACTS_SCHEMA_V1,
            request_id: 7,
            operation: v3::Operation::Analyze,
            project_id: "project".into(),
            generation: 3,
            changes: vec![v3::FileChange {
                path: "a.ts".into(),
                version: 3,
                source: b"let a = 1".to_vec(),
                deleted: false,
            }],
            demands: vec![v3::EntityDemand {
                location,
                query_location: None,
                symbol: true,
                type_descriptor: true,
                resolved_call: false,
                references: true,
                r#async: false,
                structural_accessor: false,
                callability: false,
                constructability: false,
                runtime_value_domain: false,
                primitive_value_domain: false,
                primitive_literal_candidates: false,
                parameter_object_shape: false,
                call_result_domain: false,
                constant_value: false,
                array_shape: false,
                tuple_shape: false,
                library_types: false,
                reference_space: false,
                runtime_identity: false,
            }],
            compact_demands: Some(v3::CompactDemands {
                groups: vec![v3::CompactDemandGroup(1, vec![7, 1, 1, 1, 1, 1])],
                strings: vec![String::new(), "a.ts".into()],
            }),
            state_token: "9".into(),
            reset_state: false,
            removed_demand_paths: vec!["old.ts".into()],
            symbol_queries: Vec::new(),
            release_analysis: false,
            reference_changes: false,
            reference_paths: Vec::new(),
            cancel_request_id: 2,
            module_graph: None,
        };
        assert_eq!(
            encode_sidecar_request(&request).unwrap(),
            encode(&request).unwrap()
        );
    }

    #[test]
    fn compact_demands_round_trip() {
        let location = |path: &str, start: u64, end: u64| Location {
            path: path.into(),
            start_byte: start,
            end_byte: end,
        };
        let demands = vec![
            v3::EntityDemand {
                location: location("a.ts", 1, 4),
                query_location: None,
                symbol: true,
                type_descriptor: false,
                resolved_call: false,
                references: true,
                r#async: false,
                structural_accessor: false,
                callability: false,
                constructability: false,
                runtime_value_domain: false,
                primitive_value_domain: false,
                primitive_literal_candidates: false,
                parameter_object_shape: false,
                call_result_domain: false,
                constant_value: false,
                array_shape: false,
                tuple_shape: false,
                library_types: false,
                reference_space: false,
                runtime_identity: false,
            },
            v3::EntityDemand {
                location: location("a.ts", 5, 9),
                query_location: Some(location("a.ts", 6, 8)),
                symbol: true,
                type_descriptor: true,
                resolved_call: true,
                references: false,
                r#async: true,
                structural_accessor: true,
                callability: true,
                constructability: true,
                runtime_value_domain: true,
                primitive_value_domain: true,
                primitive_literal_candidates: true,
                parameter_object_shape: false,
                call_result_domain: true,
                constant_value: true,
                array_shape: true,
                tuple_shape: true,
                library_types: true,
                reference_space: true,
                runtime_identity: true,
            },
            v3::EntityDemand {
                location: location("b.ts", 2, 8),
                query_location: None,
                symbol: false,
                type_descriptor: false,
                resolved_call: false,
                references: false,
                r#async: true,
                structural_accessor: false,
                callability: false,
                constructability: false,
                runtime_value_domain: false,
                primitive_value_domain: false,
                primitive_literal_candidates: false,
                parameter_object_shape: false,
                call_result_domain: false,
                constant_value: false,
                array_shape: false,
                tuple_shape: false,
                library_types: false,
                reference_space: false,
                runtime_identity: false,
            },
        ];
        let compact = v3::compact_demands(&demands);
        let decoded: v3::CompactDemands = decode(&encode(&compact).unwrap()).unwrap();
        assert_eq!(decoded, compact);
        assert_eq!(decoded.groups.len(), 2);
        assert_eq!(decoded.strings[0], "");
    }

    #[test]
    fn frame_codec_round_trips_and_rejects_oversized_prefixes() {
        let mut framed = Vec::new();
        write_frame(&mut framed, b"payload").unwrap();
        assert_eq!(read_frame(&mut framed.as_slice()).unwrap(), b"payload");

        let oversized = u32::try_from(MAX_MESSAGE_BYTES + 1).unwrap().to_le_bytes();
        assert!(matches!(
            read_frame(&mut oversized.as_slice()),
            Err(TypeFactsError::MessageLimit { .. })
        ));
    }

    #[test]
    fn rejects_non_deterministic_and_unsafe_cbor_before_typed_decode() {
        for (label, encoded) in [
            ("overlong integer", vec![0x18, 0x01]),
            ("indefinite array", vec![0x9f, 0xff]),
            (
                "duplicate map key",
                vec![0xa2, 0x61, b'a', 0x01, 0x61, b'a', 0x02],
            ),
            (
                "non-canonical map order",
                vec![0xa2, 0x62, b'a', b'a', 0x01, 0x61, b'b', 0x02],
            ),
            ("tag", vec![0xc0, 0x01]),
            ("null", vec![0xf6]),
        ] {
            assert!(
                matches!(
                    decode::<ciborium::Value>(&encoded),
                    Err(TypeFactsError::DeterministicCbor(_))
                ),
                "{label} was accepted"
            );
        }

        let mut too_deep = vec![0x81; MAX_NESTING_DEPTH];
        too_deep.push(0x01);
        assert!(matches!(
            decode::<ciborium::Value>(&too_deep),
            Err(TypeFactsError::DeterministicCbor(_))
        ));
    }
}
