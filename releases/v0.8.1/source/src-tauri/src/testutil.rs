//! 测试辅助：容忍 Windows 杀毒软件造成的索引文件 I/O 噪声。
//!
//! 实测（同一个测试各跑 100 次）：默认临时目录失败约 15 次，Defender 排除目录 0 次——
//! Tantivy 写新建的索引文件时偶发 `PermissionDenied`。产品代码已能在运行时恢复
//! （丢弃失效的写入器、重建并重放，见 `SearchIndex::commit`），测试里则重跑整个用例体。
//! 只对这一类 I/O 错误重试，真正的断言失败照常失败。

use std::panic::{catch_unwind, resume_unwind, RefUnwindSafe};

const ATTEMPTS: usize = 5;

fn is_transient_io(msg: &str) -> bool {
    msg.contains("PermissionDenied")
        || msg.contains("拒绝访问")
        || msg.contains("index writer was killed")
        || msg.contains("worker thread encountered an error")
}

fn payload_text(payload: &(dyn std::any::Any + Send)) -> String {
    if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else if let Some(s) = payload.downcast_ref::<&str>() {
        (*s).to_string()
    } else {
        String::new()
    }
}

/// 运行用例体；若因杀毒软件类瞬时 I/O 错误 panic，则重跑（最多 5 次）。
pub fn retry_on_index_io<F: Fn() + RefUnwindSafe>(body: F) {
    for attempt in 1..=ATTEMPTS {
        match catch_unwind(&body) {
            Ok(()) => return,
            Err(payload) => {
                if attempt == ATTEMPTS || !is_transient_io(&payload_text(payload.as_ref())) {
                    resume_unwind(payload);
                }
                eprintln!("测试遇到瞬时索引 I/O 错误（杀毒软件占用），第 {attempt} 次重试");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retries_transient_io_but_not_assertion_failures() {
        let tries = std::sync::atomic::AtomicUsize::new(0);
        retry_on_index_io(|| {
            let n = tries.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            if n < 2 {
                panic!("Failed to open file for write: PermissionDenied");
            }
        });
        assert_eq!(tries.load(std::sync::atomic::Ordering::SeqCst), 3);

        let result = catch_unwind(|| {
            retry_on_index_io(|| {
                assert_eq!(1, 2, "真正的断言失败不应被吞掉");
            })
        });
        assert!(result.is_err());
    }
}
