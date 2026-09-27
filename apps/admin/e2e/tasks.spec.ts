import { expect, test } from "@playwright/test";

test("收件箱任务归入项目、完成及冲突后保留编辑", async ({ page }) => {
  let active = false;
  let conflict = false;
  let task: {
    id: string;
    project_id: string | null;
    title: string;
    description: string;
    status: string;
    priority: string | null;
    planned_date: string | null;
    due_date: string | null;
    in_backlog: boolean;
    version: number;
  } | null = null;
  const project = {
    id: "9007199254740993",
    name: "项目甲",
    version: 1,
    completed_at: null,
    archived_at: null,
  };

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
  await page.route(/\/api\/projects(?:\/|\?|$)/, (route) =>
    route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({ items: [project], page: 1, has_more: false }),
    }),
  );
  await page.route(/\/api\/tasks(?:\/|\?|$)/, async (route) => {
    const request = route.request();
    const url = new URL(request.url());
    if (request.method() !== "GET") {
      expect(request.headers()["x-csrf-token"]).toBe("csrf-test");
    }
    if (request.method() === "POST") {
      const input = request.postDataJSON();
      task = {
        id: "9007199254740994",
        project_id: input.project_id ?? null,
        title: input.title,
        description: input.description ?? "",
        status: input.status ?? "todo",
        priority: input.priority ?? null,
        planned_date: input.planned_date ?? null,
        due_date: input.due_date ?? null,
        in_backlog: input.in_backlog ?? false,
        version: 1,
      };
      await route.fulfill({
        status: 201,
        contentType: "application/json",
        body: JSON.stringify(task),
      });
      return;
    }
    if (request.method() === "PATCH") {
      if (conflict) {
        await route.fulfill({
          status: 409,
          contentType: "application/json",
          body: JSON.stringify({
            code: "version_conflict",
            message: "任务已变更，请刷新后重试",
            request_id: "test",
          }),
        });
        return;
      }
      const input = request.postDataJSON();
      task = { ...task!, ...input, version: task!.version + 1 };
      delete (task as Record<string, unknown>).expected_version;
      await route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify(task),
      });
      return;
    }
    const scope = url.searchParams.get("scope");
    const projectId = url.searchParams.get("project_id");
    const visible =
      task &&
      (scope === "inbox"
        ? task.project_id === null
        : task.project_id === projectId);
    await route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({
        items: visible ? [task] : [],
        page: 1,
        has_more: false,
      }),
    });
  });

  await page.goto("/");
  await page.getByLabel("账号").fill("owner");
  await page.getByLabel("密码").fill("correct horse battery staple");
  await page.getByRole("button", { name: "登录", exact: true }).click();
  await page.getByRole("link", { name: "打开收件箱与任务" }).click();
  await expect(
    page.getByRole("heading", { name: "收件箱与任务" }),
  ).toBeVisible();
  await page.getByLabel("标题", { exact: true }).fill("收件箱事项");
  await page.getByLabel("说明（Markdown 原文）").first().fill("**原文**");
  await page.getByLabel("计划日期").first().fill("2026-09-27");
  await page.getByRole("button", { name: "创建任务" }).click();
  await expect(page.getByRole("heading", { name: "收件箱事项" })).toBeVisible();

  await page.getByRole("button", { name: "编辑" }).click();
  await page.locator("#edit-project").selectOption(project.id);
  await page.locator("#edit-status").selectOption("in_progress");
  await page.getByLabel("放入待规划区").last().check();
  await page.getByRole("button", { name: "保存任务" }).click();
  await expect(page.getByText("这里还没有任务。")).toBeVisible();
  await page.getByLabel("查看范围").selectOption(project.id);
  await expect(page.getByRole("heading", { name: "收件箱事项" })).toBeVisible();
  await expect(page.getByText("待规划区", { exact: true })).toBeVisible();

  await page.getByRole("button", { name: "编辑" }).click();
  await page.locator("#edit-status").selectOption("completed");
  await page.getByRole("button", { name: "保存任务" }).click();
  await expect(
    page.getByRole("listitem").getByText("已完成", { exact: true }),
  ).toBeVisible();
  await page.getByRole("button", { name: "编辑" }).click();
  await page.locator("#edit-title-input").fill("保留的编辑内容");
  conflict = true;
  await page.getByRole("button", { name: "保存任务" }).click();
  await expect(page.getByRole("alert")).toContainText("任务已变更");
  await expect(page.locator("#edit-title-input")).toHaveValue("保留的编辑内容");
});
