<script setup lang="ts">
import { Button } from "@aifuxi/semi-ui-vue/button";
import { computed, onMounted, ref } from "vue";
import { RouterLink, useRouter } from "vue-router";
import { currentSession } from "../auth";
import { ProjectRequestError, listProjects, type Project } from "../projects";
import { TaskRequestError, listTodayTasks, type TodayTask } from "../tasks";

const router = useRouter();
const projects = ref<Project[]>([]);
const items = ref<TodayTask[]>([]);
const businessDate = ref("");
const page = ref(1);
const hasMore = ref(false);
const error = ref("");

const groups = computed(() => [
  {
    key: "overdue",
    title: "已逾期",
    items: items.value.filter((item) => item.reasons.includes("overdue")),
  },
  {
    key: "due_today",
    title: "今天到期",
    items: items.value.filter((item) => item.reasons.includes("due_today")),
  },
  {
    key: "planned_today",
    title: "计划今天",
    items: items.value.filter(
      (item) =>
        item.reasons.includes("planned_today") &&
        !item.reasons.includes("overdue") &&
        !item.reasons.includes("due_today"),
    ),
  },
]);

function handleError(cause: unknown) {
  if (
    (cause instanceof TaskRequestError ||
      cause instanceof ProjectRequestError) &&
    cause.status === 401
  ) {
    void router.replace({ name: "login" });
  }
  error.value = cause instanceof Error ? cause.message : "读取今日任务失败";
}

async function load() {
  try {
    const result = await listTodayTasks(page.value);
    items.value = result.items;
    businessDate.value = result.business_date;
    hasMore.value = result.has_more;
    error.value = "";
  } catch (cause) {
    handleError(cause);
  }
}

async function loadProjects() {
  try {
    const all: Project[] = [];
    let nextPage = 1;
    while (true) {
      const result = await listProjects(nextPage, true);
      all.push(...result.items);
      if (!result.has_more) break;
      nextPage++;
    }
    projects.value = all;
  } catch (cause) {
    handleError(cause);
  }
}

function projectLabel(item: TodayTask) {
  if (!item.task.project_id) return "收件箱";
  const project = projects.value.find(
    (value) => value.id === item.task.project_id,
  );
  return project ? project.name : "项目任务";
}

function statusLabel(value: string) {
  return value === "in_progress" ? "进行中" : "待办";
}

onMounted(() => {
  void load();
  void loadProjects();
});
</script>

<template>
  <main class="page">
    <div class="shell">
      <header class="header">
        <div>
          <p class="eyebrow">Mindfolio / 私人管理</p>
          <h1>今日任务</h1>
          <p class="subtle">已登录：{{ currentSession?.username }}</p>
        </div>
        <nav class="links" aria-label="返回">
          <RouterLink to="/">返回项目</RouterLink>
          <RouterLink to="/tasks">查看全部任务</RouterLink>
        </nav>
      </header>
      <p class="intro">
        {{ businessDate }} · 计划今天、今天到期或已逾期的待处理任务
      </p>
      <p v-if="error" class="message error" role="alert">{{ error }}</p>
      <p v-if="items.length === 0 && !error" class="empty">
        今天没有需要处理的任务。
      </p>
      <section
        v-for="group in groups.filter((value) => value.items.length > 0)"
        :key="group.key"
        class="group"
        :aria-label="group.title"
      >
        <h2>
          {{ group.title }} <span>{{ group.items.length }}</span>
        </h2>
        <ul class="task-list">
          <li v-for="item in group.items" :key="item.task.id" class="task-card">
            <div class="task-main">
              <h3>{{ item.task.title }}</h3>
              <div class="meta">
                <span>{{ projectLabel(item) }}</span>
                <span v-if="item.task.parent_id">子任务</span>
                <span v-if="item.task.in_backlog">待规划区</span>
                <span>{{ statusLabel(item.task.status) }}</span>
                <span v-if="item.reasons.includes('overdue')" class="overdue"
                  >已逾期 · 截止 {{ item.task.due_date }}</span
                >
                <span v-if="item.reasons.includes('due_today')"
                  >今天到期 · {{ item.task.due_date }}</span
                >
                <span v-if="item.reasons.includes('planned_today')"
                  >计划今天 · {{ item.task.planned_date }}</span
                >
              </div>
            </div>
            <RouterLink
              :to="{
                name: 'tasks',
                query: {
                  project_id: item.task.project_id ?? undefined,
                  task_id: item.task.id,
                },
              }"
              >查看任务</RouterLink
            >
          </li>
        </ul>
      </section>
      <nav
        v-if="page > 1 || hasMore"
        class="pagination"
        aria-label="今日任务分页"
      >
        <Button
          theme="outline"
          :disabled="page === 1"
          @click="
            page--;
            load();
          "
          >上一页</Button
        >
        <span>第 {{ page }} 页</span>
        <Button
          theme="outline"
          :disabled="!hasMore"
          @click="
            page++;
            load();
          "
          >下一页</Button
        >
      </nav>
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
.task-card,
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
  display: flex;
  align-items: center;
  gap: 0.7rem;
  font-size: 1.35rem;
}
h2 span {
  color: #626c7a;
  font-size: 0.95rem;
}
.subtle,
.intro {
  color: #626c7a;
  line-height: 1.5;
}
.intro {
  margin: 1.5rem 0;
}
.group {
  margin-top: 1.5rem;
}
.task-list {
  display: grid;
  gap: 0.75rem;
  margin: 0;
  padding: 0;
  list-style: none;
}
.task-card,
.empty {
  padding: 1.2rem 1.5rem;
  border: 1px solid #e0e5ec;
  border-radius: 0.8rem;
  background: white;
}
.task-main {
  min-width: 0;
}
h3 {
  margin: 0 0 0.5rem;
  overflow-wrap: anywhere;
}
.meta {
  display: flex;
  flex-wrap: wrap;
  gap: 0.5rem;
  color: #626c7a;
  font-size: 0.85rem;
}
.meta span {
  padding: 0.2rem 0.45rem;
  border-radius: 0.3rem;
  background: #eef1f5;
}
.meta .overdue {
  background: #fcebed;
  color: #91232a;
  font-weight: 600;
}
.empty {
  color: #626c7a;
}
.pagination {
  justify-content: center;
  margin-top: 1.5rem;
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
  .task-card {
    align-items: flex-start;
    flex-direction: column;
  }
  .links {
    justify-content: flex-start;
  }
}
</style>
