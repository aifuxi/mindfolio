import { expect, test } from "@playwright/test";

test("切换周并逐日查看删除后的完成历史、习惯和记录", async ({ page }) => {
  await page.route("**/api/auth/**", async (route) => {
    await route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({ username: "owner", csrf_token: "csrf-test" }),
    });
  });
  await page.route("**/api/weekly-review**", async (route) => {
    const selected = new URL(route.request().url()).searchParams.get("date");
    const active = selected === "2026-10-01";
    await route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify(
        active
          ? {
              starts_on: "2026-09-28",
              ends_on: "2026-10-04",
              dates: [
                "2026-09-28",
                "2026-09-29",
                "2026-09-30",
                "2026-10-01",
                "2026-10-02",
                "2026-10-03",
                "2026-10-04",
              ],
              page: 1,
              completions: [
                {
                  id: "1",
                  task_id: "999",
                  task_title: "已删除任务",
                  project_name: "旧项目",
                  completed_at: "2026-10-01T01:00:00Z",
                  business_date: "2026-10-01",
                  task_exists: false,
                },
              ],
              has_more_completions: false,
              journals: [
                { business_date: "2026-10-01", body: "# 周中记录", version: 1 },
              ],
              habits: [
                {
                  id: "777",
                  name: "阅读",
                  current_exists: false,
                  snapshot: {
                    rate: { earned: 2, possible: 3, percent: 66.7 },
                    days: [
                      "2026-09-28",
                      "2026-09-29",
                      "2026-09-30",
                      "2026-10-01",
                      "2026-10-02",
                      "2026-10-03",
                      "2026-10-04",
                    ].map((date) => ({
                      date,
                      state:
                        date === "2026-10-04"
                          ? "paused"
                          : date === "2026-10-01"
                            ? "completed"
                            : "missed",
                      name: "阅读",
                      cadence: "weekly",
                      weekly_target: 3,
                      note: null,
                      checkin_version: null,
                    })),
                  },
                },
              ],
              has_more_habits: false,
            }
          : {
              starts_on: "2026-09-21",
              ends_on: "2026-09-27",
              dates: [
                "2026-09-21",
                "2026-09-22",
                "2026-09-23",
                "2026-09-24",
                "2026-09-25",
                "2026-09-26",
                "2026-09-27",
              ],
              page: 1,
              completions: [],
              has_more_completions: false,
              journals: [],
              habits: [],
              has_more_habits: false,
            },
      ),
    });
  });
  await page.goto("/weekly-review");
  await expect(
    page.getByText("这一周没有任务完成、习惯或每日记录。"),
  ).toBeVisible();
  await page.getByLabel("选择周内日期").fill("2026-10-01");
  await page.getByLabel("选择周内日期").press("Tab");
  await expect(page.getByText("2026-09-28 至 2026-10-04")).toBeVisible();
  await expect(page.getByText("阅读（已删除）")).toBeVisible();
  await expect(page.getByText("66.7%")).toBeVisible();
  const day = page.getByRole("region", { name: "2026-10-01" });
  await expect(day.getByText("已删除任务")).toBeVisible();
  await expect(day.getByText("旧项目")).toBeVisible();
  await expect(
    day.getByRole("region", { name: "2026-10-01 每日记录预览" }),
  ).toContainText("周中记录");
  await expect(day.getByRole("link", { name: "打开每日记录" })).toBeVisible();
});
