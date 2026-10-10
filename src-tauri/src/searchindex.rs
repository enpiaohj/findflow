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
    /// `None` 表示写入器已失效、待重建（见 `add` / `commit` 的错误处理）。
    pub writer: std::sync::Mutex<Option<IndexWriter>>,
    /// 写入器因 I/O 错误被丢弃过：其内存队列里「已写入未提交」的文档已丢失，需要由 outbox 重放。
    writer_reset: std::sync::atomic::AtomicBool,
    pub reader: IndexReader,
    pub fid: tantivy::schema::Field,
    pub name: tantivy::schema::Field,
    pub name_uni: tantivy::schema::Field,
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
        // 单字索引：n-gram 最小粒度若是 2，单个汉字就永远命不中任何词项，
        // 用户搜「报」会得到 0 条。单字查询召回量本身很大，靠候选上限 + 相关度排序收敛。
        let name_uni = sb.add_text_field("name_uni", text_opts(false, "uni"));
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
            writer: std::sync::Mutex::new(Some(writer)),
            writer_reset: std::sync::atomic::AtomicBool::new(false),
            reader,
            fid,
            name,
            name_uni,
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
        // 单字：整串交给 uni 分词器按字符切分
        d.add_text(self.name_uni, lower.as_str());
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

    /// 取得可用的写入器；失效后首次调用时重建。
    /// Tantivy 的写入线程一旦遇到 I/O 错误（典型：杀毒软件瞬时占用新建的段文件，
    /// `PermissionDenied`）整个 `IndexWriter` 就被「杀死」，之后所有写入与提交都会失败，
    /// 只能丢弃并重建——此前只有重启应用才会恢复，期间新文件一直搜不到。
    fn ensure_writer<'a>(&self, slot: &'a mut Option<IndexWriter>) -> Result<&'a mut IndexWriter, String> {
        if slot.is_none() {
            // 旧写入器已在丢弃时释放了索引目录锁，可以重建
            *slot = Some(
                self.index
                    .writer_with_num_threads(1, 64 * 1024 * 1024)
                    .map_err(|e| e.to_string())?,
            );
            tracing::warn!("索引写入器已重建");
        }
        Ok(slot.as_mut().expect("writer just ensured"))
    }

    /// 丢弃失效的写入器，并标记需要重放「已写入未提交」的事件。
    pub(crate) fn discard_writer(&self, slot: &mut Option<IndexWriter>) {
        *slot = None;
        self.writer_reset
            .store(true, std::sync::atomic::Ordering::SeqCst);
    }

    /// 写入器是否被丢弃过（读取即清除）。为真时，丢弃前写入的文档已丢失，调用方应把对应事件重放。
    pub fn take_writer_reset(&self) -> bool {
        self.writer_reset
            .swap(false, std::sync::atomic::Ordering::SeqCst)
    }

    pub fn add(&self, document: TantivyDocument) -> Result<(), String> {
        let mut slot = self.writer.lock().unwrap();
        let result = self.ensure_writer(&mut slot)?.add_document(document);
        match result {
            Ok(_) => Ok(()),
            Err(e) => {
                if matches!(e, tantivy::TantivyError::ErrorInThread(_)) {
                    self.discard_writer(&mut slot);
                }
                Err(e.to_string())
            }
        }
    }

    pub fn delete(&self, fid: u64) {
        let mut slot = self.writer.lock().unwrap();
        if let Ok(w) = self.ensure_writer(&mut slot) {
            w.delete_term(Term::from_field_u64(self.fid, fid));
        }
    }

    pub fn commit(&self) -> Result<(), String> {
        let mut slot = self.writer.lock().unwrap();
        let result = self.ensure_writer(&mut slot)?.commit();
        match result {
            Ok(_) => Ok(()),
            Err(e) => {
                // 提交失败后写入器状态不可信（线程已被杀死，或半途失败），一律重建
                self.discard_writer(&mut slot);
                Err(e.to_string())
            }
        }
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
        // 多个关键词是「且」：每个词自成一组（组内是名称 / 拼音 / 正文等多种命中方式，满足其一即可），
        // 各组之间必须同时满足。此前所有子句平铺为「或」，「报告 2026」会把只含其一的文件都召回，
        // 候选名额被无关项挤占，真正同时含两词的文件反而可能被截掉。
        let mut musts: Vec<(Occur, Box<dyn Query>)> = Vec::new();
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
            let mut shoulds: Vec<(Occur, Box<dyn Query>)> = Vec::new();
            let len = t.chars().count();
            match len {
                1 => shoulds.push((Occur::Should, term(self.name_uni, t, 1.0))),
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
                // 正文按 jieba 切词建索引，查询词也要切成同样的词项并要求全部命中；
                // 把整串当作一个词项只能命中恰好是词典词的查询（「年度项目管理报告」永远命不中）。
                for field in [self.content, self.ocr] {
                    if let Some(q) = self.cn_all_terms(field, t, 0.2) {
                        shoulds.push((Occur::Should, q));
                    }
                }
            }
            if !shoulds.is_empty() {
                musts.push((Occur::Must, Box::new(BooleanQuery::new(shoulds))));
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
                musts.push((Occur::Must, Box::new(BooleanQuery::new(clauses))));
            } else if let Some(first) = p.chars().next() {
                let s: String = first.to_string();
                musts.push((Occur::Must, term(self.name, &s, 0.5)));
            }
        }
        Box::new(BooleanQuery::new(musts))
    }

    /// 把查询词按 `cn` 分词器切成词项，要求该字段全部命中（且）；切不出词项返回 None。
    fn cn_all_terms(&self, field: tantivy::schema::Field, text: &str, boost: f32) -> Option<Box<dyn Query>> {
        use tantivy::tokenizer::TokenStream;
        let mut analyzer = self.index.tokenizers().get("cn")?;
        let mut stream = analyzer.token_stream(text);
        let mut seen = std::collections::HashSet::new();
        let mut clauses: Vec<(Occur, Box<dyn Query>)> = Vec::new();
        while stream.advance() {
            let tok = stream.token().text.clone();
            if tok.trim().is_empty() || !seen.insert(tok.clone()) {
                continue;
            }
            let q: Box<dyn Query> = Box::new(TermQuery::new(
                Term::from_field_text(field, &tok),
                IndexRecordOption::WithFreqsAndPositions,
            ));
            clauses.push((Occur::Must, q));
        }
        if clauses.is_empty() {
            return None;
        }
        Some(Box::new(BoostQuery::new(Box::new(BooleanQuery::new(clauses)), boost)))
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

    /// 单个汉字必须能作为子串命中文件名。
    /// n-gram 索引此前最小粒度是 2（bi），单字无法命中任何词项，
    /// 于是搜「报」得到 0 条——用户看到的就是「搜一个字没反应」。
    #[test]
    fn single_char_query_matches_name_substring() {
        crate::testutil::retry_on_index_io(|| {
            let dir = tempfile::tempdir().unwrap();
            let idx = SearchIndex::open_or_create(dir.path()).unwrap();
            idx.add(idx.build_doc(1, "报告初稿.docx", None, None)).unwrap();
            idx.add(idx.build_doc(2, "预算表.xlsx", None, None)).unwrap();
            idx.commit().unwrap();
            idx.reload();

            let ids = |q: &str| -> Vec<i64> {
                let query = idx.build_query(q, None, false);
                let mut v: Vec<i64> = idx
                    .candidate_docs(query.as_ref(), 10)
                    .unwrap()
                    .iter()
                    .map(|(id, _)| *id as i64)
                    .collect();
                v.sort();
                v
            };
            // 首字、次字、末字（含扩展名内的字符）都应能命中
            assert_eq!(ids("报"), vec![1], "首字应命中");
            assert_eq!(ids("稿"), vec![1], "中间字应命中");
            assert_eq!(ids("表"), vec![2], "末字应命中");
            // 无关单字不得命中
            assert!(ids("龙").is_empty());
        });
    }

    #[test]
    fn delete_then_add_keeps_single_doc_per_fid() {
        crate::testutil::retry_on_index_io(|| {
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
        });
    }

    /// 写入器因 I/O 错误被丢弃后，下一次写入自动重建，之后的提交正常；
    /// 丢弃前「已写入未提交」的文档确实丢失，并通过标志告知调用方重放。
    #[test]
    fn writer_is_rebuilt_after_being_discarded() {
        crate::testutil::retry_on_index_io(|| {
            let dir = tempfile::tempdir().unwrap();
            let idx = SearchIndex::open_or_create(dir.path()).unwrap();
            idx.add(idx.build_doc(1, "甲方案.docx", None, None)).unwrap();
            idx.commit().unwrap();
            idx.add(idx.build_doc(2, "乙方案.docx", None, None)).unwrap(); // 未提交
            {
                let mut slot = idx.writer.lock().unwrap();
                idx.discard_writer(&mut slot);
            }
            assert!(idx.take_writer_reset(), "应标记写入器被丢弃");
            assert!(!idx.take_writer_reset(), "标志读取即清除");
            idx.add(idx.build_doc(3, "丙方案.docx", None, None)).unwrap(); // 自动重建
            idx.commit().unwrap();
            idx.reload();
            let q = idx.build_query("方案", None, false);
            let mut ids: Vec<u64> = idx
                .candidate_docs(q.as_ref(), 10)
                .unwrap()
                .iter()
                .map(|h| h.0)
                .collect();
            ids.sort_unstable();
            assert_eq!(ids, vec![1, 3], "丢弃前未提交的文档 2 丢失，其余正常");
        });
    }
}
