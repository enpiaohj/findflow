use std::path::Path;

use tantivy::collector::TopDocs;
use tantivy::query::{BooleanQuery, BoostQuery, Occur, Query, TermQuery};
use tantivy::schema::{
    IndexRecordOption, NumericOptions, Schema, TextOptions, Value,
};
use tantivy::{Index, IndexReader, IndexWriter, ReloadPolicy, TantivyDocument, Term};

use crate::tokenizer;

pub struct SearchIndex {
    pub fresh: bool,
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
    // py/bi/tri/pya 仅需词频（无短语查询依赖位置），索引体积更小
    let indexing = tantivy::schema::TextFieldIndexing::default()
        .set_tokenizer(tokenizer_name)
        .set_index_option(IndexRecordOption::WithFreqs);
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
        // fid 必须 indexed：delete_term 依赖倒排索引定位文档，否则删除全部静默失效
        let fid = sb.add_u64_field(
            "fid",
            NumericOptions::default().set_stored().set_fast().set_indexed(),
        );
        let name = sb.add_text_field("name", text_opts(true, "cn"));
        let name_bi = sb.add_text_field("name_bi", text_opts(false, "bi"));
        let name_tri = sb.add_text_field("name_tri", text_opts(false, "tri"));
        let py_ngram = sb.add_text_field("py_ngram", text_opts(false, "py"));
        let pya_ngram = sb.add_text_field("pya_ngram", text_opts(false, "py"));
        let content = sb.add_text_field("content", text_opts(false, "cn"));
        let ocr = sb.add_text_field("ocr", text_opts(false, "cn"));
        let schema = sb.build();

