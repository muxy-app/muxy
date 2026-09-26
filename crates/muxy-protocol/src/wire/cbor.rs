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
    use std::sync::Arc;

    use minicbor::data::Type;
    use minicbor::decode::Error as DecodeError;
    use minicbor::encode::{Error as EncodeError, Write};
    use minicbor::{Decode, Decoder, Encode, Encoder};
    use uuid::Uuid;

    use super::open::{self, Variant};
    use crate::{Color, Row, Run, Style};

    #[derive(Debug, PartialEq, Encode, Decode)]
    struct Before {
        #[n(0)]
        id: u32,
    }

    #[derive(Debug, PartialEq, Encode, Decode)]
    struct After {
        #[n(0)]
        id: u32,
        #[n(1)]
        label: Option<String>,
    }

    #[test]
    fn older_builds_skip_fields_they_do_not_know() -> Result<(), Box<dyn Error>> {
        let bytes = minicbor::to_vec(After {
            id: 7,
            label: Some("new".into()),
        })?;
        assert_eq!(minicbor::decode::<Before>(&bytes)?, Before { id: 7 });
        Ok(())
    }

    #[test]
    fn newer_builds_read_missing_optional_fields_as_none() -> Result<(), Box<dyn Error>> {
        let bytes = minicbor::to_vec(Before { id: 7 })?;
        assert_eq!(
            minicbor::decode::<After>(&bytes)?,
            After { id: 7, label: None }
        );
        Ok(())
    }

    #[derive(Debug, PartialEq)]
    enum Status {
        Idle,
        Exited(i32),
        Unrecognized(u32),
    }

    impl<C> Encode<C> for Status {
        fn encode<W: Write>(
            &self,
            encoder: &mut Encoder<W>,
            ctx: &mut C,
        ) -> Result<(), EncodeError<W::Error>> {
            match self {
                Self::Idle => open::encode_unit(0, encoder),
                Self::Exited(code) => open::encode_value(1, code, encoder, ctx),
                Self::Unrecognized(index) => open::encode_unit(*index, encoder),
            }
        }
    }

    impl<'b, C> Decode<'b, C> for Status {
        fn decode(decoder: &mut Decoder<'b>, ctx: &mut C) -> Result<Self, DecodeError> {
            Ok(match open::variant(decoder)? {
                Variant::Unit(0) => Self::Idle,
                Variant::Value(1) => Self::Exited(decoder.decode_with(ctx)?),
                other => Self::Unrecognized(open::unrecognized(decoder, other)?),
            })
        }
    }

    /// The same enum in a newer build, with two variants the older one lacks.
    enum NewerStatus {
        Exited(i32),
        Paused,
        Failed(String),
    }

    impl<C> Encode<C> for NewerStatus {
        fn encode<W: Write>(
            &self,
            encoder: &mut Encoder<W>,
            ctx: &mut C,
        ) -> Result<(), EncodeError<W::Error>> {
            match self {
                Self::Exited(code) => open::encode_value(1, code, encoder, ctx),
                Self::Paused => open::encode_unit(2, encoder),
                Self::Failed(reason) => open::encode_value(3, reason, encoder, ctx),
            }
        }
    }

    #[test]
    fn unknown_variants_are_kept_by_number_and_their_values_skipped() -> Result<(), Box<dyn Error>>
    {
        let bytes = minicbor::to_vec(vec![
            NewerStatus::Paused,
            NewerStatus::Failed("disk full".into()),
            NewerStatus::Exited(3),
        ])?;
        assert_eq!(
            minicbor::decode::<Vec<Status>>(&bytes)?,
            [
                Status::Unrecognized(2),
                Status::Unrecognized(3),
                Status::Exited(3)
            ]
        );
        Ok(())
    }

    #[test]
    fn open_enums_round_trip_including_unrecognized_variants() -> Result<(), Box<dyn Error>> {
        for status in [Status::Idle, Status::Exited(-1), Status::Unrecognized(9)] {
            let bytes = minicbor::to_vec(&status)?;
            assert_eq!(minicbor::decode::<Status>(&bytes)?, status);
        }
        Ok(())
    }

    #[test]
    fn malformed_variants_are_rejected() -> Result<(), Box<dyn Error>> {
        let three = minicbor::to_vec((1_u32, 2_u32, 3_u32))?;
        assert!(minicbor::decode::<Status>(&three).is_err());
        let text = minicbor::to_vec("idle")?;
        assert!(minicbor::decode::<Status>(&text).is_err());
        Ok(())
    }

    #[derive(Debug, PartialEq, Encode, Decode)]
    struct Screen {
        #[n(0)]
        #[cbor(with = "super::rows")]
        rows: Vec<Row>,
    }

    fn sample_rows() -> Vec<Row> {
        vec![Row {
            index: 0,
            runs: vec![Run {
                text: "Muxy".into(),
                width: 4,
                style: Style {
                    fg: Color::Rgb(80, 180, 240),
                    bold: true,
                    ..Style::default()
                },
            }],
        }]
    }

    #[test]
    fn rows_are_postcard_bytes_inside_a_byte_string() -> Result<(), Box<dyn Error>> {
        let rows = sample_rows();
        let bytes = minicbor::to_vec(Screen { rows: rows.clone() })?;
        let mut decoder = Decoder::new(&bytes);
        assert_eq!(decoder.array()?, Some(1));
        assert_eq!(decoder.bytes()?, postcard::to_allocvec(&rows)?.as_slice());
        assert_eq!(minicbor::decode::<Screen>(&bytes)?, Screen { rows });
        Ok(())
    }

    #[test]
    fn rows_with_trailing_bytes_are_rejected() -> Result<(), Box<dyn Error>> {
        let mut rows = postcard::to_allocvec(&sample_rows())?;
        rows.push(0);
        let mut bytes = Vec::new();
        Encoder::new(&mut bytes).array(1)?.bytes(&rows)?;
        assert!(minicbor::decode::<Screen>(&bytes).is_err());
        Ok(())
    }

    #[derive(Debug, PartialEq, Encode, Decode)]
    struct Blobs {
        #[n(0)]
        #[cbor(with = "super::bytes")]
        input: Vec<u8>,
        #[n(1)]
        #[cbor(with = "super::bytes")]
        image: Arc<[u8]>,
        #[n(2)]
        #[cbor(with = "super::bytes")]
        fingerprint: [u8; 4],
        #[n(3)]
        #[cbor(with = "super::bytes")]
        project: Uuid,
        #[n(4)]
        #[cbor(with = "super::bytes")]
        logo: Option<Arc<[u8]>>,
    }

    fn sample_blobs() -> Blobs {
        Blobs {
            input: vec![0xff, 0x80, 0],
            image: Arc::from([1, 2, 3].as_slice()),
            fingerprint: [9, 8, 7, 6],
            project: Uuid::from_u128(0x0123_4567_89ab_cdef_0123_4567_89ab_cdef),
            logo: Some(Arc::from([0x89, b'P', b'N', b'G'].as_slice())),
        }
    }

    #[test]
    fn byte_fields_are_byte_strings() -> Result<(), Box<dyn Error>> {
        let blobs = sample_blobs();
        let bytes = minicbor::to_vec(&blobs)?;
        let mut decoder = Decoder::new(&bytes);
        assert_eq!(decoder.array()?, Some(5));
        for _ in 0..5 {
            assert_eq!(decoder.datatype()?, Type::Bytes);
            decoder.skip()?;
        }
        assert_eq!(minicbor::decode::<Blobs>(&bytes)?, blobs);
        Ok(())
    }

    #[test]
    fn missing_optional_bytes_are_none() -> Result<(), Box<dyn Error>> {
        let blobs = Blobs {
            logo: None,
            ..sample_blobs()
        };
        let bytes = minicbor::to_vec(&blobs)?;
        assert_eq!(Decoder::new(&bytes).array()?, Some(4));
        assert_eq!(minicbor::decode::<Blobs>(&bytes)?, blobs);
        Ok(())
    }

    #[test]
    fn fixed_length_bytes_reject_other_lengths() -> Result<(), Box<dyn Error>> {
        let mut bytes = Vec::new();
        Encoder::new(&mut bytes).bytes(&[1, 2, 3])?;
        let result = super::bytes::decode::<(), [u8; 4]>(&mut Decoder::new(&bytes), &mut ());
        assert!(result.is_err());
        Ok(())
    }
}
