import { expect, test } from "@playwright/test";

test("收件箱任务归入项目、完成及冲突后保留编辑", async ({ page }) => {
  let active = false;
  let conflict = false;
  let task: {
    id: string;
    parent_id: string | null;
    project_id: string | null;
    title: string;
    description: string;
    status: string;
    priority: string | null;
    planned_date: string | null;
    due_date: string | null;
    in_backlog: boolean;
    tags: string[];
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
        parent_id: null,
        project_id: input.project_id ?? null,
        title: input.title,
        description: input.description ?? "",
        status: input.status ?? "todo",
        priority: input.priority ?? null,
        planned_date: input.planned_date ?? null,
        due_date: input.due_date ?? null,
        in_backlog: input.in_backlog ?? false,
        tags: input.tags ?? [],
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

test("创建和维护一级子任务时提示未完成步骤", async ({ page }) => {
  let active = false;
  const parent = {
    id: "9007199254740993",
    parent_id: null,
    project_id: null,
    title: "整理资料",
    description: "",
    status: "todo",
    priority: null,
    planned_date: null,
    due_date: null,
    in_backlog: false,
    tags: [],
    version: 1,
  };
  let child: typeof parent | null = null;
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
      contentType: "application/json",
      body: JSON.stringify({ items: [], page: 1, has_more: false }),
    }),
  );
  await page.route(/\/api\/tasks(?:\/|\?|$)/, async (route) => {
    const request = route.request();
    const path = new URL(request.url()).pathname;
    if (path.endsWith("/subtasks")) {
      await route.fulfill({
        contentType: "application/json",
        body: JSON.stringify({
          items: child ? [child] : [],
          page: 1,
          has_more: false,
        }),
      });
      return;
    }
    if (request.method() === "POST") {
      const input = request.postDataJSON();
      expect(input.parent_id).toBe(parent.id);
      child = {
        ...parent,
        ...input,
        id: "9007199254740994",
        parent_id: parent.id,
        planned_date: input.planned_date ?? null,
        due_date: input.due_date ?? null,
      };
      await route.fulfill({
        status: 201,
        contentType: "application/json",
        body: JSON.stringify(child),
      });
      return;
    }
    if (request.method() === "PATCH") {
      const input = request.postDataJSON();
      const target = path.endsWith(child?.id ?? "missing") ? child! : parent;
      Object.assign(target, input, { version: target.version + 1 });
      delete (target as Record<string, unknown>).expected_version;
      await route.fulfill({
        contentType: "application/json",
        body: JSON.stringify(target),
      });
      return;
    }
    if (path.endsWith(parent.id)) {
      await route.fulfill({
        contentType: "application/json",
        body: JSON.stringify(parent),
      });
      return;
    }
    if (child && path.endsWith(child.id)) {
      await route.fulfill({
        contentType: "application/json",
        body: JSON.stringify(child),
      });
      return;
    }
    await route.fulfill({
      contentType: "application/json",
      body: JSON.stringify({ items: [parent], page: 1, has_more: false }),
    });
  });

  await page.goto("/");
  await page.getByLabel("账号").fill("owner");
  await page.getByLabel("密码").fill("correct horse battery staple");
  await page.getByRole("button", { name: "登录", exact: true }).click();
  await page.getByRole("link", { name: "打开收件箱与任务" }).click();
  await page.getByRole("button", { name: "编辑", exact: true }).click();
  await page.getByLabel("子任务标题").fill("收集文档");
  await page.getByLabel("子任务计划日期").fill("2026-09-29");
  await page.getByRole("button", { name: "创建子任务" }).click();
  await expect(page.getByRole("heading", { name: "收集文档" })).toBeVisible();
  await page.getByRole("button", { name: "编辑子任务" }).click();
  await page.locator("#edit-status").selectOption("in_progress");
  await page.locator("#edit-due").fill("2026-09-30");
  await page.getByRole("button", { name: "保存任务" }).click();
  expect(child?.status).toBe("in_progress");
  expect(child?.due_date).toBe("2026-09-30");
  await page.getByRole("button", { name: "编辑", exact: true }).click();
  await page.locator("#edit-status").selectOption("completed");
  await page.getByRole("button", { name: "保存任务" }).click();
  await expect(
    page.getByRole("alertdialog", { name: "确认完成父任务" }),
  ).toContainText("1 个未完成子任务");
  expect(parent.status).toBe("todo");
  await page.getByRole("button", { name: "暂不完成" }).click();
  expect(parent.status).toBe("todo");
  await page.getByRole("button", { name: "保存任务" }).click();
  await page.getByRole("button", { name: "仍要完成父任务" }).click();
  await expect(
    page.getByRole("listitem").getByText("已完成", { exact: true }),
  ).toBeVisible();
  expect(child?.status).toBe("in_progress");
});

