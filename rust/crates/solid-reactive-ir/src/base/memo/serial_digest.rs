//! A value's serde serialization as tagged binary, for a digest: every
//! field the value's `Serialize` visits, with the name of each struct field
//! (a skipped field is named as skipped), each variant's index, and each
//! string's and sequence's length. The encoding is prefix-free, so two
//! values that serialize differently digest differently, as their JSON did,
//! without formatting numbers or escaping strings.

use serde::ser::{self, Serialize};

/// Appends `value`'s serialization to `bytes`, which the caller digests
/// (one digest update per many values: an update per field costs more than
/// the bytes).
pub(super) fn append<T: Serialize + ?Sized>(bytes: &mut Vec<u8>, value: &T) {
    // Serializing into bytes cannot fail; a `Serialize` that reports an
    // error stops early, as the JSON writer this replaces did.
    let _ = value.serialize(&mut Writer(bytes));
}

struct Writer<'a>(&'a mut Vec<u8>);

#[derive(Debug)]
pub(super) struct Error;

impl std::fmt::Display for Error {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("a value failed to serialize")
    }
}

impl std::error::Error for Error {}

impl ser::Error for Error {
    fn custom<T: std::fmt::Display>(_: T) -> Self {
        Self
    }
}

mod tag {
    pub(super) const BOOL: u8 = 1;
    pub(super) const INT: u8 = 2;
    pub(super) const FLOAT: u8 = 3;
    pub(super) const CHAR: u8 = 4;
    pub(super) const STR: u8 = 5;
    pub(super) const BYTES: u8 = 6;
    pub(super) const NONE: u8 = 7;
    pub(super) const SOME: u8 = 8;
    pub(super) const UNIT: u8 = 9;
    pub(super) const UNIT_VARIANT: u8 = 10;
    pub(super) const NEWTYPE: u8 = 11;
    pub(super) const NEWTYPE_VARIANT: u8 = 12;
    pub(super) const SEQ: u8 = 13;
    pub(super) const TUPLE: u8 = 14;
    pub(super) const TUPLE_VARIANT: u8 = 15;
    pub(super) const MAP: u8 = 16;
    pub(super) const STRUCT: u8 = 17;
    pub(super) const STRUCT_VARIANT: u8 = 18;
    pub(super) const ELEMENT: u8 = 19;
    pub(super) const KEY: u8 = 20;
    pub(super) const VALUE: u8 = 21;
    pub(super) const FIELD: u8 = 22;
    pub(super) const SKIPPED: u8 = 23;
    pub(super) const END: u8 = 24;
    /// An unsigned integer past `i128::MAX`.
    pub(super) const WIDE: u8 = 25;
}

impl Writer<'_> {
    fn tag(&mut self, tag: u8) {
        self.0.push(tag);
    }

    fn len(&mut self, len: usize) {
        self.0
            .extend_from_slice(&u64::try_from(len).unwrap_or(u64::MAX).to_le_bytes());
    }

    fn text(&mut self, text: &str) {
        self.len(text.len());
        self.0.extend_from_slice(text.as_bytes());
    }

    fn variant(&mut self, tag: u8, index: u32) {
        self.tag(tag);
        self.0.extend_from_slice(&index.to_le_bytes());
    }
}

impl<'w> ser::Serializer for &mut Writer<'w> {
    type Ok = ();
    type Error = Error;
    type SerializeSeq = Self;
    type SerializeTuple = Self;
    type SerializeTupleStruct = Self;
    type SerializeTupleVariant = Self;
    type SerializeMap = Self;
    type SerializeStruct = Self;
    type SerializeStructVariant = Self;

    fn serialize_bool(self, value: bool) -> Result<(), Error> {
        self.tag(tag::BOOL);
        self.0.push(u8::from(value));
        Ok(())
    }

    fn serialize_i8(self, value: i8) -> Result<(), Error> {
        self.serialize_i128(i128::from(value))
    }

    fn serialize_i16(self, value: i16) -> Result<(), Error> {
        self.serialize_i128(i128::from(value))
    }

    fn serialize_i32(self, value: i32) -> Result<(), Error> {
        self.serialize_i128(i128::from(value))
    }

    fn serialize_i64(self, value: i64) -> Result<(), Error> {
        self.serialize_i128(i128::from(value))
    }

