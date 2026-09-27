<script setup lang="ts">
import { onMounted, onUnmounted, ref } from "vue";
import { useRouter } from "vue-router";
import { currentSession, refreshSession, signOut } from "../auth";

const router = useRouter();
const error = ref("");
const busy = ref(false);
let interval: ReturnType<typeof setInterval> | undefined;

async function checkSession() {
  if (!(await refreshSession())) {
    await router.replace({ name: "login" });
  }
}

onMounted(() => {
  interval = setInterval(() => void checkSession(), 60_000);
  document.addEventListener("visibilitychange", onVisibilityChange);
});

onUnmounted(() => {
  if (interval) clearInterval(interval);
  document.removeEventListener("visibilitychange", onVisibilityChange);
});

function onVisibilityChange() {
  if (document.visibilityState === "visible") void checkSession();
}

async function logout() {
  if (busy.value) return;
  busy.value = true;
  error.value = "";
  try {
    await signOut();
    await router.replace({ name: "login" });
  } catch (cause) {
    error.value = cause instanceof Error ? cause.message : "退出失败";
  } finally {
    busy.value = false;
  }
}
</script>

<template>
  <main class="page">
    <section class="card">
      <p class="eyebrow">Mindfolio</p>
      <h1>私人管理端</h1>
      <p>已登录：{{ currentSession?.username }}</p>
      <p class="hint">会话由服务器验证。私人功能将在后续任务加入。</p>
      <p v-if="error" role="alert" class="error">{{ error }}</p>
      <button type="button" :disabled="busy" @click="logout">
        {{ busy ? "退出中…" : "退出登录" }}
      </button>
    </section>
  </main>
</template>

<style scoped>
.page {
  min-height: 100vh;
  padding: 4rem 1.5rem;
  background: #f5f6f8;
  color: #21252b;
}
.card {
  max-width: 48rem;
  margin: auto;
  padding: 2.5rem;
  border: 1px solid #e4e7ec;
  border-radius: 1rem;
  background: white;
}
.eyebrow {
  color: #4c64ad;
  font-weight: 700;
}
.hint {
  color: #626c7a;
}
.error {
  color: #a7282f;
}
button {
  padding: 0.7rem 1rem;
  border: 0;
  border-radius: 0.5rem;
  background: #354f9b;
  color: white;
  font: inherit;
  cursor: pointer;
}
button:focus-visible {
  outline: 2px solid #354f9b;
  outline-offset: 2px;
}
</style>
