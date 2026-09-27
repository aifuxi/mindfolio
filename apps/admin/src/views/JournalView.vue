<script setup lang="ts">
import { Button } from "@aifuxi/semi-ui-vue/button";
import { onMounted, ref } from "vue";
import { RouterLink, useRoute, useRouter } from "vue-router";
import {
  JournalRequestError,
  getJournalDay,
  saveJournal,
  type JournalDay,
} from "../journal";
import { MarkdownPreview } from "../markdown";

const route = useRoute();
const router = useRouter();
const date = ref("");
const day = ref<JournalDay | null>(null);
const body = ref("");
const version = ref<number | null>(null);
const preview = ref(false);
const busy = ref(false);
const error = ref("");
const notice = ref("");

function shanghaiToday() {
  const parts = new Intl.DateTimeFormat("en", {
    timeZone: "Asia/Shanghai",
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
  }).formatToParts(new Date());
  return ["year", "month", "day"]
    .map((key) => parts.find((part) => part.type === key)?.value)
    .join("-");
}

function handleError(cause: unknown) {
  if (cause instanceof JournalRequestError && cause.status === 401)
    void router.replace({ name: "login" });
  error.value = cause instanceof Error ? cause.message : "读取每日记录失败";
}

async function load() {
  if (!date.value) return;
  try {
    const result = await getJournalDay(date.value);
    day.value = result;
    body.value = result.journal?.body ?? "";
    version.value = result.journal?.version ?? null;
    error.value = "";
    notice.value = "";
    await router.replace({ name: "journal", params: { date: date.value } });
  } catch (cause) {
    handleError(cause);
  }
}

async function save() {
  if (busy.value) return;
  busy.value = true;
  notice.value = "";
  try {
    const saved = await saveJournal(date.value, body.value, version.value);
    version.value = saved.version;
    day.value = await getJournalDay(date.value);
    error.value = "";
    notice.value = "每日记录已保存";
  } catch (cause) {
    handleError(cause);
  } finally {
    busy.value = false;
  }
}

function formatTime(value: string) {
  return new Intl.DateTimeFormat("zh-CN", {
    timeZone: "Asia/Shanghai",
    hour: "2-digit",
    minute: "2-digit",
  }).format(new Date(value));
}

onMounted(() => {
  date.value =
    typeof route.params.date === "string" ? route.params.date : shanghaiToday();
  void load();
});
</script>

<template>
  <main class="page">
    <div class="shell">
      <header>
        <div>
          <p class="eyebrow">Mindfolio / 私人管理</p>
          <h1>每日记录</h1>
        </div>
        <nav>
          <RouterLink to="/">返回项目</RouterLink
          ><RouterLink to="/today">今日</RouterLink>
        </nav>
      </header>
      <p v-if="error" role="alert" class="error">
        {{ error
        }}<span v-if="error.includes('变更')"
          >；编辑内容仍保留，请核对后重试。</span
        >
      </p>
      <p v-if="notice" role="status" class="success">{{ notice }}</p>
      <section class="card">
        <label for="journal-date">业务日期</label
        ><input id="journal-date" v-model="date" type="date" @change="load" />
        <p v-if="day && !day.journal" class="subtle">这一天还没有每日记录。</p>
        <form class="form" @submit.prevent="save">
          <label for="journal-body">Markdown 正文</label
          ><textarea
            id="journal-body"
            v-model="body"
            rows="12"
            maxlength="100000"
            required
          />
          <div class="actions">
            <Button theme="outline" @click="preview = !preview">{{
              preview ? "隐藏预览" : "安全预览"
            }}</Button
            ><Button
              html-type="submit"
              type="primary"
              theme="solid"
              :disabled="busy"
              >保存记录</Button
            >
          </div>
        </form>
        <MarkdownPreview v-if="preview" :source="body" label="每日记录预览" />
      </section>
      <section class="card">
        <h2>当天已完成任务</h2>
        <p v-if="!day || day.completions.length === 0" class="subtle">
          这一天没有任务完成记录。
        </p>
        <ul v-else>
          <li v-for="item in day.completions" :key="item.id">
            <strong>{{ item.task_title }}</strong
            ><span v-if="item.project_name"> · {{ item.project_name }}</span> ·
            {{ formatTime(item.completed_at) }}
          </li>
        </ul>
      </section>
      <section class="card">
        <h2>当天习惯打卡</h2>
        <p v-if="!day || day.checkins.length === 0" class="subtle">
          这一天没有习惯打卡记录。
        </p>
        <ul v-else>
          <li v-for="item in day.checkins" :key="item.id">
            {{ item.name }} · {{ item.completed ? "完成" : "未完成"
            }}<span v-if="item.note"> · {{ item.note }}</span>
          </li>
        </ul>
      </section>
    </div>
  </main>
</template>

<style scoped>
.page {
  min-height: 100vh;
  padding: 2rem 1.5rem;
  background: #f5f6f8;
  color: #21252b;
}
.shell {
  max-width: 70rem;
  margin: auto;
}
header,
nav,
.actions {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 1rem;
  flex-wrap: wrap;
}
header {
  border-bottom: 1px solid #dce1e9;
  margin-bottom: 1.5rem;
}
nav {
  justify-content: flex-end;
}
a {
  color: #354f9b;
  font-weight: 600;
}
.eyebrow {
  color: #4c64ad;
  font-weight: 700;
}
.card {
  padding: 1.5rem;
  margin: 1rem 0;
  background: white;
  border: 1px solid #e0e5ec;
  border-radius: 0.8rem;
}
.form {
  display: grid;
  gap: 0.6rem;
  margin-top: 1rem;
}
input,
textarea {
  padding: 0.6rem;
  border: 1px solid #aab3c0;
  border-radius: 0.4rem;
  font: inherit;
}
textarea {
  width: 100%;
  box-sizing: border-box;
}
.subtle {
  color: #626c7a;
}
.error,
.success {
  padding: 0.8rem;
  border-radius: 0.4rem;
}
.error {
  background: #fcebed;
  color: #91232a;
}
.success {
  background: #e4f4e9;
  color: #24613b;
}
li {
  margin: 0.5rem 0;
}
@media (max-width: 700px) {
  .page {
    padding: 1rem;
  }
  header {
    display: block;
  }
  nav {
    justify-content: flex-start;
  }
}
</style>
