import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { updaterCheck, updaterInstall } from "../../api";
import { useConfirm } from "../../components/ConfirmDialog";
import { progressRatio, progressText, type UpdateProgress } from "../../updateProgress";

type Phase =
  | { kind: "idle" }
  | { kind: "checking" }
  | { kind: "latest" }
  | { kind: "available"; version: string; notes: string | null }
  | { kind: "installing"; version: string; progress: UpdateProgress | null }
  | { kind: "error"; message: string };

interface Props {
  /** 当前版本号（不含 v 前缀）；读取中为空串 */
  version: string;
  notice: (message: string) => void;
}

/**
 * 软件更新卡片（「关于」页）。只在用户点击时联网检查，不做后台自动检查——
 * 「基础功能不依赖网络」是产品底线，更新检查是用户明确发起的例外。
 */
export default function UpdateCard({ version, notice }: Props) {
  const [phase, setPhase] = useState<Phase>({ kind: "idle" });
  const { confirm, confirmEl } = useConfirm();

  // 下载进度由后端事件推送
  useEffect(() => {
    const unlistenP = listen<UpdateProgress>("updater-progress", (event) => {
      setPhase((current) =>
        current.kind === "installing" ? { ...current, progress: event.payload } : current,
      );
    });
    return () => void unlistenP.then((off) => off());
  }, []);

  const check = () => {
    setPhase({ kind: "checking" });
    updaterCheck()
      .then((r) =>
        setPhase(
          r.available && r.version
            ? { kind: "available", version: r.version, notes: r.notes ?? null }
            : { kind: "latest" },
        ),
      )
      .catch((e) => setPhase({ kind: "error", message: String(e) }));
  };

  const install = async (target: string) => {
    const ok = await confirm(
      `下载并安装 FindFlow v${target}？`,
      "下载完成后 FindFlow 会自动关闭并启动安装程序，安装结束后重新打开。索引与设置会保留。",
    );
    if (!ok) return;
    setPhase({ kind: "installing", version: target, progress: null });
    updaterInstall()
      // 正常情况下安装程序启动后本进程即退出，走不到这里
      .then(() => notice("更新已安装，请重新启动 FindFlow"))
      .catch((e) => setPhase({ kind: "error", message: String(e) }));
  };

  const busy = phase.kind === "checking" || phase.kind === "installing";
  const ratio = phase.kind === "installing" && phase.progress ? progressRatio(phase.progress) : null;

  return (
    <section className="card">
      {confirmEl}
      <div className="card-head">
        <h3>软件更新</h3>
      </div>
      <div className="toggle-row">
        <div>
          <b>
            {phase.kind === "available"
              ? `发现新版本 v${phase.version}`
              : phase.kind === "installing"
                ? `正在更新到 v${phase.version}`
                : phase.kind === "latest"
                  ? "当前已是最新版本"
                  : phase.kind === "checking"
                    ? "正在检查更新…"
                    : phase.kind === "error"
                      ? "检查或安装更新失败"
                      : "检查是否有新版本"}
          </b>
          <p className="muted">
            {phase.kind === "error"
              ? phase.message
              : `当前版本 ${version ? `v${version}` : "读取中…"} · 仅在点击时访问 GitHub 读取更新清单，不发送本机数据；更新包经签名校验`}
          </p>
        </div>
        {phase.kind === "available" ? (
          <button className="btn-primary" onClick={() => void install(phase.version)}>
            下载并安装
          </button>
        ) : (
          <button disabled={busy} onClick={check}>
            {phase.kind === "checking" ? "检查中…" : "检查更新"}
          </button>
        )}
      </div>
      {phase.kind === "available" && phase.notes ? <p className="update-notes">{phase.notes}</p> : null}
      {phase.kind === "installing" ? (
        <div className="progress-line update-progress">
          <div className={"progress-bar" + (ratio === null ? " progress-indeterminate" : "")}>
            <div style={ratio === null ? undefined : { width: `${Math.round(ratio * 100)}%` }} />
          </div>
          <span>{phase.progress ? progressText(phase.progress) : "正在连接下载服务器…"}</span>
        </div>
      ) : null}
    </section>
  );
}
