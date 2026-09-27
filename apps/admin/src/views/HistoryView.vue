<script setup lang="ts">
import { Button } from "@aifuxi/semi-ui-vue/button";
import { onMounted, ref } from "vue";
import { RouterLink, useRouter } from "vue-router";
import { currentSession } from "../auth";
import {
  CompletionRequestError,
  listCompletionDays,
  listCompletions,
  type Completion,
  type DaySummary,
} from "../completions";

const router = useRouter();
const days = ref<DaySummary[]>([]);
const entries = ref<Completion[]>([]);
const dayPage = ref(1);
const entryPage = ref(1);
const hasMoreDays = ref(false);
const hasMoreEntries = ref(false);
const selectedDate = ref("");
const error = ref("");

function handleError(cause: unknown) {
  if (cause instanceof CompletionRequestError && cause.status === 401) {
    void router.replace({ name: "login" });
  }
  error.value = cause instanceof Error ? cause.message : "读取完成历史失败";
}

async function loadDays() {
  try {
    const result = await listCompletionDays(dayPage.value);
    days.value = result.items;
    hasMoreDays.value = result.has_more;
    error.value = "";
  } catch (cause) {
    handleError(cause);
  }
}

async function loadEntries() {
  if (!selectedDate.value) return;
  try {
    const result = await listCompletions(selectedDate.value, entryPage.value);
    entries.value = result.items;
    hasMoreEntries.value = result.has_more;
    error.value = "";
  } catch (cause) {
    handleError(cause);
  }
}

async function selectDay(date: string) {
  selectedDate.value = date;
  entryPage.value = 1;
  await loadEntries();
}

function formatTime(value: string) {
  return new Intl.DateTimeFormat("zh-CN", {
    timeZone: "Asia/Shanghai",
    dateStyle: "medium",
    timeStyle: "short",
  }).format(new Date(value));
}

onMounted(() => void loadDays());
</script>

<template>
  <main class="page">
    <div class="shell">
      <header class="header">
        <div>
          <p class="eyebrow">Mindfolio / 私人管理</p>
          <h1>任务完成历史</h1>
          <p class="subtle">已登录：{{ currentSession?.username }}</p>
        </div>
        <nav class="links" aria-label="返回">
          <RouterLink to="/">返回项目</RouterLink>
          <RouterLink to="/tasks">查看任务</RouterLink>
        </nav>
      </header>
      <p class="subtle">
        按 Asia/Shanghai
        日期汇总完成次数；标题和项目名称为完成当时的快照。删除当前任务或项目后，这些记录仍会保留。
      </p>
      <p v-if="error" class="message error" role="alert">{{ error }}</p>
      <div class="columns">
        <section class="card" aria-labelledby="days-title">
          <h2 id="days-title">每日汇总</h2>
          <p v-if="days.length === 0" class="subtle">还没有完成记录。</p>
          <ul v-else class="list">
            <li v-for="day in days" :key="day.business_date">
              <Button
                :theme="
                  selectedDate === day.business_date ? 'solid' : 'borderless'
                "
                @click="selectDay(day.business_date)"
                >{{ day.business_date }} ·
                {{ day.completed_count }} 次完成</Button
              >
            </li>
          </ul>
          <nav
            v-if="dayPage > 1 || hasMoreDays"
            class="pagination"
            aria-label="汇总分页"
          >
            <Button
              theme="outline"
              :disabled="dayPage === 1"
              @click="
                dayPage--;
                loadDays();
              "
              >上一页</Button
            >
            <span>第 {{ dayPage }} 页</span>
            <Button
              theme="outline"
              :disabled="!hasMoreDays"
              @click="
                dayPage++;
                loadDays();
              "
              >下一页</Button
            >
          </nav>
        </section>
        <section class="card" aria-labelledby="entries-title">
          <h2 id="entries-title">
            {{ selectedDate || "选择日期查看完成记录" }}
          </h2>
          <p v-if="selectedDate && entries.length === 0" class="subtle">
            这一天没有完成记录。
          </p>
          <ul v-else class="list entries">
            <li v-for="entry in entries" :key="entry.id">
              <strong>{{ entry.task_title }}</strong>
              <span v-if="entry.project_name">{{ entry.project_name }}</span>
              <time :datetime="entry.completed_at">{{
                formatTime(entry.completed_at)
              }}</time>
            </li>
          </ul>
          <nav
            v-if="selectedDate && (entryPage > 1 || hasMoreEntries)"
            class="pagination"
            aria-label="记录分页"
          >
            <Button
              theme="outline"
              :disabled="entryPage === 1"
              @click="
                entryPage--;
                loadEntries();
              "
              >上一页</Button
            >
            <span>第 {{ entryPage }} 页</span>
            <Button
              theme="outline"
              :disabled="!hasMoreEntries"
              @click="
                entryPage++;
                loadEntries();
              "
              >下一页</Button
            >
          </nav>
        </section>
      </div>
    </div>
  </main>
</template>

<style scoped>
.page {
  min-height: 100vh;
  padding: 2rem 1.5rem 5rem;
  background: #f5f6f8;
  color: #21252b;
}
.shell {
  max-width: 70rem;
  margin: 0 auto;
}
.header,
.links,
.pagination {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 1rem;
}
.header {
  padding-bottom: 1.5rem;
  border-bottom: 1px solid #dce1e9;
}
.links {
  justify-content: flex-end;
}
a {
  color: #354f9b;
  font-weight: 600;
  text-decoration: none;
}
a:hover {
  text-decoration: underline;
}
.eyebrow {
  color: #4c64ad;
  font-size: 0.8rem;
  font-weight: 700;
  letter-spacing: 0.08em;
}
h1 {
  margin: 0.4rem 0;
  font-size: 2.15rem;
}
h2 {
  margin: 0 0 1rem;
  font-size: 1.35rem;
}
.subtle {
  color: #626c7a;
  line-height: 1.5;
}
.columns {
  display: grid;
  grid-template-columns: minmax(15rem, 1fr) minmax(20rem, 2fr);
  gap: 1rem;
  margin-top: 1.5rem;
}
.card {
  padding: 1.5rem;
  border: 1px solid #e0e5ec;
  border-radius: 0.8rem;
  background: white;
}
.list {
  display: grid;
  gap: 0.7rem;
  margin: 0;
  padding: 0;
  list-style: none;
}
.entries li {
  display: grid;
  gap: 0.2rem;
  padding: 0.75rem;
  border-bottom: 1px solid #e0e5ec;
}
.entries span,
.entries time {
  color: #626c7a;
}
.pagination {
  justify-content: center;
  margin-top: 1.25rem;
}
.message {
  padding: 0.8rem 1rem;
  border-radius: 0.5rem;
}
.error {
  background: #fcebed;
  color: #91232a;
}
@media (max-width: 700px) {
  .page {
    padding: 1.25rem 1rem 3rem;
  }
  .header,
  .columns {
    display: block;
  }
  .links {
    justify-content: flex-start;
  }
  .card {
    margin-top: 1rem;
  }
}
</style>
