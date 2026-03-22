use thiserror::Error;

use crate::{MAX_NAME_LEN, MIN_NAME_LEN};

#[derive(Error, Debug, Clone, PartialEq)]
pub enum ValidationError {
    #[error("Name too short: must be at least {MIN_NAME_LEN} characters")]
    TooShort,
    #[error("Name too long: must be at most {MAX_NAME_LEN} characters")]
    TooLong,
    #[error("Name contains invalid characters: only a-z allowed")]
    InvalidCharacters,
}

pub fn validate_name(name: &str) -> Result<[u8; 32], ValidationError> {
    let len = name.len();

    if len < MIN_NAME_LEN {
        return Err(ValidationError::TooShort);
    }

    if len > MAX_NAME_LEN {
        return Err(ValidationError::TooLong);
    }

    let mut normalized = [0u8; 32];
    for (i, c) in name.chars().enumerate() {
        if c.is_ascii_lowercase() {
            normalized[i] = c as u8;
        } else if c.is_ascii_uppercase() {
            normalized[i] = c.to_ascii_lowercase() as u8;
        } else {
            return Err(ValidationError::InvalidCharacters);
        }
    }

    Ok(normalized)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    fn test_valid_lowercase_name() {
        let result = validate_name("alice").unwrap();
        assert_eq!(&result[..5], b"alice");
    }

    #[rstest]
    fn test_uppercase_normalization() {
        let result = validate_name("ALICE").unwrap();
        assert_eq!(&result[..5], b"alice");
    }

    #[rstest]
    fn test_mixed_case_normalization() {
        let result = validate_name("AlIcE").unwrap();
        assert_eq!(&result[..5], b"alice");
    }

    #[rstest]
    fn test_name_too_short() {
        assert_eq!(validate_name("ab"), Err(ValidationError::TooShort));
        assert_eq!(validate_name("a"), Err(ValidationError::TooShort));
        assert_eq!(validate_name(""), Err(ValidationError::TooShort));
    }

    #[rstest]
    fn test_name_too_long() {
        let long_name = "a".repeat(33);
        assert_eq!(validate_name(&long_name), Err(ValidationError::TooLong));
    }

    #[rstest]
    fn test_name_max_length() {
        let max_name = "a".repeat(32);
        assert!(validate_name(&max_name).is_ok());
    }

    #[rstest]
    fn test_invalid_characters_digits() {
        assert_eq!(
            validate_name("alice123"),
            Err(ValidationError::InvalidCharacters)
        );
    }

    #[rstest]
    fn test_invalid_characters_special() {
        assert_eq!(
            validate_name("alice_"),
            Err(ValidationError::InvalidCharacters)
        );
        assert_eq!(
            validate_name("alice-bob"),
            Err(ValidationError::InvalidCharacters)
        );
        assert_eq!(
            validate_name("alice.bob"),
            Err(ValidationError::InvalidCharacters)
        );
    }

    #[rstest]
    fn test_invalid_characters_spaces() {
        assert_eq!(
            validate_name("alice bob"),
            Err(ValidationError::InvalidCharacters)
        );
    }
}