    fn serialize_i128(self, value: i128) -> Result<(), Error> {
        // Every integer as one width, as JSON prints them alike.
        self.tag(tag::INT);
        self.0.extend_from_slice(&value.to_le_bytes());
        Ok(())
    }

    fn serialize_u8(self, value: u8) -> Result<(), Error> {
        self.serialize_i128(i128::from(value))
    }

    fn serialize_u16(self, value: u16) -> Result<(), Error> {
        self.serialize_i128(i128::from(value))
    }

    fn serialize_u32(self, value: u32) -> Result<(), Error> {
        self.serialize_i128(i128::from(value))
    }

    fn serialize_u64(self, value: u64) -> Result<(), Error> {
        self.serialize_i128(i128::from(value))
    }

    fn serialize_u128(self, value: u128) -> Result<(), Error> {
        match i128::try_from(value) {
            Ok(value) => self.serialize_i128(value),
            Err(_) => {
                self.tag(tag::WIDE);
                self.0.extend_from_slice(&value.to_le_bytes());
                Ok(())
            }
        }
    }

    fn serialize_f32(self, value: f32) -> Result<(), Error> {
        self.serialize_f64(f64::from(value))
    }

    fn serialize_f64(self, value: f64) -> Result<(), Error> {
        self.tag(tag::FLOAT);
        self.0.extend_from_slice(&value.to_bits().to_le_bytes());
        Ok(())
    }

    fn serialize_char(self, value: char) -> Result<(), Error> {
        self.tag(tag::CHAR);
        self.0.extend_from_slice(&u32::from(value).to_le_bytes());
        Ok(())
    }

    fn serialize_str(self, value: &str) -> Result<(), Error> {
        self.tag(tag::STR);
        self.text(value);
        Ok(())
    }

    fn serialize_bytes(self, value: &[u8]) -> Result<(), Error> {
        self.tag(tag::BYTES);
        self.len(value.len());
        self.0.extend_from_slice(value);
        Ok(())
    }

    fn serialize_none(self) -> Result<(), Error> {
        self.tag(tag::NONE);
        Ok(())
    }

    fn serialize_some<T: Serialize + ?Sized>(self, value: &T) -> Result<(), Error> {
        self.tag(tag::SOME);
        value.serialize(self)
    }

    fn serialize_unit(self) -> Result<(), Error> {
        self.tag(tag::UNIT);
        Ok(())
    }

    fn serialize_unit_struct(self, _: &'static str) -> Result<(), Error> {
        self.serialize_unit()
    }

    fn serialize_unit_variant(
        self,
        _: &'static str,
        index: u32,
        _: &'static str,
    ) -> Result<(), Error> {
        self.variant(tag::UNIT_VARIANT, index);
        Ok(())
    }

    fn serialize_newtype_struct<T: Serialize + ?Sized>(
        self,
        _: &'static str,
        value: &T,
    ) -> Result<(), Error> {
        self.tag(tag::NEWTYPE);
        value.serialize(self)
    }

    fn serialize_newtype_variant<T: Serialize + ?Sized>(
        self,
        _: &'static str,
        index: u32,
        _: &'static str,
        value: &T,
    ) -> Result<(), Error> {
        self.variant(tag::NEWTYPE_VARIANT, index);
        value.serialize(self)
    }

    fn serialize_seq(self, _: Option<usize>) -> Result<Self, Error> {
        self.tag(tag::SEQ);
        Ok(self)
    }

    fn serialize_tuple(self, _: usize) -> Result<Self, Error> {
        self.tag(tag::TUPLE);
        Ok(self)
    }

    fn serialize_tuple_struct(self, _: &'static str, _: usize) -> Result<Self, Error> {
        self.tag(tag::TUPLE);
        Ok(self)
    }

    fn serialize_tuple_variant(
        self,
        _: &'static str,
        index: u32,
        _: &'static str,
        _: usize,
    ) -> Result<Self, Error> {
        self.variant(tag::TUPLE_VARIANT, index);
        Ok(self)
    }

    fn serialize_map(self, _: Option<usize>) -> Result<Self, Error> {
        self.tag(tag::MAP);
        Ok(self)
    }

