import { expect, test } from "@playwright/test";

test("创建、整理项目及冲突后保留输入", async ({ page }) => {
  let active = false;
  let project: {
    id: string;
    name: string;
    version: number;
    completed_at: string | null;
    archived_at: string | null;
  } | null = null;
  let conflict = false;

  await page.route("**/api/auth/**", async (route) => {
    const path = new URL(route.request().url()).pathname;
    if (path === "/api/auth/login") active = true;
    await route.fulfill({
      status: active ? 200 : 401,
      contentType: "application/json",
      body: JSON.stringify(
        active
          ? { username: "owner", csrf_token: "csrf-test" }
          : { code: "unauthorized", message: "请先登录", request_id: "test" },
      ),
    });
  });
  await page.route(/\/api\/projects(?:\/|\?|$)/, async (route) => {
    const request = route.request();
    const path = new URL(request.url()).pathname;
    const method = request.method();
    if (method !== "GET") {
      expect(request.headers()["x-csrf-token"]).toBe("csrf-test");
    }
    if (method === "POST" && path === "/api/projects") {
      project = {
        id: "9007199254740993",
        name: request.postDataJSON().name,
        version: 1,
        completed_at: null,
        archived_at: null,
      };
      await route.fulfill({
        status: 201,
        contentType: "application/json",
        body: JSON.stringify(project),
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
            message: "项目已变更，请刷新后重试",
            request_id: "test",
          }),
        });
        return;
      }
      project = { ...project!, name: request.postDataJSON().name, version: 2 };
      await route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify(project),
      });
      return;
    }
    if (method === "POST") {
      const action = path.split("/").at(-1);
      project = {
        ...project!,
        version: project!.version + 1,
        completed_at:
          action === "complete"
            ? "2026-09-27T10:00:00Z"
            : project!.completed_at,
        archived_at:
          action === "archive"
            ? "2026-09-27T11:00:00Z"
            : action === "restore"
              ? null
              : project!.archived_at,
      };
      await route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify(project),
      });
      return;
    }
    const includeArchived =
      new URL(request.url()).searchParams.get("include_archived") === "true";
    await route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({
        items:
          project && (includeArchived || !project.archived_at) ? [project] : [],
        page: 1,
        has_more: false,
      }),
    });
  });

  await page.goto("/");
  await page.getByLabel("账号").fill("owner");
  await page.getByLabel("密码").fill("correct horse battery staple");
  await page.getByRole("button", { name: "登录", exact: true }).click();
  await expect(page.getByRole("heading", { name: "私人管理端" })).toBeVisible();
  await page.getByLabel("项目名称").fill("个人网站");
  await page.getByRole("button", { name: "创建项目" }).click();
  await expect(page.getByRole("heading", { name: "个人网站" })).toBeVisible();
  await page.getByRole("button", { name: "改名" }).click();
  await page.getByLabel("修改项目名称").fill("新的项目名称");
  conflict = true;
  await page.getByRole("button", { name: "保存" }).click();
  await expect(page.getByRole("alert")).toContainText("项目已变更");
  await expect(page.getByLabel("修改项目名称")).toHaveValue("新的项目名称");
  conflict = false;
  await page.getByRole("button", { name: "保存" }).click();
  await expect(
    page.getByRole("heading", { name: "新的项目名称" }),
  ).toBeVisible();
  await page.getByRole("button", { name: "标记完成" }).click();
  await expect(page.getByText("已完成", { exact: true })).toBeVisible();
  await page.getByRole("button", { name: "归档" }).click();
  await expect(
    page.getByText("当前没有项目，可以从上方创建一个。"),
  ).toBeVisible();
  await page.getByRole("button", { name: "查看全部" }).click();
  await expect(page.getByText("已归档", { exact: true })).toBeVisible();
  await page.getByRole("button", { name: "恢复" }).click();
  await expect(page.getByText("已完成", { exact: true })).toBeVisible();
});
