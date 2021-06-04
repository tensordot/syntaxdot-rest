use alpino_tokenizer::Tokenizer;

/// Simple whitespace-based tokenizer.
///
/// Splits sentences on newlines (`\n` or `\r\n`) and tokens on ASCII whitespace.
pub struct WhitespaceTokenizer;

impl Tokenizer for WhitespaceTokenizer {
    fn tokenize(&self, text: &str) -> Option<Vec<Vec<String>>> {
        Some(
            text.lines()
                .map(|s| s.split_ascii_whitespace().map(ToOwned::to_owned).collect())
                .collect(),
        )
    }
}

#[cfg(test)]
mod tests {
    use alpino_tokenizer::Tokenizer;

    use crate::tokenizer::WhitespaceTokenizer;

    #[test]
    fn test_whitespace_tokenizer() {
        let tokenizer = WhitespaceTokenizer;
        assert_eq!(
            tokenizer.tokenize("Dit is een zin .\nDit is de tweede zin .\r\nEn de derde zin\n"),
            Some(vec![
                vec![
                    "Dit".to_string(),
                    "is".to_string(),
                    "een".to_string(),
                    "zin".to_string(),
                    ".".to_string()
                ],
                vec![
                    "Dit".to_string(),
                    "is".to_string(),
                    "de".to_string(),
                    "tweede".to_string(),
                    "zin".to_string(),
                    ".".to_string()
                ],
                vec![
                    "En".to_string(),
                    "de".to_string(),
                    "derde".to_string(),
                    "zin".to_string()
                ]
            ])
        )
    }
}
