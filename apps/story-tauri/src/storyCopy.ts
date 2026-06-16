import type { PlayerActionKind } from "./types";

export function actionLabel(kind: PlayerActionKind): string {
  switch (kind) {
    case "speak":
      return "说一句";
    case "act":
      return "做动作";
    case "continue":
      return "继续剧情";
  }
}

export function actionPlaceholder(kind: PlayerActionKind): string {
  switch (kind) {
    case "speak":
      return "输入你想对角色说的话";
    case "act":
      return "输入你想做的动作";
    case "continue":
      return "不用输入，直接推进下一段剧情";
  }
}

export function defaultActionText(kind: PlayerActionKind): string {
  switch (kind) {
    case "speak":
      return "我压低声音说出自己的判断。";
    case "act":
      return "我先观察周围，再做一个谨慎的动作。";
    case "continue":
      return "我保持当前选择，继续跟着剧情往前走。";
  }
}

export function formatCount(value: number, unit: string): string {
  if (value >= 10_000) {
    return `${(value / 10_000).toFixed(1)}万${unit}`;
  }
  return `${value}${unit}`;
}
