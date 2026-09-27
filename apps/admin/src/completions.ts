import createClient from "openapi-fetch";
import type { components, paths } from "@mindfolio/api-contract";
import { currentSession } from "./auth";

export type Completion = components["schemas"]["CompletionResponse"];
export type DaySummary = components["schemas"]["DaySummary"];

const client = createClient<paths>({ baseUrl: "/api" });

export class CompletionRequestError extends Error {
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
  message?: string,
): T {
  if (!response.ok || !data) {
    if (response.status === 401) currentSession.value = null;
    throw new CompletionRequestError(
      message ?? "读取完成历史失败",
      response.status,
    );
  }
  return data;
}

export async function listCompletionDays(page: number) {
  const { data, error, response } = await client.GET("/task-completions/days", {
    params: { query: { page } },
  });
  return result(response, data, error?.message);
}

export async function listCompletions(date: string, page: number) {
  const { data, error, response } = await client.GET("/task-completions", {
    params: { query: { date, page } },
  });
  return result(response, data, error?.message);
}
