use crate::{LexicalPolicy, WriteError, XmlDocument, XmlWriter};

/// Lossless source tree. Opaque ownership stays exclusively in canonical IR.
///
/// XML-004 does not rewrite typed slots yet.  Keeping the parsed tree (rather
/// than a raw whole-document byte blob) means XML-005 can replace only typed
/// slots while all sibling nodes retain their original lexical representation.
#[derive(Clone, Debug)]
pub(crate) struct Fallback {
    document: XmlDocument,
}

impl Fallback {
    pub(crate) fn new(document: XmlDocument) -> Self {
        Self { document }
    }

    pub(crate) fn emit(&self) -> Result<Vec<u8>, FallbackEmitError> {
        Ok(XmlWriter::to_vec(&self.document, LexicalPolicy::Preserve)?)
    }
    pub(crate) fn document(&self) -> &XmlDocument {
        &self.document
    }
    pub(crate) fn into_document(self) -> XmlDocument {
        self.document
    }
}

#[derive(Debug)]
pub(crate) enum FallbackEmitError {
    Write(WriteError),
}
impl From<WriteError> for FallbackEmitError {
    fn from(value: WriteError) -> Self {
        Self::Write(value)
    }
}
