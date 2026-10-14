/** 失焦不是会话结束：只有真正隐藏窗口才应用清空策略。 */
export function shouldClearSession(reason: string): boolean {
  return reason === "manual" || reason === "blur";
}
