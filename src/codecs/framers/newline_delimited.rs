use crate::codecs::{BoxedFramer, BoxedFramingError, CharacterDelimitedCodec, FramingConfig};
use bytes::{Bytes, BytesMut};
use serde::{Deserialize, Serialize};
use tokio_util::codec::Decoder;

/// Config used to build a `NewlineDelimitedCodec`.
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
pub struct NewlineDelimitedDecoderConfig {
    #[serde(
        default,
        skip_serializing_if = "crate::serde::skip_serializing_if_default"
    )]
    newline_delimited: NewlineDelimitedDecoderOptions,
}

/// Options for building a `NewlineDelimitedCodec`.
#[derive(Debug, Clone, Derivative, Deserialize, Serialize, PartialEq)]
#[derivative(Default)]
pub struct NewlineDelimitedDecoderOptions {
    /// The maximum length of the byte buffer.
    ///
    /// This length does *not* include the trailing delimiter.
    #[serde(skip_serializing_if = "crate::serde::skip_serializing_if_default")]
    max_length: Option<usize>,
}

impl NewlineDelimitedDecoderOptions {
    /// Creates a `NewlineDelimitedDecoderOptions` with a maximum frame length limit.
    pub const fn new_with_max_length(max_length: usize) -> Self {
        Self {
            max_length: Some(max_length),
        }
    }
}

impl NewlineDelimitedDecoderConfig {
    /// Creates a new `NewlineDelimitedDecoderConfig`.
    pub fn new() -> Self {
        Default::default()
    }

    /// Creates a `NewlineDelimitedCodec` with a maximum frame length limit.
    pub const fn new_with_max_length(max_length: usize) -> Self {
        Self {
            newline_delimited: { NewlineDelimitedDecoderOptions::new_with_max_length(max_length) },
        }
    }
}

#[typetag::serde(name = "newline_delimited")]
impl FramingConfig for NewlineDelimitedDecoderConfig {
    fn build(&self) -> crate::Result<BoxedFramer> {
        if let Some(max_length) = self.newline_delimited.max_length {
            Ok(Box::new(NewlineDelimitedCodec::new_with_max_length(
                max_length,
            )))
        } else {
            Ok(Box::new(NewlineDelimitedCodec::new()))
        }
    }
}

/// A codec for handling bytes that are delimited by (a) newline(s).
#[derive(Debug, Clone)]
pub struct NewlineDelimitedCodec(CharacterDelimitedCodec);

impl NewlineDelimitedCodec {
    /// Creates a new `NewlineDelimitedCodec`.
    pub const fn new() -> Self {
        Self(CharacterDelimitedCodec::new('\n'))
    }

    /// Creates a `NewlineDelimitedCodec` with a maximum frame length limit.
    ///
    /// Any frames longer than `max_length` bytes will be discarded entirely.
    pub const fn new_with_max_length(max_length: usize) -> Self {
        Self(CharacterDelimitedCodec::new_with_max_length(
            '\n', max_length,
        ))
    }
}

impl Default for NewlineDelimitedCodec {
    fn default() -> Self {
        Self::new()
    }
}

impl Decoder for NewlineDelimitedCodec {
    type Item = Bytes;
    type Error = BoxedFramingError;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        self.0.decode(src)
    }

    fn decode_eof(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        self.0.decode_eof(src)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_bytes_with_newlines() {
        let mut input = BytesMut::from("foo\nbar\nbaz");
        let mut decoder = NewlineDelimitedCodec::new();

        assert_eq!(decoder.decode(&mut input).unwrap().unwrap(), "foo");
        assert_eq!(decoder.decode(&mut input).unwrap().unwrap(), "bar");
        assert_eq!(decoder.decode(&mut input).unwrap(), None);
    }

    #[test]
    fn decode_bytes_with_newlines_trailing() {
        let mut input = BytesMut::from("foo\nbar\nbaz\n");
        let mut decoder = NewlineDelimitedCodec::new();

        assert_eq!(decoder.decode(&mut input).unwrap().unwrap(), "foo");
        assert_eq!(decoder.decode(&mut input).unwrap().unwrap(), "bar");
        assert_eq!(decoder.decode(&mut input).unwrap().unwrap(), "baz");
        assert_eq!(decoder.decode(&mut input).unwrap(), None);
    }

    #[test]
    fn decode_bytes_with_newlines_and_max_length() {
        let mut input = BytesMut::from("foo\nbarbara\nbaz\n");
        let mut decoder = NewlineDelimitedCodec::new_with_max_length(3);

        assert_eq!(decoder.decode(&mut input).unwrap().unwrap(), "foo");
        assert_eq!(decoder.decode(&mut input).unwrap(), None);
        assert_eq!(decoder.decode(&mut input).unwrap().unwrap(), "baz");
        assert_eq!(decoder.decode(&mut input).unwrap(), None);
    }

    #[test]
    fn decode_eof_bytes_with_newlines() {
        let mut input = BytesMut::from("foo\nbar\nbaz");
        let mut decoder = NewlineDelimitedCodec::new();

        assert_eq!(decoder.decode_eof(&mut input).unwrap().unwrap(), "foo");
        assert_eq!(decoder.decode_eof(&mut input).unwrap().unwrap(), "bar");
        assert_eq!(decoder.decode_eof(&mut input).unwrap().unwrap(), "baz");
    }

    #[test]
    fn decode_eof_bytes_with_newlines_trailing() {
        let mut input = BytesMut::from("foo\nbar\nbaz\n");
        let mut decoder = NewlineDelimitedCodec::new();

        assert_eq!(decoder.decode_eof(&mut input).unwrap().unwrap(), "foo");
        assert_eq!(decoder.decode_eof(&mut input).unwrap().unwrap(), "bar");
        assert_eq!(decoder.decode_eof(&mut input).unwrap().unwrap(), "baz");
        assert_eq!(decoder.decode_eof(&mut input).unwrap(), None);
    }

    #[test]
    fn decode_eof_bytes_with_newlines_and_max_length() {
        let mut input = BytesMut::from("foo\nbarbara\nbaz\n");
        let mut decoder = NewlineDelimitedCodec::new_with_max_length(3);

        assert_eq!(decoder.decode_eof(&mut input).unwrap().unwrap(), "foo");
        assert_eq!(decoder.decode_eof(&mut input).unwrap(), None);
        assert_eq!(decoder.decode_eof(&mut input).unwrap().unwrap(), "baz");
        assert_eq!(decoder.decode_eof(&mut input).unwrap(), None);
    }
}