        // 格式版本标记：分词方案变更或索引损坏时清空重建（数据以 SQLite 为准，可全量重放）
        let format_path = dir.join(".ff-format");
        let mut fresh = !dir.join("meta.json").exists();
        if dir.join("meta.json").exists()
            && std::fs::read_to_string(&format_path)
                .map(|v| v.trim() != crate::tokenizer::INDEX_FORMAT)
                .unwrap_or(true)
        {
            for entry in std::fs::read_dir(dir).map_err(|e| e.to_string())?.flatten() {
                let _ = std::fs::remove_file(entry.path());
            }
            fresh = true;
            tracing::warn!("索引格式变更，已清空旧索引待自动重建");
        }
        let index = if fresh {
            Index::create_in_dir(dir, schema.clone()).map_err(|e| e.to_string())?
        } else {
            match Index::open_in_dir(dir) {
                Ok(i) => i,
                Err(_) => {
                    tracing::warn!("索引打开失败，重建索引");
                    for entry in std::fs::read_dir(dir).map_err(|e| e.to_string())?.flatten() {
                        let _ = std::fs::remove_file(entry.path());
                    }
                    Index::create_in_dir(dir, schema.clone()).map_err(|e| e.to_string())?
                }
            }
        };
        std::fs::write(&format_path, crate::tokenizer::INDEX_FORMAT).ok();
        tokenizer::register(&index)?;
        // 写入期间降当前线程优先级：批量重放不与界面争抢 CPU
        #[cfg(windows)]
        unsafe {
            use windows::Win32::System::Threading::{
                GetCurrentThread, SetThreadPriority, THREAD_PRIORITY_BELOW_NORMAL,
            };
            let _ = SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_BELOW_NORMAL);
        }
        // 单线程 writer：桌面场景写入吞吐足够，且避免多 worker 的 io 并发问题
        let writer = index
            .writer_with_num_threads(1, 64 * 1024 * 1024)
            .map_err(|e| e.to_string())?;
        #[cfg(windows)]
        unsafe {
            use windows::Win32::System::Threading::{
                GetCurrentThread, SetThreadPriority, THREAD_PRIORITY_NORMAL,
            };
            let _ = SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_NORMAL);
        }
        let reader = index
            .reader_builder()
            .reload_policy(ReloadPolicy::Manual)
            .try_into()
            .map_err(|e| e.to_string())?;
        Ok(Self {
            fresh,
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
        // 仅含 CJK 的名称才需要拼音（纯 ASCII 名称拼音与原名等价，跳过省一半开销）
        if !name.is_ascii() {
            d.add_text(self.py_ngram, crate::util::pinyin_full(name));
            d.add_text(self.pya_ngram, crate::util::pinyin_abbr(name));
        }
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
                    // 全部连续三元组必须命中（等价子串召回），精度由 SQL 侧完整命中加权兜底
                    let tris = tokenizer::trigrams(t);
                    if tris.len() >= 2 {
                        let clauses: Vec<(Occur, Box<dyn Query>)> = tris
                            .iter()
                            .map(|g| {
                                let q: Box<dyn Query> = Box::new(TermQuery::new(
                                    Term::from_field_text(self.name_tri, g),
                                    IndexRecordOption::WithFreqsAndPositions,
                                ));
                                (Occur::Must, q)
                            })
                            .collect();
                        shoulds.push((
                            Occur::Should,
                            Box::new(BoostQuery::new(Box::new(BooleanQuery::new(clauses)), 0.8)),
                        ));
                    }
                }
            }
            if (2..=8).contains(&len) && t.is_ascii() {
                shoulds.push((Occur::Should, term(self.pya_ngram, t, 0.35)));
                shoulds.push((Occur::Should, term(self.py_ngram, t, 0.35)));
            }
            if len >= 2 && search_content {
                shoulds.push((Occur::Should, term(self.content, t, 0.2)));
                shoulds.push((Occur::Should, term(self.ocr, t, 0.2)));
            }
        }
        if let Some(p) = phrase {
            let tris = tokenizer::trigrams(p);
            if tris.len() >= 2 {
                let clauses: Vec<(Occur, Box<dyn Query>)> = tris
                    .iter()
                    .map(|g| {
                        let q: Box<dyn Query> = Box::new(TermQuery::new(
                            Term::from_field_text(self.name_tri, g),
                            IndexRecordOption::WithFreqsAndPositions,
                        ));
                        (Occur::Must, q)
                    })
                    .collect();
                shoulds.push((Occur::Should, Box::new(BooleanQuery::new(clauses))));
            } else if let Some(first) = p.chars().next() {
                let s: String = first.to_string();
                shoulds.push((Occur::Should, term(self.name, &s, 0.5)));
            }
        }
        Box::new(BooleanQuery::new(shoulds))
    }

    /// 检索候选：(file_id, 文档地址)。片段生成直接复用地址，零额外查询。
    pub fn candidate_docs(
        &self,
        query: &dyn Query,
        limit: usize,
    ) -> Result<Vec<(u64, tantivy::DocAddress)>, String> {
        let searcher = self.reader.searcher();
        let top = searcher
            .search(query, &TopDocs::with_limit(limit))
            .map_err(|e| e.to_string())?;
        let mut out = Vec::with_capacity(top.len());
        for (_score, addr) in top {
            let d: TantivyDocument = searcher.doc(addr).map_err(|e| e.to_string())?;
            if let Some(v) = d.get_first(self.fid).and_then(|v| v.as_u64()) {
                out.push((v, addr));
            }
        }
        Ok(out)
    }

    /// 用已知文档地址生成命中片段（无额外查询）。
    pub fn snippet_at(
        &self,
        generator: &tantivy::snippet::SnippetGenerator,
        addr: tantivy::DocAddress,
        _field: tantivy::schema::Field,
    ) -> Option<String> {
        let searcher = self.reader.searcher();
        let d: TantivyDocument = searcher.doc(addr).ok()?;
        let snip = generator.snippet_from_doc(&d);
        if snip.is_empty() {
            None
        } else {
            Some(snip.fragment().to_string())
        }
    }

    /// 为查询创建片段生成器（searcher 需与文档获取一致）。
    pub fn snippet_generator(
        &self,
        query: &dyn Query,
        field: tantivy::schema::Field,
    ) -> Option<tantivy::snippet::SnippetGenerator> {
        let searcher = self.reader.searcher();
        tantivy::snippet::SnippetGenerator::create(&searcher, query, field).ok()
    }

    /// 用文档地址与生成器产出片段。
    pub fn snippet_from_addr(
        &self,
        generator: &tantivy::snippet::SnippetGenerator,
        addr: tantivy::DocAddress,
    ) -> Option<String> {
        let searcher = self.reader.searcher();
        let d: TantivyDocument = searcher.doc(addr).ok()?;
        let snippet = generator.snippet_from_doc(&d);
        let fragment = snippet.fragment();
        if fragment.is_empty() {
            None
        } else {
            Some(fragment.to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delete_then_add_keeps_single_doc_per_fid() {
        let dir = tempfile::tempdir().unwrap();
        let idx = SearchIndex::open_or_create(dir.path()).unwrap();
        idx.add(idx.build_doc(7, "报告初稿.docx", None, None)).unwrap();
        idx.commit().unwrap();
        // 与 outbox 消费一致：同一次提交内先删后加
        idx.delete(7);
        idx.add(idx.build_doc(7, "报告终稿.docx", None, None)).unwrap();
        idx.commit().unwrap();
        idx.reload();
        let q = idx.build_query("报告", None, false);
        let hits = idx.candidate_docs(q.as_ref(), 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].0, 7);
    }
}
