import createClient from "openapi-fetch";
import type { components, paths } from "@mindfolio/api-contract";
import { currentSession } from "./auth";

export type WeeklyReview = components["schemas"]["WeeklyReview"];
const client = createClient<paths>({ baseUrl: "/api" });

export class ReviewRequestError extends Error {
  constructor(
    message: string,
    public readonly status: number,
  ) {
    super(message);
  }
}

export async function getWeeklyReview(date: string, page: number) {
  const { data, error, response } = await client.GET("/weekly-review", {
    params: { query: { date, page } },
  });
  if (!response.ok || !data) {
    if (response.status === 401) currentSession.value = null;
    throw new ReviewRequestError(
      error?.message ?? "读取每周回顾失败",
      response.status,
    );
  }
  return data;
}
