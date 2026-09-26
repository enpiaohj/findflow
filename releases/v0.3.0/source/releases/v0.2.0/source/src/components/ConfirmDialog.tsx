import { useEffect, useState } from "react";

interface ConfirmState {
  message: string;
  sub?: string;
  resolve: (ok: boolean) => void;
}

/** 应用内确认框：替代原生 window.confirm，风格与整体一致。 */
export function useConfirm() {
  const [state, setState] = useState<ConfirmState | null>(null);

  useEffect(() => {
    if (!state) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        state.resolve(false);
        setState(null);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [state]);

  const confirm = (message: string, sub?: string) =>
    new Promise<boolean>((resolve) => setState({ message, sub, resolve }));

  const confirmEl = state ? (
    <div
    className="menu-overlay"
    style={{
      position: "fixed",
      inset: 0,
      zIndex: 80,
      background: "rgba(17,24,39,0.32)",
    }}
  >
      <div
      className="confirm-panel"
      style={{
        position: "fixed",
        top: "50%",
        left: "50%",
        transform: "translate(-50%, -50%)",
        width: "min(420px, 80vw)",
        background: "#ffffff",
        border: "1px solid #e5e7eb",
        borderRadius: 12,
        boxShadow: "0 16px 48px rgba(0,0,0,0.22)",
        padding: 18,
        zIndex: 90,
      }}
    >
        <p className="confirm-msg">{state.message}</p>
        {state.sub ? <p className="muted">{state.sub}</p> : null}
        <div className="trial-actions" style={{ justifyContent: "flex-end" }}>
          <button
            type="button"
            onClick={() => {
              state.resolve(false);
              setState(null);
            }}
          >
            取消
          </button>
          <button
            className="btn-primary"
            onClick={() => {
              state.resolve(true);
              setState(null);
            }}
          >
            确定
          </button>
        </div>
      </div>
    </div>
  ) : null;

  return { confirm, confirmEl };
}
