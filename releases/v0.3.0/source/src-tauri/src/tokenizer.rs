use std::sync::Arc;

use jieba_rs::Jieba;
use tantivy::tokenizer::{Token, TokenStream, Tokenizer};

/// jieba 搜索切分分词器（tantivy 0.22 TokenStream 接口）。
#[derive(Clone)]
pub struct CnTokenizer {
    jieba: Arc<Jieba>,
}

impl Default for CnTokenizer {
    fn default() -> Self {
        Self {
            jieba: Arc::new(Jieba::new()),
        }
    }
}

pub struct CnTokenStream {
    tokens: Vec<Token>,
    index: usize,
}

impl Tokenizer for CnTokenizer {
    type TokenStream<'a> = CnTokenStream;

    fn token_stream<'a>(&'a mut self, text: &'a str) -> Self::TokenStream<'a> {
        // tokenize(Search) 直接给出重叠切分的正确字节偏移
        let mut tokens = Vec::new();
        let mut position = 0usize;
        for tok in self.jieba.tokenize(text, jieba_rs::TokenizeMode::Search, true) {
            if tok.word.is_empty() {
                continue;
            }
            tokens.push(Token {
                offset_from: tok.start,
                offset_to: tok.end,
                position,
                text: tok.word.to_lowercase(),
                position_length: 1,
            });
            position += 1;
        }
        CnTokenStream { tokens, index: 0 }
    }
}

impl TokenStream for CnTokenStream {
    fn advance(&mut self) -> bool {
        if self.index < self.tokens.len() {
            self.index += 1;
            true
        } else {
            false
        }
    }

    fn token(&self) -> &Token {
        &self.tokens[self.index.saturating_sub(1)]
    }

    fn token_mut(&mut self) -> &mut Token {
        &mut self.tokens[self.index.saturating_sub(1)]
    }
}

/// 索引格式版本：分词方案变更时递增，触发自动重建。
pub const INDEX_FORMAT: &str = "2";

/// 注册索引所需的全部分词器（n-gram 一律小写化）。
/// v2 瘦身：拼音 n-gram 上限 8→6，首字母 1→2，去掉单字噪声。
pub fn register(index: &tantivy::Index) -> Result<(), String> {
    use tantivy::tokenizer::{LowerCaser, TextAnalyzer};
    index.tokenizers().register("cn", CnTokenizer::default());
    for (name, min, max) in [
        ("bi", 2usize, 2usize),
        ("tri", 3, 3),
        ("py", 1, 6),
        ("pya", 2, 6),
    ] {
        let ngram = tantivy::tokenizer::NgramTokenizer::new(min, max, false)
            .map_err(|e| e.to_string())?;
        let analyzer = TextAnalyzer::builder(ngram).filter(LowerCaser).build();
        index.tokenizers().register(name, analyzer);
    }
    Ok(())
}

/// 查询串的 3-gram 序列（len>=3 时用于 tri 字段短语子串匹配）。
pub fn trigrams(q: &str) -> Vec<String> {
    let lower = q.to_lowercase();
    let chars: Vec<char> = lower.chars().collect();
    if chars.len() < 3 {
        return Vec::new();
    }
    chars
        .windows(3)
        .map(|w| w.iter().collect::<String>())
        .collect()
}

pub fn bigram(q: &str) -> Option<String> {
    let chars: Vec<char> = q.to_lowercase().chars().collect();
    if chars.len() == 2 {
        Some(chars.iter().collect())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenizer_splits_cjk_and_ascii() {
        let mut tok = CnTokenizer::default();
        let mut stream = tok.token_stream("AD升级方案.docx");
        let mut texts = Vec::new();
        while stream.advance() {
            texts.push(stream.token().text.clone());
        }
        assert!(texts.contains(&"ad".to_string()));
        assert!(texts.iter().any(|t| t.contains("升级")));
    }

    #[test]
    fn trigram_of_query() {
        assert_eq!(trigrams("abcd"), vec!["abc".to_string(), "bcd".to_string()]);
        assert!(trigrams("ab").is_empty());
    }
}
