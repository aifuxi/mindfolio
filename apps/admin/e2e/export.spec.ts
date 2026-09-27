import { readFile } from "node:fs/promises";
import { expect, test } from "@playwright/test";

test("导出失败不生成文件，成功下载可解析的 JSON", async ({ page }) => {
  let fail = true;
  await page.route("**/api/auth/**", async (route) => {
    await route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({ username: "owner", csrf_token: "csrf-test" }),
    });
  });
  await page.route("**/api/export", async (route) => {
    if (fail) {
      await route.fulfill({
        status: 503,
        contentType: "application/json",
        body: JSON.stringify({
          code: "database_unavailable",
          message: "数据库不可用",
          request_id: "test",
        }),
      });
      return;
    }
    await route.fulfill({
      status: 200,
      headers: {
        "content-type": "application/json; charset=utf-8",
        "content-disposition":
          'attachment; filename="mindfolio-2026-09-28.json"',
        "cache-control": "private, no-store",
      },
      body: JSON.stringify({
        format_version: 1,
        generated_at: "2026-09-27T16:01:00+00:00",
        projects: [],
        tasks: [],
        task_tags: [],
        task_tag_links: [],
        task_completions: [],
        habits: [],
        habit_settings: [],
        habit_pauses: [],
        habit_checkins: [],
        daily_journals: [],
      }),
    });
  });
  await page.goto("/export");
  await page.getByRole("button", { name: "下载私人数据" }).click();
  await expect(page.getByRole("alert")).toContainText("数据库不可用");
  await expect(page.getByRole("status")).toHaveCount(0);
  fail = false;
  const downloadPromise = page.waitForEvent("download");
  await page.getByRole("button", { name: "下载私人数据" }).click();
  const download = await downloadPromise;
  expect(download.suggestedFilename()).toBe("mindfolio-2026-09-28.json");
  const path = await download.path();
  expect(path).not.toBeNull();
  const content = JSON.parse(await readFile(path!, "utf8"));
  expect(content.format_version).toBe(1);
  expect(content.projects).toEqual([]);
  await expect(page.getByRole("status")).toContainText("已开始下载");
});
