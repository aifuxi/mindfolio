import { describe, expect, it, vi } from "vitest";
import { checkApiStatus } from "./api-status";

describe("API 状态检查", () => {
  it("将成功的存活响应显示为正常", async () => {
    const request = vi.fn<typeof fetch>().mockResolvedValue({
      ok: true,
      json: async () => ({ status: "ok" }),
    } as Response);

    await expect(checkApiStatus(request)).resolves.toBe("API 正常");
    expect(request).toHaveBeenCalledWith("/api/health/live");
  });

  it("在网络错误时显示不可用", async () => {
    const request = vi
      .fn<typeof fetch>()
      .mockRejectedValue(new Error("连接失败"));
    await expect(checkApiStatus(request)).resolves.toBe("API 不可用");
  });
});
