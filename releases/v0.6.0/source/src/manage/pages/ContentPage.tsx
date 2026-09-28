import { useEffect, useState } from "react";
import type { PageCtx } from "../ManageBody";
import { diagnosticsHealth, fulltextStart, ocrStart } from "../../api";

/** 文件与内容：索引提取范围（元数据 / 全文 / OCR）。 */
export default function ContentPage({ notice }: PageCtx) {
  const [health, setHealth] = useState<{
    totalFiles: number;
    fulltextDone: number;
    ocrDone: number;
    outboxPending: number;
  } | null>(null);

  useEffect(() => {
    diagnosticsHealth().then(setHealth).catch(() => undefined);
  }, []);

  return (
    <div className="page">
      <h2>文件与内容</h2>
      <p className="page-desc">控制索引哪些内容：文件名元数据、文档全文与图片文字。</p>

      <div className="health-banner">
        <span className="health-ok">✓</span>
        <span>索引正常</span>
        <span className="health-sep" />
        <span>已索引 {health?.totalFiles ?? 0} 个文件</span>
        <span className="health-sep" />
        <span>全文 {health?.fulltextDone ?? 0}</span>
        <span className="health-sep" />
        <span>OCR {health?.ocrDone ?? 0}</span>
      </div>

      <section className="card">
        <div className="card-head">
          <h3>索引内容</h3>
        </div>
        <div className="toggle-row">
          <div>
            <b>文件名与元数据</b>
            <p className="muted">文件名、类型、大小、修改时间等基本信息（始终启用，推荐）</p>
          </div>
          <span className="toggle-on">启用</span>
        </div>
        <div className="toggle-row">
          <div>
            <b>文档全文</b>
            <p className="muted">解析 Office、PDF、文本与代码文件内容，供全文搜索与命中片段</p>
          </div>
          <button
            onClick={() =>
              fulltextStart()
                .then(() => notice("全文提取任务已开始，可在「任务」查看进度"))
                .catch((e) => notice(String(e)))
            }
          >
            启动提取
          </button>
        </div>
        <div className="toggle-row">
          <div>
            <b>图片 OCR</b>
            <p className="muted">识别图片中的文字（使用系统 OCR，需要已安装对应语言包）</p>
          </div>
          <button
            onClick={() =>
              ocrStart().then(() => notice("OCR 任务已开始")).catch((e) => notice(String(e)))
            }
          >
            启动 OCR
          </button>
        </div>
      </section>
    </div>
  );
}
