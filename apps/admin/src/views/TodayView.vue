<script setup lang="ts">
import { Button } from "@aifuxi/semi-ui-vue/button";
import { computed, onMounted, ref } from "vue";
import { RouterLink, useRouter } from "vue-router";
import { currentSession } from "../auth";
import { HabitRequestError, saveHabitCheckin } from "../habits";
import { ProjectRequestError, listProjects, type Project } from "../projects";
import { TaskRequestError, listTodayTasks, type TodayTask } from "../tasks";
import { TodayRequestError, getTodayOverview, type TodayHabit } from "../today";

const router = useRouter();
const projects = ref<Project[]>([]);
const items = ref<TodayTask[]>([]);
const businessDate = ref("");
const page = ref(1);
const hasMore = ref(false);
const error = ref("");
const habits = ref<TodayHabit[]>([]);
const habitPage = ref(1);
const hasMoreHabits = ref(false);
const journalExists = ref(false);
const editingHabitId = ref<string | null>(null);
const habitCompleted = ref(true);
const habitNote = ref("");
const habitVersion = ref<number | null>(null);
const busyHabit = ref(false);
const notice = ref("");

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
      cause instanceof ProjectRequestError ||
      cause instanceof HabitRequestError ||
      cause instanceof TodayRequestError) &&
    cause.status === 401
  ) {
    void router.replace({ name: "login" });
  }
  error.value = cause instanceof Error ? cause.message : "读取今日任务失败";
}

async function loadOverview() {
  try {
    const result = await getTodayOverview(habitPage.value);
    habits.value = result.habits;
    hasMoreHabits.value = result.has_more;
    journalExists.value = result.journal_exists;
    businessDate.value = result.business_date;
    error.value = "";
  } catch (cause) {
    handleError(cause);
  }
}

function editHabit(habit: TodayHabit) {
  editingHabitId.value = habit.id;
  habitCompleted.value = habit.checkin_completed ?? true;
  habitNote.value = habit.checkin_note ?? "";
  habitVersion.value = habit.checkin_version ?? null;
  notice.value = "";
}

async function saveTodayHabit(habit: TodayHabit) {
  if (busyHabit.value || !businessDate.value) return;
  busyHabit.value = true;
  try {
    await saveHabitCheckin(
      habit.id,
      businessDate.value,
      habitCompleted.value,
      habitNote.value,
      habitVersion.value,
    );
    editingHabitId.value = null;
    notice.value = "今日习惯打卡已保存";
    await loadOverview();
  } catch (cause) {
    handleError(cause);
  } finally {
    busyHabit.value = false;
  }
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
  void loadOverview();
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
          <RouterLink to="/habits">管理习惯</RouterLink>
        </nav>
      </header>
      <p class="intro">
        {{ businessDate }} · 计划今天、今天到期或已逾期的待处理任务
      </p>
      <p v-if="error" class="message error" role="alert">{{ error }}</p>
      <p v-if="notice" class="message success" role="status">{{ notice }}</p>
      <section class="group" aria-label="今日习惯">
        <h2>
          今日习惯 <span>{{ habits.length }}</span>
        </h2>
        <p v-if="habits.length === 0" class="empty">今天没有需要执行的习惯。</p>
        <ul v-else class="task-list">
          <li v-for="habit in habits" :key="habit.id" class="task-card">
            <div class="task-main">
              <h3>{{ habit.name }}</h3>
              <p class="meta">
                <span>{{
                  habit.cadence === "daily"
                    ? "每天"
                    : `每周 ${habit.weekly_target} 天`
                }}</span
                ><span v-if="habit.checkin_completed === true">今天已完成</span
                ><span v-else-if="habit.needs_checkin" class="pending"
                  >待打卡</span
                ><span v-else>本周目标已达成</span>
              </p>
            </div>
            <Button theme="outline" @click="editHabit(habit)">{{
              habit.checkin_version ? "更正打卡" : "打卡"
            }}</Button>
            <form
              v-if="editingHabitId === habit.id"
              class="habit-form"
              @submit.prevent="saveTodayHabit(habit)"
            >
              <label
                ><input v-model="habitCompleted" type="checkbox" />已完成</label
              >
              <label :for="`habit-note-${habit.id}`">备注</label
              ><input
                :id="`habit-note-${habit.id}`"
                v-model="habitNote"
                maxlength="2000"
              />
              <Button
                html-type="submit"
                type="primary"
                theme="solid"
                :disabled="busyHabit"
                >保存打卡</Button
              >
            </form>
          </li>
        </ul>
        <nav v-if="habitPage > 1 || hasMoreHabits" class="pagination">
          <Button
            :disabled="habitPage === 1"
            @click="
              habitPage--;
              loadOverview();
            "
            >上一页习惯</Button
          ><span>第 {{ habitPage }} 页</span
          ><Button
            :disabled="!hasMoreHabits"
            @click="
              habitPage++;
              loadOverview();
            "
            >下一页习惯</Button
          >
        </nav>
      </section>
      <section class="group" aria-label="今日记录">
        <h2>今日记录</h2>
        <div class="task-card">
          <p>
            {{
              journalExists
                ? "今天已有每日记录，可继续编辑。"
                : "今天还没有每日记录。"
            }}
          </p>
          <RouterLink
            :to="{ name: 'journal', params: { date: businessDate } }"
            >{{ journalExists ? "编辑每日记录" : "写每日记录" }}</RouterLink
          >
        </div>
      </section>
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
.meta .pending {
  background: #fff1d6;
  color: #7b4d00;
  font-weight: 600;
}
.habit-form {
  display: grid;
  gap: 0.5rem;
  min-width: 15rem;
}
.habit-form input:not([type="checkbox"]) {
  padding: 0.5rem;
  border: 1px solid #aab3c0;
  border-radius: 0.4rem;
  font: inherit;
}
.success {
  background: #e4f4e9;
  color: #24613b;
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
