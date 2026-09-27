import { expect, test } from "@playwright/test";

test("完成、重新打开、删除后仍可回看名称快照", async ({ page }) => {
  let active = false;
  let project: {
    id: string;
    name: string;
    version: number;
    completed_at: null;
    archived_at: null;
  } | null = {
    id: "9007199254740993",
    name: "旧项目",
    version: 1,
    completed_at: null,
    archived_at: null,
  };
  let task: {
    id: string;
    parent_id: null;
    project_id: string;
    title: string;
    description: string;
    status: string;
    priority: null;
    planned_date: null;
    due_date: null;
    in_backlog: boolean;
    tags: string[];
    version: number;
  } | null = {
    id: "9007199254740994",
    parent_id: null,
    project_id: "9007199254740993",
    title: "旧任务",
    description: "",
    status: "todo",
    priority: null,
    planned_date: null,
    due_date: null,
    in_backlog: false,
    tags: [],
    version: 1,
  };
  const completions: {
    id: string;
    task_id: string;
    task_title: string;
    project_name: string;
    completed_at: string;
    business_date: string;
  }[] = [];

  await page.route("**/api/auth/**", async (route) => {
    if (new URL(route.request().url()).pathname === "/api/auth/login")
      active = true;
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
    if (request.method() === "DELETE") {
      expect(request.headers()["x-csrf-token"]).toBe("csrf-test");
      expect(request.postDataJSON().expected_version).toBe(project?.version);
      project = null;
      await route.fulfill({ status: 204 });
      return;
    }
    await route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({
        items: project ? [project] : [],
        page: 1,
        has_more: false,
      }),
    });
  });
  await page.route(/\/api\/tasks(?:\/|\?|$)/, async (route) => {
    const request = route.request();
    const path = new URL(request.url()).pathname;
    if (path.endsWith("/subtasks")) {
      await route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify({ items: [], page: 1, has_more: false }),
      });
      return;
    }
    if (request.method() === "PATCH") {
      expect(request.headers()["x-csrf-token"]).toBe("csrf-test");
      const input = request.postDataJSON();
      expect(input.expected_version).toBe(task?.version);
      const oldStatus = task!.status;
      task = { ...task!, ...input, version: task!.version + 1 };
      delete (task as Record<string, unknown>).expected_version;
      if (oldStatus !== "completed" && task.status === "completed") {
        completions.unshift({
          id: String(completions.length + 1),
          task_id: task.id,
          task_title: task.title,
          project_name: project!.name,
          completed_at: "2026-09-27T10:00:00Z",
          business_date: "2026-09-27",
        });
      }
      await route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify(task),
      });
      return;
    }
    if (request.method() === "DELETE") {
      expect(request.headers()["x-csrf-token"]).toBe("csrf-test");
      expect(request.postDataJSON().expected_version).toBe(task?.version);
      task = null;
      await route.fulfill({ status: 204 });
      return;
    }
    if (path === "/api/tasks") {
      await route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify({
          items: task ? [task] : [],
          page: 1,
          has_more: false,
        }),
      });
      return;
    }
    await route.fulfill({
      status: task ? 200 : 404,
      contentType: "application/json",
      body: JSON.stringify(
        task ?? {
          code: "not_found",
          message: "任务不存在",
          request_id: "test",
        },
      ),
    });
  });
  await page.route(/\/api\/task-completions(?:\/|\?|$)/, async (route) => {
    const path = new URL(route.request().url()).pathname;
    await route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({
        items: path.endsWith("/days")
          ? completions.length
            ? [
                {
                  business_date: "2026-09-27",
                  completed_count: completions.length,
                },
              ]
            : []
          : completions,
        page: 1,
        has_more: false,
      }),
    });
  });

  await page.goto("/");
  await page.getByLabel("账号").fill("owner");
  await page.getByLabel("密码").fill("correct horse battery staple");
  await page.getByRole("button", { name: "登录", exact: true }).click();
  await page.getByRole("link", { name: "查看任务" }).click();
  await page.getByRole("button", { name: "编辑" }).click();
  await page.locator("#edit-status").selectOption("completed");
  await page.getByRole("button", { name: "保存任务" }).click();
  await page.getByRole("link", { name: "完成历史" }).click();
  await page.getByRole("button", { name: /2026-09-27 · 1 次完成/ }).click();
  await expect(
    page.getByRole("listitem").filter({ hasText: "旧任务" }),
  ).toContainText("旧项目");

  await page.getByRole("link", { name: "查看任务" }).click();
  await page.getByRole("button", { name: "编辑" }).click();
  await page.locator("#edit-status").selectOption("todo");
  await page.locator("#edit-title-input").fill("新任务");
  await page.getByRole("button", { name: "保存任务" }).click();
  await page.getByRole("button", { name: "编辑" }).click();
  await page.locator("#edit-status").selectOption("completed");
  await page.getByRole("button", { name: "保存任务" }).click();
  await page.getByRole("button", { name: "编辑" }).click();
  await page.getByRole("button", { name: "删除任务" }).click();
  await expect(
    page.getByRole("alertdialog", { name: "确认删除任务" }),
  ).toContainText("已有完成历史");
  await page.getByRole("button", { name: "确认删除任务" }).click();
  await expect(page.getByText("这里还没有任务。")).toBeVisible();
  await page.getByRole("link", { name: "返回项目" }).click();
  await page.getByRole("button", { name: "删除", exact: true }).click();
  await expect(
    page.getByRole("alertdialog", { name: "确认删除项目" }),
  ).toContainText("已有完成历史");
  await page.getByRole("button", { name: "确认删除项目" }).click();
  await page.getByRole("link", { name: "查看完成历史" }).click();
  await page.getByRole("button", { name: /2026-09-27 · 2 次完成/ }).click();
  await expect(
    page.getByRole("listitem").filter({ hasText: "旧任务" }),
  ).toContainText("旧项目");
  await expect(
    page.getByRole("listitem").filter({ hasText: "新任务" }),
  ).toContainText("旧项目");
  await expect(page.getByRole("link", { name: "旧任务" })).toHaveCount(0);
});
