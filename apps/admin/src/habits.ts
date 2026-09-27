import createClient from "openapi-fetch";
import type { components, paths } from "@mindfolio/api-contract";
import { currentSession } from "./auth";

export type Habit = components["schemas"]["HabitResponse"];
export type HabitDetail = components["schemas"]["HabitDetail"];
export type HabitInput = components["schemas"]["HabitInput"];
export type HabitCalendar = components["schemas"]["CalendarResponse"];
type ApiError = components["schemas"]["ApiError"];

const client = createClient<paths>({ baseUrl: "/api" });

export class HabitRequestError extends Error {
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
    throw new HabitRequestError(
      error?.message ?? "习惯操作失败",
      response.status,
    );
  }
  return data;
}

function csrf() {
  const token = currentSession.value?.csrf_token;
  if (!token) throw new HabitRequestError("登录已过期", 401);
  return token;
}

export async function listHabits(page = 1, includeDeleted = false) {
  const { data, error, response } = await client.GET("/habits", {
    params: { query: { page, include_deleted: includeDeleted } },
  });
  return result(response, data, error);
}

export async function getHabit(id: string) {
  const { data, error, response } = await client.GET("/habits/{id}", {
    params: { path: { id } },
  });
  return result(response, data, error);
}

export async function createHabit(body: HabitInput) {
  const { data, error, response } = await client.POST("/habits", {
    params: { header: { "x-csrf-token": csrf() } },
    body,
  });
  return result(response, data, error);
}

export async function updateHabit(habit: Habit, body: HabitInput) {
  const { data, error, response } = await client.PATCH("/habits/{id}", {
    params: { path: { id: habit.id }, header: { "x-csrf-token": csrf() } },
    body: { ...body, expected_version: habit.version },
  });
  return result(response, data, error);
}

export async function changeHabit(
  habit: Habit,
  action: "pause" | "resume" | "delete",
) {
  const params = { path: { id: habit.id }, header: { "x-csrf-token": csrf() } };
  const body = { expected_version: habit.version };
  const outcome =
    action === "pause"
      ? await client.POST("/habits/{id}/pause", { params, body })
      : action === "resume"
        ? await client.POST("/habits/{id}/resume", { params, body })
        : await client.DELETE("/habits/{id}", { params, body });
  return result(outcome.response, outcome.data, outcome.error);
}

export async function getHabitCalendar(id: string, month: string) {
  const { data, error, response } = await client.GET("/habits/{id}/calendar", {
    params: { path: { id }, query: { month } },
  });
  return result(response, data, error);
}

export async function listHabitCheckins(id: string, from: string, to: string) {
  const { data, error, response } = await client.GET("/habits/{id}/checkins", {
    params: { path: { id }, query: { from, to } },
  });
  return result(response, data, error);
}

export async function saveHabitCheckin(
  id: string,
  date: string,
  completed: boolean,
  note: string,
  expectedVersion: number | null,
) {
  const { data, error, response } = await client.PUT(
    "/habits/{id}/checkins/{date}",
    {
      params: { path: { id, date }, header: { "x-csrf-token": csrf() } },
      body: { completed, note, expected_version: expectedVersion },
    },
  );
  return result(response, data, error);
}