test("项目列表和看板共用筛选结果且切换不写入任务", async ({ page }) => {
  let active = false;
  let writes = 0;
  let reads = 0;
  const project = {
    id: "9007199254740993",
    name: "检索项目",
    version: 1,
    completed_at: null,
    archived_at: null,
  };
  const base = {
    project_id: project.id,
    description: "",
    priority: null,
    planned_date: null,
    due_date: null,
    in_backlog: false,
    tags: [] as string[],
    version: 1,
  };
  const tasks = [
    {
      ...base,
      id: "11",
      parent_id: null,
      title: "制定方案",
      status: "todo",
      in_backlog: true,
      tags: ["研究"],
    },
    {
      ...base,
      id: "12",
      parent_id: "11",
      title: "查找资料",
      status: "in_progress",
      planned_date: "2026-09-29",
      tags: ["研究"],
    },
    {
      ...base,
      id: "13",
      parent_id: null,
      title: "提交成果",
      status: "completed",
      priority: "high",
      tags: ["交付"],
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
      contentType: "application/json",
      body: JSON.stringify({ items: [project], page: 1, has_more: false }),
    }),
  );
  await page.route(/\/api\/tasks(?:\/|\?|$)/, async (route) => {
    if (route.request().method() !== "GET") writes++;
    reads++;
    const url = new URL(route.request().url());
    const visible =
      url.searchParams.get("scope") === "project"
        ? tasks.filter(
            (task) =>
              (!url.searchParams.get("keyword") ||
                task.title.includes(url.searchParams.get("keyword")!)) &&
              (!url.searchParams.get("status") ||
                task.status === url.searchParams.get("status")) &&
              (!url.searchParams.get("priority") ||
                task.priority === url.searchParams.get("priority")) &&
              (!url.searchParams.get("planned_date") ||
                task.planned_date === url.searchParams.get("planned_date")) &&
              (!url.searchParams.get("tag") ||
                task.tags.includes(url.searchParams.get("tag")!)),
          )
        : [];
    await route.fulfill({
      contentType: "application/json",
      body: JSON.stringify({ items: visible, page: 1, has_more: false }),
    });
  });

  await page.goto("/");
  await page.getByLabel("账号").fill("owner");
  await page.getByLabel("密码").fill("correct horse battery staple");
  await page.getByRole("button", { name: "登录", exact: true }).click();
  await page.getByRole("link", { name: "打开收件箱与任务" }).click();
  await page.getByLabel("查看范围").selectOption(project.id);
  await expect(
    page.getByRole("list", { name: "任务列表" }).getByRole("listitem"),
  ).toHaveCount(3);
  const beforeSwitch = reads;
  await page.getByRole("button", { name: "看板" }).click();
  await expect(
    page.getByRole("region", { name: "待规划区" }).getByText("制定方案"),
  ).toBeVisible();
  await expect(
    page.getByRole("region", { name: "进行中" }).getByText("查找资料"),
  ).toBeVisible();
  await expect(
    page.getByRole("region", { name: "已完成" }).getByText("提交成果"),
  ).toBeVisible();
  await page.getByRole("button", { name: "列表", exact: true }).click();
  expect(reads).toBe(beforeSwitch);
  expect(writes).toBe(0);

  await page.getByLabel("关键词").fill("资料");
  await page
    .getByLabel("状态", { exact: true })
    .last()
    .selectOption("in_progress");
  await page.getByLabel("计划日期", { exact: true }).last().fill("2026-09-29");
  await page.getByLabel("任务标签", { exact: true }).fill("研究");
  await page.getByRole("button", { name: "筛选", exact: true }).click();
  await expect(
    page.getByRole("list", { name: "任务列表" }).getByRole("listitem"),
  ).toHaveCount(1);
  await expect(page.getByRole("heading", { name: "↳ 查找资料" })).toBeVisible();
  await page.getByRole("button", { name: "看板" }).click();
  await expect(
    page.getByRole("region", { name: "进行中" }).getByText("查找资料"),
  ).toBeVisible();
  await page.getByRole("button", { name: "清除筛选" }).click();
  await expect(
    page.getByRole("region", { name: "已完成" }).getByText("提交成果"),
  ).toBeVisible();
  expect(writes).toBe(0);
});
