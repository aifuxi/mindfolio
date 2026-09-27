import createClient from "openapi-fetch";
import type { components, paths } from "@mindfolio/api-contract";
import { currentSession } from "./auth";

export type Project = components["schemas"]["ProjectResponse"];
export type ProjectPage = components["schemas"]["ProjectPage"];
type ApiError = components["schemas"]["ApiError"];

const client = createClient<paths>({ baseUrl: "/api" });

export class ProjectRequestError extends Error {
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
    throw new ProjectRequestError(
      error?.message ?? "项目操作失败",
      response.status,
    );
  }
  return data;
}

function csrf(): string {
  const token = currentSession.value?.csrf_token;
  if (!token) throw new ProjectRequestError("登录已过期", 401);
  return token;
}

export async function listProjects(page: number, includeArchived: boolean) {
  const { data, error, response } = await client.GET("/projects", {
    params: { query: { page, include_archived: includeArchived } },
  });
  return result(response, data, error);
}

export async function createProject(name: string) {
  const { data, error, response } = await client.POST("/projects", {
    params: { header: { "x-csrf-token": csrf() } },
    body: { name },
  });
  return result(response, data, error);
}

export async function renameProject(project: Project, name: string) {
  const { data, error, response } = await client.PATCH("/projects/{id}", {
    params: { path: { id: project.id }, header: { "x-csrf-token": csrf() } },
    body: { name, expected_version: project.version },
  });
  return result(response, data, error);
}

export async function changeProject(
  project: Project,
  action: "complete" | "archive" | "restore",
) {
  const params = {
    path: { id: project.id },
    header: { "x-csrf-token": csrf() },
  };
  const body = { expected_version: project.version };
  const outcome =
    action === "complete"
      ? await client.POST("/projects/{id}/complete", { params, body })
      : action === "archive"
        ? await client.POST("/projects/{id}/archive", { params, body })
        : await client.POST("/projects/{id}/restore", { params, body });
  return result(outcome.response, outcome.data, outcome.error);
}

export async function deleteProject(project: Project) {
  const { response, error } = await client.DELETE("/projects/{id}", {
    params: { path: { id: project.id }, header: { "x-csrf-token": csrf() } },
    body: { expected_version: project.version },
  });
  if (!response.ok) {
    if (response.status === 401) currentSession.value = null;
    throw new ProjectRequestError(
      error?.message ?? "删除项目失败",
      response.status,
    );
  }
}
