import { expect, test } from "@playwright/test";

test("每日记录安全预览、保存、冲突及当日事实", async ({ page }) => {
  let saved: { body: string; version: number } | null = null;
  let conflict = false;
  await page.route("**/api/auth/**", async (route) => {
    await route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({ username: "owner", csrf_token: "csrf-test" }),
    });
  });
  await page.route("**/api/journal/2026-09-27", async (route) => {
    const request = route.request();
    if (request.method() === "PUT") {
      expect(request.headers()["x-csrf-token"]).toBe("csrf-test");
      if (conflict) {
        await route.fulfill({
          status: 409,
          contentType: "application/json",
          body: JSON.stringify({
            code: "version_conflict",
            message: "记录已变更",
            request_id: "test",
          }),
        });
        return;
      }
      saved = {
        body: request.postDataJSON().body,
        version: (saved?.version ?? 0) + 1,
      };
      await route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify({
          business_date: "2026-09-27",
          ...saved,
          created_at: "2026-09-27T01:00:00Z",
          updated_at: "2026-09-27T01:00:00Z",
        }),
      });
      return;
    }
    await route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({
        business_date: "2026-09-27",
        journal: saved
          ? {
              business_date: "2026-09-27",
              ...saved,
              created_at: "2026-09-27T01:00:00Z",
              updated_at: "2026-09-27T01:00:00Z",
            }
          : null,
        completions: [
          {
            id: "1",
            task_id: "999",
            task_title: "已删除任务",
            project_name: "旧项目",
            completed_at: "2026-09-27T01:00:00Z",
          },
        ],
        checkins: [
          {
            id: "2",
            habit_id: "777",
            name: "阅读",
            completed: true,
            note: "睡前",
          },
        ],
      }),
    });
  });
  await page.goto("/journal/2026-09-27");
  await expect(page.getByText("这一天还没有每日记录。")).toBeVisible();
  await expect(page.getByText("已删除任务")).toBeVisible();
  await expect(page.getByText("阅读 · 完成 · 睡前")).toBeVisible();
  await page
    .getByLabel("Markdown 正文")
    .fill(
      "# 今天\n<script>alert(1)</script>\n[危险](javascript:alert(1))\n![远程](https://example.com/a.png)",
    );
  await page.getByRole("button", { name: "安全预览" }).click();
  const preview = page.getByRole("region", { name: "每日记录预览" });
  await expect(preview.locator("img")).toHaveCount(0);
  await expect(preview.locator('a[href^="javascript:"]')).toHaveCount(0);
  await page.getByRole("button", { name: "保存记录" }).click();
  await expect(page.getByRole("status")).toContainText("已保存");
  conflict = true;
  await page.getByLabel("Markdown 正文").fill("# 仍在编辑");
  await page.getByRole("button", { name: "保存记录" }).click();
  await expect(page.getByRole("alert")).toContainText("记录已变更");
  await expect(page.getByLabel("Markdown 正文")).toHaveValue("# 仍在编辑");
});
