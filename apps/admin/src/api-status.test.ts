import { describe, expect, it, vi } from "vitest";
import { checkApiStatus } from "./api-status";

describe("API 状态检查", () => {
  it("将数据库就绪响应显示为正常", async () => {
    const request = vi.fn<typeof fetch>().mockResolvedValue(
      new Response(JSON.stringify({ status: "ok" }), {
        status: 200,
        headers: { "content-type": "application/json" },
      }),
    );

    await expect(checkApiStatus(request, "http://localhost/api")).resolves.toBe(
      "API 正常",
    );
    expect((request.mock.calls[0]?.[0] as Request).url).toBe(
      "http://localhost/api/health/ready",
    );
  });

  it("使用生成的错误类型识别数据库不可用", async () => {
    const request = vi.fn<typeof fetch>().mockResolvedValue(
      new Response(
        JSON.stringify({
          code: "database_unavailable",
          message: "数据库不可用",
          request_id: "test-request-id",
        }),
        {
          status: 503,
          headers: { "content-type": "application/json" },
        },
      ),
    );
    await expect(checkApiStatus(request, "http://localhost/api")).resolves.toBe(
      "数据库不可用",
    );
  });

  it("在网络错误时显示不可用", async () => {
    const request = vi
      .fn<typeof fetch>()
      .mockRejectedValue(new Error("连接失败"));
    await expect(checkApiStatus(request, "http://localhost/api")).resolves.toBe(
      "API 不可用",
    );
  });
});
