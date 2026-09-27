import { currentSession } from "./auth";

export class ExportRequestError extends Error {
  constructor(
    message: string,
    public readonly status: number,
  ) {
    super(message);
  }
}

export async function downloadPrivateExport(): Promise<string> {
  const response = await fetch("/api/export", {
    credentials: "same-origin",
    cache: "no-store",
  });
  if (!response.ok) {
    if (response.status === 401) currentSession.value = null;
    const error = (await response.json().catch(() => null)) as {
      message?: string;
    } | null;
    throw new ExportRequestError(
      error?.message ?? "导出失败，请稍后重试",
      response.status,
    );
  }
  const filename =
    response.headers
      .get("content-disposition")
      ?.match(/filename="([^"]+)"/)?.[1] ?? "mindfolio-export.json";
  const file = await response.blob();
  const url = URL.createObjectURL(file);
  const link = document.createElement("a");
  link.href = url;
  link.download = filename;
  document.body.append(link);
  link.click();
  link.remove();
  window.setTimeout(() => URL.revokeObjectURL(url), 60_000);
  return filename;
}
