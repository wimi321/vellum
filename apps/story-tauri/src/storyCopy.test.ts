import { describe, expect, it } from "vitest";
import { actionLabel, actionPlaceholder, defaultActionText, formatCount } from "./storyCopy";

describe("story copy helpers", () => {
  it("keeps the three core actions simple", () => {
    expect(actionLabel("speak")).toBe("说一句");
    expect(actionLabel("act")).toBe("做动作");
    expect(actionLabel("continue")).toBe("继续剧情");
  });

  it("uses action-specific placeholder text", () => {
    expect(actionPlaceholder("speak")).toContain("说的话");
    expect(defaultActionText("continue")).toContain("继续");
  });

  it("formats large novel counts for normal readers", () => {
    expect(formatCount(120_000, "字")).toBe("12.0万字");
    expect(formatCount(42, "章")).toBe("42章");
  });
});
