/// How a discovered path is stored in the virtual filesystem payload.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StoreKind {
    /// V8 bytecode without source text.
    Blob,
    /// Raw file contents.
    Content,
    /// Directory child listing.
    Links,
    /// Serialized file metadata.
    Stat,
}

impl StoreKind {
    /// Return the original JavaScript store index for fixture parity.
    ///
    /// # Example
    ///
    /// ```
    /// assert_eq!(pkg_rust::StoreKind::Content.as_index(), 1);
    /// ```
    #[must_use]
    pub const fn as_index(self) -> u8 {
        match self {
            Self::Blob => 0,
            Self::Content => 1,
            Self::Links => 2,
            Self::Stat => 3,
        }
    }
}

/// How a discovered module alias should be resolved.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AliasKind {
    /// Resolve relative to the current source file.
    Relative,
    /// Resolve through Node package/module resolution.
    Resolvable,
}

impl AliasKind {
    /// Return the original JavaScript alias index for fixture parity.
    ///
    /// # Example
    ///
    /// ```
    /// assert_eq!(pkg_rust::AliasKind::Resolvable.as_index(), 1);
    /// ```
    #[must_use]
    pub const fn as_index(self) -> u8 {
        match self {
            Self::Relative => 0,
            Self::Resolvable => 1,
        }
    }
}
