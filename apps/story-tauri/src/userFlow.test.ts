import { describe, expect, it } from "vitest";
import { canImportBook, canStartStory, importStatusText } from "./userFlow";

describe("first-time user flow guards", () => {
  it("does not allow empty import paths", () => {
    expect(canImportBook("", "")).toBe(false);
    expect(canImportBook("   ", "")).toBe(false);
    expect(canImportBook("/Books/long.txt", "")).toBe(true);
  });

  it("does not allow starting before a book is ready", () => {
    expect(canStartStory(false, "")).toBe(false);
    expect(canStartStory(true, "正在导入小说")).toBe(false);
    expect(canStartStory(true, "")).toBe(true);
  });

  it("keeps import status text friendly", () => {
    expect(
      importStatusText({
        id: "job",
        status: "completed",
        progress: 1,
        message: "",
        bookId: "book",
        createdAt: new Date().toISOString(),
      }),
    ).toBe("索引完成，可以开始穿书");
  });
});
