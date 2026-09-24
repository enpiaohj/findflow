use std::path::Path;

use tantivy::collector::TopDocs;
use tantivy::query::{BooleanQuery, BoostQuery, Occur, PhraseQuery, Query, TermQuery};
use tantivy::schema::{
    IndexRecordOption, NumericOptions, Schema, TextOptions, Value,
};
use tantivy::{Index, IndexReader, IndexWriter, ReloadPolicy, TantivyDocument, Term};

use crate::tokenizer;

pub struct SearchIndex {
    pub index: Index,
    pub writer: std::sync::Mutex<IndexWriter>,
    pub reader: IndexReader,
    pub fid: tantivy::schema::Field,
    pub name: tantivy::schema::Field,
    pub name_bi: tantivy::schema::Field,
    pub name_tri: tantivy::schema::Field,
    pub py_ngram: tantivy::schema::Field,
    pub pya_ngram: tantivy::schema::Field,
    pub content: tantivy::schema::Field,
    pub ocr: tantivy::schema::Field,
}

fn text_opts(stored: bool, tokenizer_name: &str) -> TextOptions {
    let indexing = tantivy::schema::TextFieldIndexing::default()
        .set_tokenizer(tokenizer_name)
        .set_index_option(IndexRecordOption::WithFreqsAndPositions);
    let opts = TextOptions::default().set_indexing_options(indexing);
    if stored {
        opts.set_stored()
    } else {
        opts
    }
}

impl SearchIndex {
    pub fn open_or_create(dir: &Path) -> Result<Self, String> {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        let mut sb = Schema::builder();
        let fid = sb.add_u64_field("fid", NumericOptions::default().set_stored().set_fast());
        let name = sb.add_text_field("name", text_opts(true, "cn"));
        let name_bi = sb.add_text_field("name_bi", text_opts(false, "bi"));
        let name_tri = sb.add_text_field("name_tri", text_opts(false, "tri"));
        let py_ngram = sb.add_text_field("py_ngram", text_opts(false, "py"));
        let pya_ngram = sb.add_text_field("pya_ngram", text_opts(false, "py"));
        let content = sb.add_text_field("content", text_opts(false, "cn"));
        let ocr = sb.add_text_field("ocr", text_opts(false, "cn"));
        let schema = sb.build();

        let index = if dir.join("meta.json").exists() {
            Index::open_in_dir(dir).map_err(|e| e.to_string())?
        } else {
            Index::create_in_dir(dir, schema.clone()).map_err(|e| e.to_string())?
        };
        tokenizer::register(&index)?;
        let writer = index.writer(64 * 1024 * 1024).map_err(|e| e.to_string())?;
        let reader = index
            .reader_builder()
            .reload_policy(ReloadPolicy::Manual)
            .try_into()
            .map_err(|e| e.to_string())?;
        Ok(Self {
            index,
            writer: std::sync::Mutex::new(writer),
            reader,
            fid,
            name,
            name_bi,
            name_tri,
            py_ngram,
            pya_ngram,
            content,
            ocr,
        })
    }

    pub fn build_doc(
        &self,
        fid: u64,
        name: &str,
        content: Option<&str>,
        ocr: Option<&str>,
    ) -> TantivyDocument {
        let mut d = TantivyDocument::default();
        d.add_u64(self.fid, fid);
        d.add_text(self.name, name);
        let lower = name.to_lowercase();
        let chars: Vec<char> = lower.chars().collect();
        for w in chars.windows(2) {
            d.add_text(self.name_bi, w.iter().collect::<String>());
        }
        for w in chars.windows(3) {
            d.add_text(self.name_tri, w.iter().collect::<String>());
        }
        d.add_text(self.py_ngram, crate::util::pinyin_full(name));
        d.add_text(self.pya_ngram, crate::util::pinyin_abbr(name));
        if let Some(c) = content {
            if !c.is_empty() {
                d.add_text(self.content, c);
            }
        }
        if let Some(o) = ocr {
            if !o.is_empty() {
                d.add_text(self.ocr, o);
            }
        }
        d
    }

