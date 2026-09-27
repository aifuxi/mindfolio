<script setup lang="ts">
import { Button } from "@aifuxi/semi-ui-vue/button";
import { onMounted, ref } from "vue";
import { RouterLink, useRouter } from "vue-router";
import { MarkdownPreview } from "../markdown";
import {
  ReviewRequestError,
  getWeeklyReview,
  type WeeklyReview,
} from "../weekly-review";

const router = useRouter();
const selectedDate = ref("");
const review = ref<WeeklyReview | null>(null);
const page = ref(1);
const error = ref("");

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

async function load() {
  if (!selectedDate.value) return;
  try {
    review.value = await getWeeklyReview(selectedDate.value, page.value);
    error.value = "";
  } catch (cause) {
    if (cause instanceof ReviewRequestError && cause.status === 401)
      void router.replace({ name: "login" });
    error.value = cause instanceof Error ? cause.message : "读取每周回顾失败";
  }
}

function shiftWeek(delta: number) {
  if (!review.value) return;
  const date = new Date(`${review.value.starts_on}T12:00:00Z`);
  date.setUTCDate(date.getUTCDate() + delta * 7);
  selectedDate.value = date.toISOString().slice(0, 10);
  page.value = 1;
  void load();
}

function stateLabel(state: string) {
  return (
    (
      {
        before_creation: "创建前",
        deleted: "已删除",
        future: "未来",
        paused: "暂停",
        completed: "完成",
        incomplete: "未完成",
        missed: "漏打卡",
      } as Record<string, string>
    )[state] ?? state
  );
}

function formatTime(value: string) {
  return new Intl.DateTimeFormat("zh-CN", {
    timeZone: "Asia/Shanghai",
    hour: "2-digit",
    minute: "2-digit",
  }).format(new Date(value));
}

onMounted(() => {
  selectedDate.value = shanghaiToday();
  void load();
});
</script>

<template>
  <main class="page">
    <div class="shell">
      <header>
        <div>
          <p class="eyebrow">Mindfolio / 私人管理</p>
          <h1>每周回顾</h1>
        </div>
        <nav>
          <RouterLink to="/">返回项目</RouterLink
          ><RouterLink to="/today">今日</RouterLink>
        </nav>
      </header>
      <p v-if="error" role="alert" class="error">{{ error }}</p>
      <section class="card">
        <label for="review-date">选择周内日期</label
        ><input
          id="review-date"
          v-model="selectedDate"
          type="date"
          @change="
            page = 1;
            load();
          "
        />
        <div v-if="review" class="actions">
          <Button theme="outline" @click="shiftWeek(-1)">上一周</Button
          ><strong>{{ review.starts_on }} 至 {{ review.ends_on }}</strong
          ><Button theme="outline" @click="shiftWeek(1)">下一周</Button>
        </div>
      </section>
      <template v-if="review">
        <p
          v-if="
            review.completions.length === 0 &&
            review.journals.length === 0 &&
            review.habits.length === 0
          "
          class="empty"
        >
          这一周没有任务完成、习惯或每日记录。
        </p>
        <section class="card">
          <h2>习惯执行</h2>
          <p v-if="review.habits.length === 0" class="subtle">
            这一周没有习惯。
          </p>
          <ul v-else>
            <li v-for="habit in review.habits" :key="habit.id">
              <RouterLink v-if="habit.current_exists" to="/habits">{{
                habit.name
              }}</RouterLink
              ><strong v-else>{{ habit.name }}（已删除）</strong> ·
              {{
                habit.snapshot.rate.percent == null
                  ? "无应执行日"
                  : `${habit.snapshot.rate.earned.toFixed(1)} / ${habit.snapshot.rate.possible.toFixed(1)} · ${habit.snapshot.rate.percent}%`
              }}
            </li>
          </ul>
        </section>
        <section
          v-for="day in review.dates"
          :key="day"
          class="card"
          :aria-label="day"
        >
          <h2>{{ day }}</h2>
          <h3>完成任务</h3>
          <p
            v-if="
              !review.completions.some((item) => item.business_date === day)
            "
            class="subtle"
          >
            无完成记录。
          </p>
          <ul v-else>
            <li
              v-for="item in review.completions.filter(
                (value) => value.business_date === day,
              )"
              :key="item.id"
            >
              <RouterLink
                v-if="item.task_exists"
                :to="{ name: 'tasks', query: { task_id: item.task_id } }"
                >{{ item.task_title }}</RouterLink
              ><strong v-else>{{ item.task_title }}</strong
              ><span v-if="item.project_name"> · {{ item.project_name }}</span>
              · {{ formatTime(item.completed_at) }}
            </li>
          </ul>
          <h3>习惯</h3>
          <p v-if="review.habits.length === 0" class="subtle">无习惯记录。</p>
          <ul v-else>
            <li v-for="habit in review.habits" :key="habit.id">
              {{ habit.name }} ·
              {{
                stateLabel(
                  habit.snapshot.days.find((item) => item.date === day)
                    ?.state ?? "",
                )
              }}<span
                v-if="
                  habit.snapshot.days.find((item) => item.date === day)?.note
                "
              >
                ·
                {{
                  habit.snapshot.days.find((item) => item.date === day)?.note
                }}</span
              >
            </li>
          </ul>
          <h3>每日记录</h3>
          <template
            v-if="review.journals.find((item) => item.business_date === day)"
            ><MarkdownPreview
              :source="
                review.journals.find((item) => item.business_date === day)
                  ?.body ?? ''
              "
              :label="`${day} 每日记录预览`"
            /><RouterLink :to="{ name: 'journal', params: { date: day } }"
              >打开每日记录</RouterLink
            ></template
          >
          <p v-else class="subtle">无每日记录。</p>
        </section>
        <nav
          v-if="
            page > 1 || review.has_more_completions || review.has_more_habits
          "
          class="actions"
        >
          <Button
            :disabled="page === 1"
            @click="
              page--;
              load();
            "
            >上一页</Button
          ><span>第 {{ page }} 页</span
          ><Button
            :disabled="!review.has_more_completions && !review.has_more_habits"
            @click="
              page++;
              load();
            "
            >下一页</Button
          >
        </nav>
      </template>
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
.card,
.empty {
  padding: 1.5rem;
  margin: 1rem 0;
  background: white;
  border: 1px solid #e0e5ec;
  border-radius: 0.8rem;
}
input {
  padding: 0.6rem;
  border: 1px solid #aab3c0;
  border-radius: 0.4rem;
  font: inherit;
  margin: 0.5rem;
}
h3 {
  margin: 1.5rem 0 0.5rem;
}
.subtle,
.empty {
  color: #626c7a;
}
.error {
  padding: 0.8rem;
  background: #fcebed;
  color: #91232a;
  border-radius: 0.4rem;
}
li {
  margin: 0.5rem 0;
}
a {
  color: #354f9b;
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
