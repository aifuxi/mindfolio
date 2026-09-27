export async function checkApiStatus(request: typeof fetch): Promise<string> {
  try {
    const response = await request("/api/health/live");
    if (!response.ok) return "API 不可用";
    const result: { status: string } = await response.json();
    return result.status === "ok" ? "API 正常" : "API 响应异常";
  } catch {
    return "API 不可用";
  }
}
