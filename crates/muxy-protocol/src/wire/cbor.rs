//! CBOR helpers for protocol types.
//!
//! Fields and variants carry explicit numbers instead of positions, so a peer
//! skips fields it doesn't know and reads missing optional fields as `None`.

/// Byte fields and UUIDs as CBOR byte strings. Without this, CBOR writes a
/// byte vector as one item per byte.
pub mod bytes {
    use std::sync::Arc;

    use minicbor::data::Type;
    use minicbor::decode::Error as DecodeError;
    use minicbor::encode::{Error as EncodeError, Write};
    use minicbor::{Decoder, Encoder};
    use uuid::Uuid;

    pub trait Bytes: Sized {
        fn encode<W: Write>(&self, encoder: &mut Encoder<W>) -> Result<(), EncodeError<W::Error>>;

        fn decode(decoder: &mut Decoder<'_>) -> Result<Self, DecodeError>;
    }

    impl Bytes for Vec<u8> {
        fn encode<W: Write>(&self, encoder: &mut Encoder<W>) -> Result<(), EncodeError<W::Error>> {
            encoder.bytes(self)?;
            Ok(())
        }

        fn decode(decoder: &mut Decoder<'_>) -> Result<Self, DecodeError> {
            decoder.bytes().map(<[u8]>::to_vec)
        }
    }

    impl Bytes for Arc<[u8]> {
        fn encode<W: Write>(&self, encoder: &mut Encoder<W>) -> Result<(), EncodeError<W::Error>> {
            encoder.bytes(self)?;
            Ok(())
        }

        fn decode(decoder: &mut Decoder<'_>) -> Result<Self, DecodeError> {
            decoder.bytes().map(Arc::from)
        }
    }

    impl<const N: usize> Bytes for [u8; N] {
        fn encode<W: Write>(&self, encoder: &mut Encoder<W>) -> Result<(), EncodeError<W::Error>> {
            encoder.bytes(self)?;
            Ok(())
        }

        fn decode(decoder: &mut Decoder<'_>) -> Result<Self, DecodeError> {
            let position = decoder.position();
            Self::try_from(decoder.bytes()?)
                .map_err(|_| DecodeError::message("byte string has the wrong length").at(position))
        }
    }

    impl Bytes for Uuid {
        fn encode<W: Write>(&self, encoder: &mut Encoder<W>) -> Result<(), EncodeError<W::Error>> {
            encoder.bytes(self.as_bytes())?;
            Ok(())
        }

        fn decode(decoder: &mut Decoder<'_>) -> Result<Self, DecodeError> {
            <[u8; 16]>::decode(decoder).map(Self::from_bytes)
        }
    }

    impl<T: Bytes> Bytes for Option<T> {
        fn encode<W: Write>(&self, encoder: &mut Encoder<W>) -> Result<(), EncodeError<W::Error>> {
            if let Some(value) = self {
                return value.encode(encoder);
            }
            encoder.null()?;
            Ok(())
        }

        fn decode(decoder: &mut Decoder<'_>) -> Result<Self, DecodeError> {
            if decoder.datatype()? == Type::Null {
                decoder.null()?;
                return Ok(None);
            }
            T::decode(decoder).map(Some)
        }
    }

    pub fn encode<C, T: Bytes, W: Write>(
        value: &T,
        encoder: &mut Encoder<W>,
        _: &mut C,
    ) -> Result<(), EncodeError<W::Error>> {
        value.encode(encoder)
    }

    pub fn decode<C, T: Bytes>(decoder: &mut Decoder<'_>, _: &mut C) -> Result<T, DecodeError> {
        T::decode(decoder)
    }
}

/// Screen rows keep the compact postcard layout that saved records also store.
///
/// The layout is frozen: changing a row type changes both the wire and saved records.
pub mod rows {
    use minicbor::decode::Error as DecodeError;
    use minicbor::encode::{Error as EncodeError, Write};
    use minicbor::{Decoder, Encoder};

    use crate::Row;

    pub fn encode<C, W: Write>(
        rows: &[Row],
        encoder: &mut Encoder<W>,
        _: &mut C,
    ) -> Result<(), EncodeError<W::Error>> {
        let bytes = postcard::to_allocvec(rows).map_err(EncodeError::custom)?;
        encoder.bytes(&bytes)?;
        Ok(())
    }

