import createClient from "openapi-fetch";
import type { components, paths } from "@mindfolio/api-contract";
import { currentSession } from "./auth";

export type JournalDay = components["schemas"]["JournalDay"];
export type Journal = components["schemas"]["JournalResponse"];
type ApiError = components["schemas"]["ApiError"];

const client = createClient<paths>({ baseUrl: "/api" });

export class JournalRequestError extends Error {
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
    throw new JournalRequestError(
      error?.message ?? "每日记录操作失败",
      response.status,
    );
  }
  return data;
}

export async function getJournalDay(date: string) {
  const { data, error, response } = await client.GET("/journal/{date}", {
    params: { path: { date } },
  });
  return result(response, data, error);
}

export async function saveJournal(
  date: string,
  body: string,
  expectedVersion: number | null,
) {
  const token = currentSession.value?.csrf_token;
  if (!token) throw new JournalRequestError("登录已过期", 401);
  const { data, error, response } = await client.PUT("/journal/{date}", {
    params: { path: { date }, header: { "x-csrf-token": token } },
    body: { body, expected_version: expectedVersion },
  });
  return result(response, data, error);
}
