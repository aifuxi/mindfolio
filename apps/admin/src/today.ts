import createClient from "openapi-fetch";
import type { components, paths } from "@mindfolio/api-contract";
import { currentSession } from "./auth";

export type TodayOverview = components["schemas"]["TodayOverview"];
export type TodayHabit = components["schemas"]["TodayHabit"];

const client = createClient<paths>({ baseUrl: "/api" });

export class TodayRequestError extends Error {
  constructor(
    message: string,
    public readonly status: number,
  ) {
    super(message);
  }
}

export async function getTodayOverview(page: number) {
  const { data, error, response } = await client.GET("/today/overview", {
    params: { query: { page } },
  });
  if (!response.ok || !data) {
    if (response.status === 401) currentSession.value = null;
    throw new TodayRequestError(
      error?.message ?? "读取今日概览失败",
      response.status,
    );
  }
  return data;
}
