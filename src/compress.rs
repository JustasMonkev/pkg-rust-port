use std::str::FromStr;

use thiserror::Error;

/// Compression algorithm used for payload entries.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Compression {
    /// Store payload entries without compression.
    None,
    /// Compress payload entries with gzip.
    Gzip,
    /// Compress payload entries with Brotli.
    Brotli,
}

impl Compression {
    /// Return the original JavaScript enum index for fixture parity.
    ///
    /// # Example
    ///
    /// ```
    /// assert_eq!(pkg_rust::Compression::Brotli.as_index(), 2);
    /// ```
    #[must_use]
    pub const fn as_index(self) -> u8 {
        match self {
            Self::None => 0,
            Self::Gzip => 1,
            Self::Brotli => 2,
        }
    }

    /// Return the original JavaScript enum label.
    ///
    /// # Example
    ///
    /// ```
    /// assert_eq!(pkg_rust::Compression::Gzip.cli_label(), "GZip");
    /// ```
    #[must_use]
    pub const fn cli_label(self) -> &'static str {
        match self {
            Self::None => "None",
            Self::Gzip => "GZip",
            Self::Brotli => "Brotli",
        }
    }
}

impl FromStr for Compression {
    type Err = CompressionParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.to_ascii_lowercase().as_str() {
            "none" => Ok(Self::None),
            "gzip" | "gz" => Ok(Self::Gzip),
            "brotli" | "br" => Ok(Self::Brotli),
            _ => Err(CompressionParseError {
                value: value.to_owned(),
            }),
        }
    }
}

/// Error returned when a compression name is not supported.
#[derive(Debug, Error, Eq, PartialEq)]
#[error("Invalid compression algorithm {value} ( should be None, Brotli or Gzip)")]
pub struct CompressionParseError {
    value: String,
}
