import createClient from "openapi-fetch";
import type { paths } from "@mindfolio/api-contract";

export async function checkApiStatus(
  request: typeof fetch,
  baseUrl = "/api",
): Promise<string> {
  try {
    const client = createClient<paths>({ baseUrl, fetch: request });
    const { data, error, response } = await client.GET("/health/ready");
    if (error?.code === "database_unavailable") return "数据库不可用";
    if (!response.ok) return "API 不可用";
    return data?.status === "ok" ? "API 正常" : "API 响应异常";
  } catch {
    return "API 不可用";
  }
}