    fn serialize_struct(self, _: &'static str, _: usize) -> Result<Self, Error> {
        self.tag(tag::STRUCT);
        Ok(self)
    }

    fn serialize_struct_variant(
        self,
        _: &'static str,
        index: u32,
        _: &'static str,
        _: usize,
    ) -> Result<Self, Error> {
        self.variant(tag::STRUCT_VARIANT, index);
        Ok(self)
    }
}

impl Writer<'_> {
    fn element<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Error> {
        self.tag(tag::ELEMENT);
        value.serialize(self)
    }

    fn field<T: Serialize + ?Sized>(&mut self, key: &'static str, value: &T) -> Result<(), Error> {
        self.tag(tag::FIELD);
        self.text(key);
        value.serialize(self)
    }

    fn end(&mut self) -> Result<(), Error> {
        self.tag(tag::END);
        Ok(())
    }
}

impl ser::SerializeSeq for &mut Writer<'_> {
    type Ok = ();
    type Error = Error;

    fn serialize_element<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Error> {
        self.element(value)
    }

    fn end(self) -> Result<(), Error> {
        Writer::end(self)
    }
}

impl ser::SerializeTuple for &mut Writer<'_> {
    type Ok = ();
    type Error = Error;

    fn serialize_element<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Error> {
        self.element(value)
    }

    fn end(self) -> Result<(), Error> {
        Writer::end(self)
    }
}

impl ser::SerializeTupleStruct for &mut Writer<'_> {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Error> {
        self.element(value)
    }

    fn end(self) -> Result<(), Error> {
        Writer::end(self)
    }
}

impl ser::SerializeTupleVariant for &mut Writer<'_> {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Error> {
        self.element(value)
    }

    fn end(self) -> Result<(), Error> {
        Writer::end(self)
    }
}

impl ser::SerializeMap for &mut Writer<'_> {
    type Ok = ();
    type Error = Error;

    fn serialize_key<T: Serialize + ?Sized>(&mut self, key: &T) -> Result<(), Error> {
        self.tag(tag::KEY);
        key.serialize(&mut **self)
    }

    fn serialize_value<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Error> {
        self.tag(tag::VALUE);
        value.serialize(&mut **self)
    }

    fn end(self) -> Result<(), Error> {
        Writer::end(self)
    }
}

impl ser::SerializeStruct for &mut Writer<'_> {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T: Serialize + ?Sized>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), Error> {
        self.field(key, value)
    }

    fn skip_field(&mut self, key: &'static str) -> Result<(), Error> {
        self.tag(tag::SKIPPED);
        self.text(key);
        Ok(())
    }

    fn end(self) -> Result<(), Error> {
        Writer::end(self)
    }
}

impl ser::SerializeStructVariant for &mut Writer<'_> {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T: Serialize + ?Sized>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), Error> {
        self.field(key, value)
    }

    fn skip_field(&mut self, key: &'static str) -> Result<(), Error> {
        self.tag(tag::SKIPPED);
        self.text(key);
        Ok(())
    }

    fn end(self) -> Result<(), Error> {
        Writer::end(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest<T: Serialize>(value: &T) -> Vec<u8> {
        let mut bytes = Vec::new();
        append(&mut bytes, value);
        bytes
    }

    #[derive(serde::Serialize)]
    struct Pair {
        #[serde(skip_serializing_if = "Option::is_none")]
        left: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        right: Option<String>,
    }

    #[test]
    fn values_that_serialize_differently_digest_differently() {
        // A skipped field is named: one value on either side differs.
        let left = Pair {
            left: Some("a".into()),
            right: None,
        };
        let right = Pair {
            left: None,
            right: Some("a".into()),
        };
        assert_ne!(digest(&left), digest(&right));
        // Lengths delimit: ["ab", "c"] is not ["a", "bc"].
        assert_ne!(digest(&vec!["ab", "c"]), digest(&vec!["a", "bc"]));
        // Nesting delimits: [[1], [2]] is not [[1, 2]].
        assert_ne!(digest(&vec![vec![1], vec![2]]), digest(&vec![vec![1, 2]]));
        assert_ne!(digest(&Some(0_u32)), digest(&None::<u32>));
        assert_eq!(digest(&vec!["ab", "c"]), digest(&vec!["ab", "c"]));
    }
}
