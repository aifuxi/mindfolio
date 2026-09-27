import { expect, test } from "@playwright/test";

test("从收件箱安排到今天并从今日任务进入原任务", async ({ page }) => {
  let active = false;
  const project = {
    id: "9007199254740993",
    name: "项目甲",
    version: 1,
    completed_at: null,
    archived_at: null,
  };
  const tasks = [
    {
      id: "9007199254740994",
      parent_id: null,
      project_id: null as string | null,
      title: "收件箱事项",
      description: "",
      status: "todo",
      priority: null,
      planned_date: null as string | null,
      due_date: "2026-10-01",
      in_backlog: false,
      tags: [],
      version: 1,
    },
    {
      id: "9007199254740995",
      parent_id: null,
      project_id: project.id as string | null,
      title: "项目逾期",
      description: "",
      status: "in_progress",
      priority: null,
      planned_date: "2026-09-28" as string | null,
      due_date: "2026-09-27",
      in_backlog: false,
      tags: [],
      version: 1,
    },
  ];

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
    if (request.method() === "POST" && url.pathname.endsWith("/plan-today")) {
      expect(request.headers()["x-csrf-token"]).toBe("csrf-test");
      const task = tasks.find((value) => url.pathname.includes(value.id))!;
      expect(request.postDataJSON().expected_version).toBe(task.version);
      task.planned_date = "2026-09-28";
      task.version++;
      await route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify(task),
      });
      return;
    }
    if (url.pathname.endsWith("/subtasks")) {
      await route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify({ items: [], page: 1, has_more: false }),
      });
      return;
    }
    if (url.pathname === "/api/tasks") {
      const projectId = url.searchParams.get("project_id");
      const visible = tasks.filter((task) => task.project_id === projectId);
      await route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify({ items: visible, page: 1, has_more: false }),
      });
      return;
    }
    const task = tasks.find((value) => url.pathname.endsWith(value.id));
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
  await page.route(/\/api\/today\/tasks(?:\?|$)/, (route) => {
    const inbox = tasks[0];
    const overdue = tasks[1];
    const items = [
      {
        task: overdue,
        reasons: ["overdue", "planned_today"],
      },
      ...(inbox.planned_date === "2026-09-28"
        ? [{ task: inbox, reasons: ["planned_today"] }]
        : []),
    ];
    return route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({
        business_date: "2026-09-28",
        items,
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
  await page.getByRole("button", { name: "安排到今天" }).click();
  await expect(page.getByRole("status")).toContainText("已安排到 2026-09-28");
  await expect(page.getByText("截止 2026-10-01")).toBeVisible();
  await page.getByRole("link", { name: "今日任务" }).click();
  await expect(page.getByRole("heading", { name: "今日任务" })).toBeVisible();
  await expect(page.getByText("2026-09-28 · 计划今天")).toBeVisible();
  await expect(page.getByRole("heading", { name: "项目逾期" })).toHaveCount(1);
  await expect(page.getByText("已逾期 · 截止 2026-09-27")).toBeVisible();
  await expect(page.getByText("计划今天 · 2026-09-28")).toHaveCount(2);
  await page
    .getByRole("region", { name: "已逾期" })
    .getByRole("link", { name: "查看任务" })
    .click();
  await expect(page).toHaveURL(
    /\/tasks\?project_id=9007199254740993&task_id=9007199254740995/,
  );
  await expect(page.locator("#edit-title-input")).toHaveValue("项目逾期");
});
