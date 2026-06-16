import type { ImportJob } from "./types";

export function canImportBook(path: string, busy: string): boolean {
  return !busy && path.trim().length > 0;
}

export function canStartStory(hasBook: boolean, busy: string): boolean {
  return hasBook && !busy;
}

export function importStatusText(job: ImportJob | null): string {
  if (!job) {
    return "";
  }
  if (job.status === "completed") {
    return "索引完成，可以开始穿书";
  }
  if (job.status === "failed") {
    return job.message || "导入失败";
  }
  return job.message || "正在导入小说";
}
