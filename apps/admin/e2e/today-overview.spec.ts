import { expect, test } from "@playwright/test";

test("今日同时处理习惯打卡并进入每日记录", async ({ page }) => {
  let checked = false;
  let journal = false;
  await page.route("**/api/auth/**", async (route) => {
    await route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({ username: "owner", csrf_token: "csrf-test" }),
    });
  });
  await page.route("**/api/projects**", async (route) => {
    await route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({ items: [], page: 1, has_more: false }),
    });
  });
  await page.route("**/api/today/tasks**", async (route) => {
    await route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({
        business_date: "2026-09-27",
        items: [],
        page: 1,
        has_more: false,
      }),
    });
  });
  await page.route("**/api/today/overview**", async (route) => {
    await route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({
        business_date: "2026-09-27",
        habits: [
          {
            id: "9007199254740993",
            version: 1,
            name: "阅读",
            cadence: "daily",
            weekly_target: null,
            checkin_completed: checked ? true : null,
            checkin_note: checked ? "睡前" : null,
            checkin_version: checked ? 1 : null,
            needs_checkin: !checked,
          },
        ],
        page: 1,
        has_more: false,
        journal_exists: journal,
      }),
    });
  });
  await page.route(
    "**/api/habits/9007199254740993/checkins/2026-09-27",
    async (route) => {
      expect(route.request().headers()["x-csrf-token"]).toBe("csrf-test");
      expect(route.request().postDataJSON().note).toBe("睡前");
      checked = true;
      await route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify({
          id: "1",
          habit_id: "9007199254740993",
          business_date: "2026-09-27",
          completed: true,
          note: "睡前",
          version: 1,
        }),
      });
    },
  );
  await page.route("**/api/journal/2026-09-27", async (route) => {
    if (route.request().method() === "PUT") {
      journal = true;
      await route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify({
          business_date: "2026-09-27",
          body: "今日回顾",
          version: 1,
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
        journal: journal
          ? {
              business_date: "2026-09-27",
              body: "今日回顾",
              version: 1,
              created_at: "2026-09-27T01:00:00Z",
              updated_at: "2026-09-27T01:00:00Z",
            }
          : null,
        completions: [],
        checkins: [],
      }),
    });
  });
  await page.goto("/today");
  await expect(
    page.getByRole("region", { name: "今日习惯" }).getByText("待打卡"),
  ).toBeVisible();
  await expect(page.getByText("今天没有需要处理的任务。")).toBeVisible();
  await page
    .getByRole("region", { name: "今日习惯" })
    .getByRole("button", { name: "打卡", exact: true })
    .click();
  await page
    .getByRole("region", { name: "今日习惯" })
    .getByLabel("备注")
    .fill("睡前");
  await page.getByRole("button", { name: "保存打卡" }).click();
  await expect(
    page.getByRole("region", { name: "今日习惯" }).getByText("今天已完成"),
  ).toBeVisible();
  await page.getByRole("link", { name: "写每日记录" }).click();
  await expect(page.getByLabel("Markdown 正文")).toBeVisible();
  await page.getByLabel("Markdown 正文").fill("今日回顾");
  await page.getByRole("button", { name: "保存记录" }).click();
  await page.getByRole("link", { name: "今日" }).click();
  await expect(page.getByText("今天已有每日记录，可继续编辑。")).toBeVisible();
});
