import { useState } from "react";

interface ConfirmState {
  message: string;
  sub?: string;
  resolve: (ok: boolean) => void;
}

/** 应用内确认框：替代原生 window.confirm，风格与整体一致。 */
export function useConfirm() {
  const [state, setState] = useState<ConfirmState | null>(null);

  const confirm = (message: string, sub?: string) =>
    new Promise<boolean>((resolve) => setState({ message, sub, resolve }));

  const confirmEl = state ? (
    <div className="menu-overlay">
      <div className="confirm-panel">
        <p className="confirm-msg">{state.message}</p>
        {state.sub ? <p className="muted">{state.sub}</p> : null}
        <div className="trial-actions" style={{ justifyContent: "flex-end" }}>
          <button
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