    pub fn add(&self, document: TantivyDocument) -> Result<(), String> {
        self.writer
            .lock()
            .unwrap()
            .add_document(document)
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    pub fn delete(&self, fid: u64) {
        self.writer
            .lock()
            .unwrap()
            .delete_term(Term::from_field_u64(self.fid, fid));
    }

    pub fn commit(&self) -> Result<(), String> {
        self.writer
            .lock()
            .unwrap()
            .commit()
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    pub fn reload(&self) {
        self.reader.reload().ok();
    }

    /// 组装文件名/拼音/子串/全文检索的复合查询。
    pub fn build_query(
        &self,
        text: &str,
        phrase: Option<&str>,
        search_content: bool,
    ) -> Box<dyn Query> {
        let mut shoulds: Vec<(Occur, Box<dyn Query>)> = Vec::new();
        let term =
            |field: tantivy::schema::Field, v: &str, boost: f32| -> Box<dyn Query> {
                Box::new(BoostQuery::new(
                    Box::new(TermQuery::new(
                        Term::from_field_text(field, &v.to_lowercase()),
                        IndexRecordOption::WithFreqsAndPositions,
                    )),
                    boost,
                ))
            };
        for t in text.split_whitespace() {
            let len = t.chars().count();
            match len {
                1 => shoulds.push((Occur::Should, term(self.name, t, 1.0))),
                2 => {
                    if let Some(b) = tokenizer::bigram(t) {
                        shoulds.push((Occur::Should, term(self.name_bi, &b, 0.6)));
                    }
                }
                3 => {
                    if let Some(one) = tokenizer::trigrams(t).first() {
                        shoulds.push((Occur::Should, term(self.name_tri, one, 0.7)));
                    }
                }
                _ => {
                    let tris = tokenizer::trigrams(t);
                    if tris.len() >= 2 {
                        let terms: Vec<Term> = tris
                            .iter()
                            .map(|g| Term::from_field_text(self.name_tri, g))
                            .collect();
                        shoulds.push((
                            Occur::Should,
                            Box::new(BoostQuery::new(Box::new(PhraseQuery::new(terms)), 0.8)),
                        ));
                    }
                }
            }
            if len >= 2 && len <= 8 && t.is_ascii() {
                shoulds.push((Occur::Should, term(self.pya_ngram, t, 0.35)));
                shoulds.push((Occur::Should, term(self.py_ngram, t, 0.35)));
            }
            if len >= 2 && search_content {
                shoulds.push((Occur::Should, term(self.content, t, 0.2)));
                shoulds.push((Occur::Should, term(self.ocr, t, 0.2)));
            }
        }
        if let Some(p) = phrase {
            let terms: Vec<Term> = tokenizer::trigrams(p)
                .iter()
                .map(|g| Term::from_field_text(self.name_tri, g))
                .collect();
            if terms.len() >= 2 {
                shoulds.push((Occur::Should, Box::new(PhraseQuery::new(terms))));
            } else if let Some(first) = p.chars().next() {
                let s: String = first.to_string();
                shoulds.push((Occur::Should, term(self.name, &s, 0.5)));
            }
        }
        Box::new(BooleanQuery::new(shoulds))
    }

    /// 检索候选 file_id 列表。
    pub fn candidate_ids(&self, query: &dyn Query, limit: usize) -> Result<Vec<u64>, String> {
        let searcher = self.reader.searcher();
        let top = searcher
            .search(query, &TopDocs::with_limit(limit))
            .map_err(|e| e.to_string())?;
        let mut ids = Vec::with_capacity(top.len());
        for (_score, addr) in top {
            let d: TantivyDocument = searcher.doc(addr).map_err(|e| e.to_string())?;
            if let Some(v) = d.get_first(self.fid).and_then(|v| v.as_u64()) {
                ids.push(v);
            }
        }
        Ok(ids)
    }

    /// 生成命中片段（FR06）。
    pub fn snippet(
        &self,
        query: &dyn Query,
        fid: u64,
        field: tantivy::schema::Field,
    ) -> Option<String> {
        let searcher = self.reader.searcher();
        let generator =
            tantivy::snippet::SnippetGenerator::create(&searcher, query, field).ok()?;
        let term = Term::from_field_u64(self.fid, fid);
        let q = TermQuery::new(term, IndexRecordOption::Basic);
        let top = searcher.search(&q, &TopDocs::with_limit(1)).ok()?;
        let (_score, addr) = top.first()?.clone();
        let d: TantivyDocument = searcher.doc(addr).ok()?;
        Some(generator.snippet_from_doc(&d).fragment().to_string())
    }
}
