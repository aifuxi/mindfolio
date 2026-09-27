import createClient from "openapi-fetch";
import type { components, paths } from "@mindfolio/api-contract";
import { currentSession } from "./auth";

export type Task = components["schemas"]["TaskResponse"];
export type TaskStatus = components["schemas"]["TaskStatus"];
export type TaskPriority = components["schemas"]["TaskPriority"];
export type CreateTask = components["schemas"]["CreateTask"];
export type UpdateTask = components["schemas"]["UpdateTask"];
export type TodayTask = components["schemas"]["TodayTaskResponse"];
export type TaskFilters = {
  keyword?: string;
  status?: TaskStatus;
  priority?: TaskPriority;
  planned_date?: string;
  due_date?: string;
  tag?: string;
};
type ApiError = components["schemas"]["ApiError"];

const client = createClient<paths>({ baseUrl: "/api" });

export class TaskRequestError extends Error {
  constructor(
    message: string,
    public readonly status: number,
  ) {
    super(message);
  }
}

function result<T>(
  response: Response,
  data: T | undefined,
  error: ApiError | undefined,
): T {
  if (!response.ok || !data) {
    if (response.status === 401) currentSession.value = null;
    throw new TaskRequestError(
      error?.message ?? "任务操作失败",
      response.status,
    );
  }
  return data;
}

function csrf(): string {
  const token = currentSession.value?.csrf_token;
  if (!token) throw new TaskRequestError("登录已过期", 401);
  return token;
}

export async function listTasks(
  page: number,
  projectId: string | null,
  filters: TaskFilters = {},
) {
  const { data, error, response } = await client.GET("/tasks", {
    params: {
      query: projectId
        ? {
            scope: "project",
            project_id: projectId,
            include_subtasks: true,
            page,
            ...filters,
          }
        : { scope: "inbox", include_subtasks: true, page, ...filters },
    },
  });
  return result(response, data, error);
}

export async function getTask(id: string) {
  const { data, error, response } = await client.GET("/tasks/{id}", {
    params: { path: { id } },
  });
  return result(response, data, error);
}

export async function listSubtasks(parentId: string) {
  const items: Task[] = [];
  let page = 1;
  while (true) {
    const { data, error, response } = await client.GET("/tasks/{id}/subtasks", {
      params: { path: { id: parentId }, query: { page } },
    });
    const resultPage = result(response, data, error);
    items.push(...resultPage.items);
    if (!resultPage.has_more) return items;
    page++;
  }
}

export async function createTask(body: CreateTask) {
  const { data, error, response } = await client.POST("/tasks", {
    params: { header: { "x-csrf-token": csrf() } },
    body,
  });
  return result(response, data, error);
}

export async function updateTask(
  task: Task,
  body: Omit<UpdateTask, "expected_version">,
) {
  const { data, error, response } = await client.PATCH("/tasks/{id}", {
    params: { path: { id: task.id }, header: { "x-csrf-token": csrf() } },
    body: { expected_version: task.version, ...body },
  });
  return result(response, data, error);
}

export async function deleteTask(task: Task) {
  const { response, error } = await client.DELETE("/tasks/{id}", {
    params: { path: { id: task.id }, header: { "x-csrf-token": csrf() } },
    body: { expected_version: task.version },
  });
  if (!response.ok) {
    if (response.status === 401) currentSession.value = null;
    throw new TaskRequestError(
      error?.message ?? "删除任务失败",
      response.status,
    );
  }
}

export async function listTodayTasks(page: number) {
  const { data, error, response } = await client.GET("/today/tasks", {
    params: { query: { page } },
  });
  return result(response, data, error);
}

export async function planTaskToday(task: Task) {
  const { data, error, response } = await client.POST(
    "/tasks/{id}/plan-today",
    {
      params: { path: { id: task.id }, header: { "x-csrf-token": csrf() } },
      body: { expected_version: task.version },
    },
  );
  return result(response, data, error);
}