    pub fn decode<C>(decoder: &mut Decoder<'_>, _: &mut C) -> Result<Vec<Row>, DecodeError> {
        let position = decoder.position();
        let (rows, trailing) = postcard::take_from_bytes(decoder.bytes()?)
            .map_err(|error| DecodeError::custom(error).at(position))?;
        if !trailing.is_empty() {
            return Err(DecodeError::message("trailing bytes after screen rows").at(position));
        }
        Ok(rows)
    }
}

/// Enums the server sends, which older builds must still read.
///
/// A variant without a value is written as its number, and a variant with a
/// value as `[number, value]`. A build that doesn't know a number keeps it as
/// an unrecognized variant and skips its value, so a newer server can add
/// variants without breaking older clients.
pub mod open {
    use minicbor::data::Type;
    use minicbor::decode::Error as DecodeError;
    use minicbor::encode::{Error as EncodeError, Write};
    use minicbor::{Decoder, Encode, Encoder};

    /// A variant's number, read before its value.
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum Variant {
        Unit(u32),
        Value(u32),
    }

    pub fn encode_unit<W: Write>(
        index: u32,
        encoder: &mut Encoder<W>,
    ) -> Result<(), EncodeError<W::Error>> {
        encoder.u32(index)?;
        Ok(())
    }

    pub fn encode_value<C, T: Encode<C>, W: Write>(
        index: u32,
        value: &T,
        encoder: &mut Encoder<W>,
        ctx: &mut C,
    ) -> Result<(), EncodeError<W::Error>> {
        encoder.array(2)?.u32(index)?.encode_with(value, ctx)?;
        Ok(())
    }

    /// Reads a variant's number. The value of a [`Variant::Value`] comes next:
    /// decode it, or pass the variant to [`unrecognized`].
    pub fn variant(decoder: &mut Decoder<'_>) -> Result<Variant, DecodeError> {
        if decoder.datatype()? != Type::Array {
            return Ok(Variant::Unit(decoder.u32()?));
        }
        let position = decoder.position();
        if decoder.array()? != Some(2) {
            return Err(DecodeError::message("expected a [number, value] variant").at(position));
        }
        Ok(Variant::Value(decoder.u32()?))
    }

    /// Skips the value of a variant this build doesn't know and returns its number.
    pub fn unrecognized(decoder: &mut Decoder<'_>, variant: Variant) -> Result<u32, DecodeError> {
        match variant {
            Variant::Unit(index) => Ok(index),
            Variant::Value(index) => {
                decoder.skip()?;
                Ok(index)
            }
        }
    }
}

/// Declares an open enum whose variants carry no values. Each variant's number
/// is written after `=` and never changes; an unknown number decodes as
/// `Unrecognized`.
macro_rules! open_enum {
    (
        $(#[$meta:meta])*
        $vis:vis enum $name:ident {
            $($(#[$variant_meta:meta])* $variant:ident = $index:literal,)*
        }
    ) => {
        $(#[$meta])*
        $vis enum $name {
            $($(#[$variant_meta])* $variant,)*
            /// A variant from a newer build, kept by its number.
            Unrecognized(u32),
        }

        impl<C> minicbor::Encode<C> for $name {
            fn encode<W: minicbor::encode::Write>(
                &self,
                encoder: &mut minicbor::Encoder<W>,
                _: &mut C,
            ) -> Result<(), minicbor::encode::Error<W::Error>> {
                let index = match self {
                    $(Self::$variant => $index,)*
                    Self::Unrecognized(index) => *index,
                };
                $crate::wire::cbor::open::encode_unit(index, encoder)
            }
        }

        impl<'b, C> minicbor::Decode<'b, C> for $name {
            fn decode(
                decoder: &mut minicbor::Decoder<'b>,
                _: &mut C,
            ) -> Result<Self, minicbor::decode::Error> {
                use $crate::wire::cbor::open::{self, Variant};
                Ok(match open::variant(decoder)? {
                    $(Variant::Unit($index) => Self::$variant,)*
                    other => Self::Unrecognized(open::unrecognized(decoder, other)?),
                })
            }
        }
    };
}
pub(crate) use open_enum;

#[cfg(test)]
mod tests {
    use std::error::Error;

    use minicbor::{Decoder, Encoder};

    #[test]
    fn fixed_length_bytes_reject_other_lengths() -> Result<(), Box<dyn Error>> {
        let mut bytes = Vec::new();
        Encoder::new(&mut bytes).bytes(&[1, 2, 3])?;
        let result = super::bytes::decode::<(), [u8; 4]>(&mut Decoder::new(&bytes), &mut ());
        assert!(result.is_err());
        Ok(())
    }
}
