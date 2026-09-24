// 性能基准数据生成器（设计文档 §15.2）：
//   cargo run --release --bin bench_gen -- --count 1000000
// 数据写入 <repo>/.bench/：bench.db（SQLite）+ index/（Tantivy），目录已 gitignore。
// --query "关键词" --rounds 200 进入查询基准模式，输出 P50/P95/P99。
use std::path::PathBuf;
use std::time::Instant;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut count: usize = 1_000_000;
    let mut query: Option<String> = None;
    let mut rounds: usize = 200;
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--count" => {
                count = args.get(i + 1).and_then(|v| v.parse().ok()).unwrap_or(count);
                i += 1;
            }
            "--query" => {
                query = args.get(i + 1).cloned();
                i += 1;
            }
            "--rounds" => {
                rounds = args.get(i + 1).and_then(|v| v.parse().ok()).unwrap_or(rounds);
                i += 1;
            }
            _ => {}
        }
        i += 1;
    }

    let bench_dir = PathBuf::from("../.bench");
    std::fs::create_dir_all(&bench_dir).ok();
    let db_path = bench_dir.join("bench.db");
    let index_dir = bench_dir.join("index");

    let store = std::sync::Arc::new(findflow_lib::settings::SettingsStore::new(
        findflow_lib::db::open(&db_path).expect("打开 bench.db 失败"),
    ));

    if let Some(q) = query {
        store
            .with_conn(|c| {
                c.execute_batch("CREATE INDEX IF NOT EXISTS idx_files_ext_size ON files(ext, size);")
                    .unwrap();
            });
        let index = findflow_lib::searchindex::SearchIndex::open_or_create(&index_dir)
            .expect("打开索引失败");
        run_query_bench(&store, &index, &q, rounds);
        return;
    }

    store.with_conn(|c| {
        c.execute_batch("CREATE INDEX IF NOT EXISTS idx_files_ext_size ON files(ext, size);")
            .unwrap();
    });
    println!("生成 {count} 条元数据到 {}", db_path.display());
    // 先注册基准范围：files.scope_id 的外键依赖它
    store.with_conn(|c| {
        c.execute(
            "INSERT OR REPLACE INTO scopes(scope_id, path, added_at) VALUES (1, 'D:\\bench', ?1)",
            [findflow_lib::util::now_rfc3339()],
        )
        .unwrap();
    });
    let started = Instant::now();
    let batch: usize = 5000;
    let mut generated = 0usize;
    let words: Vec<&str> = vec![
        "方案", "报告", "清单", "记录", "周报", "计划", "调研", "设计", "测试", "部署", "审计",
        "总结", "规划", "预算", "合同", "图片", "视频", "备份", "归档", "说明",
    ];
    let exts: Vec<&str> = vec![
        "docx", "xlsx", "pdf", "png", "jpg", "mp4", "txt", "md", "zip", "pptx", "csv", "json",
    ];
    let mut rand_state: u64 = 0x9E3779B97F4A7C15;
    let mut rand = move || {
        rand_state ^= rand_state << 13;
        rand_state ^= rand_state >> 7;
        rand_state ^= rand_state << 17;
        rand_state
    };
    while generated < count {
        let mut items = Vec::with_capacity(batch);
        for _ in 0..batch.min(count - generated) {
            let r1 = rand();
            let r2 = rand();
            let w1 = words[(r1 % words.len() as u64) as usize];
            let w2 = words[(r2 % words.len() as u64) as usize];
            let num = r1 % 10_000;
            let ext = exts[(r2 % exts.len() as u64) as usize];
            let kind = findflow_lib::util::kind_from(ext, false);
            let name = format!("{w1}{w2}-{num}.{ext}");
            let path = format!("D:\\bench\\数据{}\\{name}", r2 % 100);
            let size = (r1 % 500_000_000) as i64;
            let modified = 1_700_000_000_000i64 + (r2 % 100_000_000_000) as i64;
            items.push(findflow_lib::scanner::FileUpsert {
                scope_id: 1,
                name: name.clone(),
                path: path.clone(),
                ext: ext.to_string(),
                kind: kind.to_string(),
                size,
                created_ms: modified,
                modified_ms: modified,
                attributes: 32,
                is_dir: false,
                native_serial: Some(123456),
                native_file_id: Some((r1 % i32::MAX as u64) as i64),
                parent_path: format!("D:\\bench\\数据{}", r2 % 100),
            });
        }
        store
            .with_conn(|c| {
                c.execute("BEGIN IMMEDIATE", []).unwrap();
                findflow_lib::scanner::upsert_batch(c, &items).unwrap();
                c.execute("COMMIT", []).unwrap();
            });
        generated += items.len();
        if generated % 100_000 == 0 {
            let elapsed = started.elapsed().as_secs_f64();
            println!("已生成 {generated} 条，速度 {:.0} 条/秒", generated as f64 / elapsed);
        }
    }
    let elapsed = started.elapsed().as_secs_f64();
    println!(
        "SQLite 完成：{count} 条，总耗时 {elapsed:.1}s，平均 {:.0} 条/秒",
        count as f64 / elapsed
    );

    // 构建倒排索引
    println!("构建 Tantivy 索引...");
    let index = findflow_lib::searchindex::SearchIndex::open_or_create(&index_dir).unwrap();
    let idx_started = Instant::now();
    let mut indexed = 0usize;
    loop {
        let batch_rows: Vec<(i64, String)> = store.with_conn(|c| {
            let mut stmt = c
                .prepare("SELECT file_id, name FROM files WHERE file_id > ?1 ORDER BY file_id LIMIT 5000")
                .unwrap();
            let rows = stmt
                .query_map([indexed as i64], |r| Ok((r.get(0)?, r.get(1)?)))
                .unwrap();
            rows.filter_map(|x| x.ok()).collect()
        });
        if batch_rows.is_empty() {
            break;
        }
        for (fid, name) in &batch_rows {
            let doc = index.build_doc(*fid as u64, name, None, None);
            if let Err(e) = index.add(doc) {
                eprintln!("索引写入失败 fid={fid} name={name}: {e}");
                match index.commit() {
                    Err(ce) => eprintln!("commit 错误（根因）: {ce}"),
                    Ok(_) => eprintln!("commit 正常"),
                }
                std::thread::sleep(std::time::Duration::from_millis(200));
                return;
            }
        }
        indexed += batch_rows.len();
        if indexed % 100_000 == 0 {
            println!("已索引 {indexed} 条");
        }
    }
    index.commit().unwrap();
    let idx_elapsed = idx_started.elapsed().as_secs_f64();
    println!(
        "Tantivy 完成：{indexed} 条，耗时 {idx_elapsed:.1}s"
    );
}

fn run_query_bench(
    store: &std::sync::Arc<findflow_lib::settings::SettingsStore>,
    index: &findflow_lib::searchindex::SearchIndex,
    q: &str,
    rounds: usize,
) {
    let ast = findflow_lib::query::parse(q);
    let mut samples: Vec<u128> = Vec::new();
    for _ in 0..rounds {
        let t = Instant::now();
        let outcome = findflow_lib::search::run(store, index, &ast, 20).expect("查询失败");
        samples.push(t.elapsed().as_millis());
        if samples.len() == 1 {
            println!("结果 {} 项（总数 {}）", outcome.items.len(), outcome.total);
        }
    }
    samples.sort();
    let p = |ratio: f64| -> u128 {
        let idx = ((samples.len() as f64 - 1.0) * ratio).round() as usize;
        samples[idx.min(samples.len() - 1)]
    };
    println!(
        "查询 \"{q}\" {} 轮：P50={}ms P95={}ms P99={}ms",
        rounds,
        p(0.50),
        p(0.95),
        p(0.99)
    );
}
