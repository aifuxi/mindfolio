import createClient from "openapi-fetch";
import { shallowRef } from "vue";
import type { components, paths } from "@mindfolio/api-contract";

type Session = components["schemas"]["SessionResponse"];

const client = createClient<paths>({ baseUrl: "/api" });

export const currentSession = shallowRef<Session | null>(null);

export async function refreshSession(): Promise<Session | null> {
  try {
    const { data, response } = await client.GET("/auth/session");
    currentSession.value = response.ok ? (data ?? null) : null;
  } catch {
    currentSession.value = null;
  }
  return currentSession.value;
}

export async function signIn(
  username: string,
  password: string,
): Promise<Session> {
  const { data, error, response } = await client.POST("/auth/login", {
    body: { username, password },
  });
  if (!response.ok || !data) {
    throw new Error(error?.message ?? "登录失败，请稍后再试");
  }
  currentSession.value = data;
  return data;
}

export async function signOut(): Promise<void> {
  const csrf = currentSession.value?.csrf_token;
  if (!csrf) {
    currentSession.value = null;
    return;
  }
  const { error, response } = await client.POST("/auth/logout", {
    params: { header: { "x-csrf-token": csrf } },
  });
  if (!response.ok && response.status !== 401) {
    throw new Error(error?.message ?? "退出失败，请稍后再试");
  }
  currentSession.value = null;
}
