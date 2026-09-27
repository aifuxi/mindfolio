import { expect, test } from "@playwright/test";

test("创建、编辑及暂停习惯时保留冲突输入", async ({ page }) => {
  let habit: Record<string, unknown> | null = null;
  let conflict = false;
  await page.route("**/api/auth/**", async (route) => {
    await route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({ username: "owner", csrf_token: "csrf-test" }),
    });
  });
  await page.route(/\/api\/habits(?:\/|\?|$)/, async (route) => {
    const request = route.request();
    const path = new URL(request.url()).pathname;
    const method = request.method();
    if (method !== "GET")
      expect(request.headers()["x-csrf-token"]).toBe("csrf-test");
    if (method === "POST" && path === "/api/habits") {
      habit = {
        id: "9007199254740993",
        created_on: "2026-09-27",
        deleted_at: null,
        version: 1,
        paused: false,
        ...request.postDataJSON(),
      };
      await route.fulfill({
        status: 201,
        contentType: "application/json",
        body: JSON.stringify(habit),
      });
      return;
    }
    if (method === "PATCH") {
      if (conflict) {
        await route.fulfill({
          status: 409,
          contentType: "application/json",
          body: JSON.stringify({
            code: "version_conflict",
            message: "习惯已变更",
            request_id: "test",
          }),
        });
        return;
      }
      habit = { ...habit, ...request.postDataJSON(), version: 2 };
    }
    if (method === "POST" && path.endsWith("/pause"))
      habit = { ...habit, version: 3, paused: true };
    if (method === "GET" && path === "/api/habits") {
      await route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify({
          items: habit ? [habit] : [],
          page: 1,
          has_more: false,
        }),
      });
      return;
    }
    if (method === "GET") {
      await route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify({
          habit,
          settings: [
            {
              effective_on: "2026-09-27",
              name: "阅读",
              cadence: "daily",
              weekly_target: null,
            },
          ],
          pauses: [],
        }),
      });
      return;
    }
    await route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify(habit),
    });
  });
  await page.goto("/habits");
  await page.getByLabel("名称").fill("阅读");
  await page.getByRole("button", { name: "保存" }).click();
  await expect(page.getByText("阅读 · 每天")).toBeVisible();
  await page.getByRole("button", { name: "编辑" }).click();
  await page.getByLabel("名称").fill("每周阅读");
  await page.getByLabel("目标").selectOption("weekly");
  conflict = true;
  await page.getByRole("button", { name: "保存" }).click();
  await expect(page.getByRole("alert")).toContainText("习惯已变更");
  await expect(page.getByLabel("名称")).toHaveValue("每周阅读");
  conflict = false;
  await page.getByRole("button", { name: "保存" }).click();
  await expect(page.getByText("每周阅读 · 每周 3 天")).toBeVisible();
  await page.getByRole("button", { name: "暂停" }).click();
  await expect(page.getByText("已暂停")).toBeVisible();
  await page.getByRole("button", { name: "查看历史" }).click();
  await expect(
    page.getByRole("heading", { name: "每周阅读 · 设定历史" }),
  ).toBeVisible();
});
