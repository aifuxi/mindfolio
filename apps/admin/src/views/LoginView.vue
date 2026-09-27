<script setup lang="ts">
import { ref } from "vue";
import { useRouter } from "vue-router";
import { signIn } from "../auth";

const router = useRouter();
const username = ref("");
const password = ref("");
const busy = ref(false);
const error = ref("");

async function submit() {
  if (busy.value) return;
  busy.value = true;
  error.value = "";
  try {
    await signIn(username.value, password.value);
    password.value = "";
    await router.replace({ name: "home" });
  } catch (cause) {
    error.value =
      cause instanceof Error ? cause.message : "登录失败，请稍后再试";
  } finally {
    busy.value = false;
  }
}
</script>

<template>
  <main class="page">
    <section class="card" aria-labelledby="title">
      <p class="eyebrow">Mindfolio</p>
      <h1 id="title">管理者登录</h1>
      <p class="hint">使用服务器已初始化的账号访问私人管理端。</p>
      <form @submit.prevent="submit">
        <label for="username">账号</label>
        <input
          id="username"
          v-model="username"
          name="username"
          autocomplete="username"
          required
        />
        <label for="password">密码</label>
        <input
          id="password"
          v-model="password"
          name="password"
          type="password"
          autocomplete="current-password"
          required
        />
        <p v-if="error" class="error" role="alert">{{ error }}</p>
        <button type="submit" :disabled="busy">
          {{ busy ? "登录中…" : "登录" }}
        </button>
      </form>
    </section>
  </main>
</template>

<style scoped>
.page {
  min-height: 100vh;
  display: grid;
  place-items: center;
  padding: 1.5rem;
  background: #f5f6f8;
  color: #21252b;
}
.card {
  width: min(100%, 26rem);
  padding: 2.5rem;
  background: white;
  border: 1px solid #e4e7ec;
  border-radius: 1rem;
  box-shadow: 0 1rem 3rem #1f29370d;
}
.eyebrow {
  color: #4c64ad;
  font-weight: 700;
  letter-spacing: 0.06em;
}
h1 {
  margin: 0.5rem 0;
  font-size: 1.7rem;
}
.hint {
  color: #626c7a;
  line-height: 1.6;
}
form {
  display: grid;
  gap: 0.65rem;
  margin-top: 1.8rem;
}
label {
  font-weight: 600;
}
input {
  height: 2.75rem;
  padding: 0 0.75rem;
  border: 1px solid #aab3c0;
  border-radius: 0.5rem;
  margin-bottom: 0.5rem;
  font: inherit;
}
input:focus-visible,
button:focus-visible {
  outline: 2px solid #4c64ad;
  outline-offset: 2px;
}
button {
  height: 2.8rem;
  margin-top: 0.5rem;
  border: 0;
  border-radius: 0.5rem;
  background: #354f9b;
  color: white;
  font: inherit;
  font-weight: 600;
  cursor: pointer;
}
button:disabled {
  opacity: 0.65;
  cursor: wait;
}
.error {
  color: #a7282f;
}
</style>
