import { useEffect, useState } from "react";
import type { PageCtx } from "../ManageBody";
import { getSetting, setSetting, scopesList, type ScopeInfo } from "../../api";
import {
  DEFAULT_START_VIEW,
  HIDE_BROWSE_ON_EMPTY_KEY,
  LEGACY_START_LOCATION_KEY,
  START_VIEW_KEY,
  resolveStartView,
  type StartViewSetting,
} from "../../startView";

/** 搜索：空搜索行为与搜索历史。 */
export default function SearchSettingsPage({ notice }: PageCtx) {
  const [scopes, setScopes] = useState<ScopeInfo[]>([]);
  const [startView, setStartViewState] = useState<StartViewSetting>({
    ...DEFAULT_START_VIEW,
  });
  const [startViewLoaded, setStartViewLoaded] = useState(false);
  // 空搜索时是否隐藏启动列表；与启动列表设置同一次 Promise.all 读取，故共用 startViewLoaded
  const [hideOnEmpty, setHideOnEmpty] = useState(false);
  const [historyEnabled, setHistoryEnabledState] = useState(true);

  useEffect(() => {
    let cancelled = false;

    Promise.all([
      scopesList(),
      getSetting<unknown>(START_VIEW_KEY),
      getSetting<unknown>(LEGACY_START_LOCATION_KEY),
      getSetting<boolean>(HIDE_BROWSE_ON_EMPTY_KEY),
    ])
      .then(([nextScopes, currentValue, legacyValue, hideValue]) => {
        if (cancelled) return;
        const resolved = resolveStartView(currentValue, legacyValue, nextScopes);
        setScopes(nextScopes);
        setStartViewState(resolved.setting);
        setHideOnEmpty(hideValue !== null ? !!hideValue : false);
        setStartViewLoaded(true);
        if (resolved.needsPersist) {
          setSetting(START_VIEW_KEY, resolved.setting).catch((error) =>
            notice("启动列表设置迁移失败：" + String(error)),
          );
        }
      })
      .catch((error) => {
        if (!cancelled) notice("读取启动列表设置失败：" + String(error));
      });

    getSetting<boolean>("ui.searchHistoryEnabled")
      .then((v) => setHistoryEnabledState(v !== null ? !!v : true))
      .catch(() => undefined);

    return () => {
      cancelled = true;
    };
  }, [notice]);

  useEffect(() => {
    if (!startViewLoaded) return;
    const resolved = resolveStartView(startView, null, scopes);
    if (!resolved.needsPersist) return;

    setStartViewState(resolved.setting);
    setSetting(START_VIEW_KEY, resolved.setting).catch((error) =>
      notice("默认索引位置更新失败：" + String(error)),
    );
  }, [notice, scopes, startView, startViewLoaded]);

  const saveStartView = async (next: StartViewSetting) => {
    try {
      await setSetting(START_VIEW_KEY, next);
      setStartViewState(next);
    } catch (error) {
      notice("保存启动列表设置失败：" + String(error));
    }
  };

  const enabledScopes = scopes.filter((scope) => scope.enabled);
  // 渲染期直接派生显示用的启动列表设置：读取完成前不渲染，位置失效时无需等回退 effect 提交，
  // 避免「索引内容」按钮闪现与下拉短暂空白（显示与数据始终一致）
  const resolvedStartView = startViewLoaded
    ? resolveStartView(startView, null, scopes).setting
    : null;

  return (
    <div className="page">
      <h2>搜索</h2>
      <p className="page-desc">搜索框为空时的行为与搜索历史。</p>

      <section className="card">
        <div className="card-head">
          <h3>启动列表</h3>
        </div>
        {resolvedStartView ? (
          <>
            <div className="toggle-row">
              <div>
                <b>打开 FindFlow 时显示</b>
                <p className="muted">仅控制搜索框为空时的列表；输入关键词后始终搜索全部索引位置</p>
              </div>
              <div className="segmented">
                <button
                  className={resolvedStartView.mode === "index" ? "active" : ""}
                  onClick={() =>
                    void saveStartView(
                      resolveStartView({ mode: "index", scopeId: resolvedStartView.scopeId }, null, scopes)
                        .setting,
                    )
                  }
                >
                  索引内容
                </button>
                <button
                  className={resolvedStartView.mode === "large" ? "active" : ""}
                  onClick={() => void saveStartView({ mode: "large", scopeId: null })}
                >
                  大文件
                </button>
              </div>
            </div>
            {resolvedStartView.mode === "index" ? (
              <div className="toggle-row">
                <div>
                  <b>默认索引位置</b>
                  <p className="muted">空搜索时列出该位置根目录下的文件和文件夹，并按名称排序</p>
                </div>
                <select
                  value={resolvedStartView.scopeId ?? ""}
                  onChange={(event) =>
                    void saveStartView({
                      mode: "index",
                      scopeId: Number(event.target.value),
                    })
                  }
                  disabled={enabledScopes.length === 0}
                >
                  {enabledScopes.length === 0 ? <option value="">暂无启用的索引位置</option> : null}
                  {enabledScopes.map((scope) => (
                    <option key={scope.scopeId} value={scope.scopeId}>
                      {scope.path}
                    </option>
                  ))}
                </select>
              </div>
            ) : (
              <div className="toggle-row">
                <div>
                  <b>大文件范围</b>
                  <p className="muted">跨全部索引位置，并按文件大小降序显示</p>
                </div>
                <span className="toggle-on">全部位置</span>
              </div>
            )}
            <div className="toggle-row">
              <div>
                <b>空搜索时隐藏结果</b>
                <p className="muted">
                  开启后搜索框为空时不显示启动列表，界面保持空白；输入关键词后才显示结果
                </p>
              </div>
              <input
                type="checkbox"
                checked={hideOnEmpty}
                onChange={(e) => {
                  setHideOnEmpty(e.target.checked);
                  setSetting(HIDE_BROWSE_ON_EMPTY_KEY, e.target.checked).catch((error) =>
                    notice("保存空搜索隐藏设置失败：" + String(error)),
                  );
                }}
              />
            </div>
          </>
        ) : (
          <div className="toggle-row">
            <div>
              <b>打开 FindFlow 时显示</b>
              <p className="muted">正在读取启动列表设置…</p>
            </div>
          </div>
        )}
      </section>

      <section className="card">
        <div className="card-head">
          <h3>搜索历史</h3>
        </div>
        <div className="toggle-row">
          <div>
            <b>保留搜索历史</b>
            <p className="muted">搜索词仅保存在本机，用于搜索框的历史列表；关闭后不再记录，已有记录保留</p>
          </div>
          <input
            type="checkbox"
            checked={historyEnabled}
            onChange={(e) => {
              setHistoryEnabledState(e.target.checked);
              setSetting("ui.searchHistoryEnabled", e.target.checked).catch((err) =>
                notice(String(err))
              );
            }}
          />
        </div>
        <div className="toggle-row">
          <div>
            <b>清空搜索历史</b>
            <p className="muted">删除全部已保存的搜索词，不可恢复</p>
          </div>
          <button
            className="btn-danger-text"
            onClick={() =>
              setSetting("ui.searchHistory", [])
                .then(() => notice("已清空搜索历史"))
                .catch((err) => notice(String(err)))
            }
          >
            清空
          </button>
        </div>
      </section>
    </div>
  );
}
