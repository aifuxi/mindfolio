import { expect, test } from "@playwright/test";

test("管理者登录、刷新、过期和退出", async ({ page, context }) => {
  const health = await page.request.get("/api/health/live");
  expect(health.status()).toBe(200);
  expect(await health.json()).toEqual({ status: "ok" });
  let active = false;
  await page.route("**/api/auth/**", async (route) => {
    const path = new URL(route.request().url()).pathname;
    if (path === "/api/auth/login") {
      active = true;
      await route.fulfill({
        status: 200,
        contentType: "application/json",
        headers: {
          "set-cookie":
            "mf_session=browser-test; Path=/; HttpOnly; SameSite=Strict",
        },
        body: JSON.stringify({ username: "owner", csrf_token: "csrf-test" }),
      });
      return;
    }
    if (path === "/api/auth/logout") {
      expect(route.request().headers()["x-csrf-token"]).toBe("csrf-test");
      active = false;
      await route.fulfill({ status: 204 });
      return;
    }
    await route.fulfill({
      status: active ? 200 : 401,
      contentType: "application/json",
      body: active
        ? JSON.stringify({ username: "owner", csrf_token: "csrf-test" })
        : JSON.stringify({
            code: "unauthorized",
            message: "请先登录",
            request_id: "test",
          }),
    });
  });

  await page.goto("/");
  await expect(page.getByRole("heading", { name: "管理者登录" })).toBeVisible();
  await page.getByLabel("账号").fill("owner");
  await page.getByLabel("密码").fill("correct horse battery staple");
  await page.getByRole("button", { name: "登录", exact: true }).click();
  await expect(page.getByRole("heading", { name: "私人管理端" })).toBeVisible();
  expect(
    (await context.cookies()).some((cookie) => cookie.name === "mf_session"),
  ).toBe(true);

  await page.reload();
  await expect(page.getByText("已登录：owner")).toBeVisible();
  active = false;
  await page.reload();
  await expect(page.getByRole("heading", { name: "管理者登录" })).toBeVisible();

  await page.getByLabel("账号").fill("owner");
  await page.getByLabel("密码").fill("correct horse battery staple");
  await page.getByRole("button", { name: "登录", exact: true }).click();
  await expect(page.getByRole("heading", { name: "私人管理端" })).toBeVisible();
  await page.getByRole("button", { name: "退出登录" }).click();
  await expect(page.getByRole("heading", { name: "管理者登录" })).toBeVisible();
  await page.goto("/");
  await expect(page.getByRole("heading", { name: "管理者登录" })).toBeVisible();
});
