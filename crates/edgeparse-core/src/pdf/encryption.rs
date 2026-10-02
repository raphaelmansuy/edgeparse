//! PDF encryption detection and password-based loading.

use lopdf::Document;

/// Information about a PDF's encryption status.
#[derive(Debug, Clone)]
pub struct EncryptionInfo {
    /// Whether the document is encrypted.
    pub is_encrypted: bool,
    /// Encryption algorithm version (V value from /Encrypt).
    pub version: Option<i64>,
    /// Key length in bits.
    pub key_length: Option<i64>,
    /// Encryption filter name (e.g., "Standard").
    pub filter: Option<String>,
    /// Permissions flags (P value).
    pub permissions: Option<i64>,
}

impl EncryptionInfo {
    /// Check if printing is allowed (bit 3 of permissions).
    pub fn can_print(&self) -> bool {
        self.permissions.is_none_or(|p| p & 0x4 != 0)
    }

    /// Check if content copying is allowed (bit 5 of permissions).
    pub fn can_copy(&self) -> bool {
        self.permissions.is_none_or(|p| p & 0x10 != 0)
    }

    /// Check if modification is allowed (bit 4 of permissions).
    pub fn can_modify(&self) -> bool {
        self.permissions.is_none_or(|p| p & 0x8 != 0)
    }
}

/// Detect encryption info from a loaded PDF document.
///
/// Note: if the document couldn't be loaded due to encryption, this function
/// can't inspect it. Use [`detect_encryption_from_bytes`] for raw file analysis.
pub fn detect_encryption(doc: &Document) -> EncryptionInfo {
    let trailer = &doc.trailer;

    let encrypt_dict = trailer.get(b"Encrypt").ok().and_then(|obj| match obj {
        lopdf::Object::Dictionary(d) => Some(d.clone()),
        lopdf::Object::Reference(id) => doc
            .get_object(*id)
            .ok()
            .and_then(|o| o.as_dict().ok().cloned()),
        _ => None,
    });

    let Some(dict) = encrypt_dict else {
        return EncryptionInfo {
            is_encrypted: false,
            version: None,
            key_length: None,
            filter: None,
            permissions: None,
        };
    };

    let version = dict.get(b"V").ok().and_then(|o| {
        if let lopdf::Object::Integer(i) = o {
            Some(*i)
        } else {
            None
        }
    });

    let key_length = dict.get(b"Length").ok().and_then(|o| {
        if let lopdf::Object::Integer(i) = o {
            Some(*i)
        } else {
            None
        }
    });

    let filter = dict.get(b"Filter").ok().and_then(|o| match o {
        lopdf::Object::Name(n) => String::from_utf8(n.clone()).ok(),
        _ => None,
    });

    let permissions = dict.get(b"P").ok().and_then(|o| {
        if let lopdf::Object::Integer(i) = o {
            Some(*i)
        } else {
            None
        }
    });

    EncryptionInfo {
        is_encrypted: true,
        version,
        key_length,
        filter,
        permissions,
    }
}

/// Try to load a PDF with an optional password.
///
/// Uses the Standard Security Handler decrypt path in `pdf-cos` (lopdf)
/// for revisions 2–6 when a password is supplied.
pub fn load_with_password(
    data: &[u8],
    password: Option<&str>,
) -> Result<Document, crate::EdgePdfError> {
    let mut doc = Document::load_mem(data).map_err(|e| {
        crate::EdgePdfError::LoadError(format!(
            "Failed to load PDF (may be encrypted — try providing a password): {e}"
        ))
    })?;

    decrypt_document(&mut doc, password)?;
    Ok(doc)
}

/// Decrypt an already-parsed document in place when encrypted.
///
/// No-op when the document is not encrypted. When encrypted and `password` is
/// `None`, attempts an empty-user-password decrypt (common for owner-only locks).
pub fn decrypt_document(
    doc: &mut Document,
    password: Option<&str>,
) -> Result<(), crate::EdgePdfError> {
    if !doc.is_encrypted() {
        return Ok(());
    }

    let candidates: Vec<&str> = match password {
        Some(p) => vec![p, ""],
        None => vec![""],
    };

    let mut last_err = None;
    for pw in candidates {
        match doc.decrypt(pw) {
            Ok(()) => return Ok(()),
            Err(e) => last_err = Some(e),
        }
    }

    Err(crate::EdgePdfError::LoadError(format!(
        "Failed to decrypt PDF (password may be incorrect): {}",
        last_err
            .map(|e| e.to_string())
            .unwrap_or_else(|| "unknown error".into())
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unencrypted_document() {
        let doc = Document::new();
        let info = detect_encryption(&doc);
        assert!(!info.is_encrypted);
        assert!(info.version.is_none());
        assert!(info.can_print());
        assert!(info.can_copy());
        assert!(info.can_modify());
    }

    #[test]
    fn test_permissions_parsing() {
        let info = EncryptionInfo {
            is_encrypted: true,
            version: Some(2),
            key_length: Some(128),
            filter: Some("Standard".to_string()),
            permissions: Some(-1),
        };
        assert!(info.can_print());
        assert!(info.can_copy());
        assert!(info.can_modify());
    }

    #[test]
    fn test_restricted_permissions() {
        let info = EncryptionInfo {
            is_encrypted: true,
            version: Some(2),
            key_length: Some(128),
            filter: Some("Standard".to_string()),
            permissions: Some(0),
        };
        assert!(!info.can_print());
        assert!(!info.can_copy());
        assert!(!info.can_modify());
    }

    #[test]
    fn test_load_empty_pdf_bytes() {
        let mut doc = Document::new();
        let mut buf = Vec::new();
        doc.save_to(&mut buf).unwrap();
        let result = load_with_password(&buf, None);
        assert!(result.is_ok());
    }

    #[test]
    fn decrypt_noop_on_plain_doc() {
        let mut doc = Document::new();
        assert!(decrypt_document(&mut doc, Some("secret")).is_ok());
    }
}
