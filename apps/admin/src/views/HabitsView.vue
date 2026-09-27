<script setup lang="ts">
import { Button } from "@aifuxi/semi-ui-vue/button";
import { onMounted, ref } from "vue";
import { RouterLink, useRouter } from "vue-router";
import {
  HabitRequestError,
  changeHabit,
  createHabit,
  getHabit,
  getHabitCalendar,
  listHabits,
  saveHabitCheckin,
  updateHabit,
  type Habit,
  type HabitCalendar,
  type HabitDetail,
  type HabitInput,
} from "../habits";

const router = useRouter();
const habits = ref<Habit[]>([]);
const selected = ref<HabitDetail | null>(null);
const editing = ref<Habit | null>(null);
const name = ref("");
const cadence = ref<"daily" | "weekly">("daily");
const target = ref(3);
const page = ref(1);
const hasMore = ref(false);
const includeDeleted = ref(false);
const busy = ref(false);
const error = ref("");
const notice = ref("");
const calendar = ref<HabitCalendar | null>(null);
const calendarMonth = ref("");
const selectedDate = ref("");
const checkinCompleted = ref(true);
const checkinNote = ref("");
const checkinVersion = ref<number | null>(null);

function handleError(cause: unknown) {
  if (cause instanceof HabitRequestError && cause.status === 401)
    void router.replace({ name: "login" });
  error.value = cause instanceof Error ? cause.message : "习惯操作失败";
}

async function load() {
  try {
    const data = await listHabits(page.value, includeDeleted.value);
    habits.value = data.items;
    hasMore.value = data.has_more;
    error.value = "";
  } catch (cause) {
    handleError(cause);
  }
}

function edit(habit: Habit) {
  editing.value = habit;
  name.value = habit.name;
  cadence.value = habit.cadence as "daily" | "weekly";
  target.value = habit.weekly_target ?? 3;
  error.value = "";
}

function resetForm() {
  editing.value = null;
  name.value = "";
  cadence.value = "daily";
  target.value = 3;
}

async function save() {
  if (busy.value) return;
  busy.value = true;
  error.value = "";
  notice.value = "";
  const body: HabitInput = {
    name: name.value,
    cadence: cadence.value,
    weekly_target: cadence.value === "weekly" ? target.value : null,
  };
  try {
    const editedId = editing.value?.id;
    if (editing.value) await updateHabit(editing.value, body);
    else await createHabit(body);
    resetForm();
    await load();
    if (editedId && selected.value?.habit.id === editedId) {
      selected.value = await getHabit(editedId);
      if (calendar.value) await loadCalendar();
    }
    notice.value = "习惯已保存";
  } catch (cause) {
    handleError(cause);
  } finally {
    busy.value = false;
  }
}

async function transition(habit: Habit, action: "pause" | "resume" | "delete") {
  if (busy.value) return;
  busy.value = true;
  error.value = "";
  try {
    await changeHabit(habit, action);
    await load();
    if (selected.value?.habit.id === habit.id) {
      selected.value = await getHabit(habit.id);
      if (calendar.value) await loadCalendar();
    }
    notice.value =
      action === "delete" ? "习惯已从当前列表移除，历史仍保留" : "状态已更新";
  } catch (cause) {
    handleError(cause);
  } finally {
    busy.value = false;
  }
}

async function showHistory(id: string) {
  try {
    selected.value = await getHabit(id);
    calendar.value = null;
    error.value = "";
  } catch (cause) {
    handleError(cause);
  }
}

async function openCalendar(habit: Habit) {
  try {
    selected.value = await getHabit(habit.id);
    const parts = new Intl.DateTimeFormat("en", {
      timeZone: "Asia/Shanghai",
      year: "numeric",
      month: "2-digit",
    }).formatToParts(new Date());
    calendarMonth.value = `${parts.find((part) => part.type === "year")?.value}-${parts.find((part) => part.type === "month")?.value}`;
    await loadCalendar();
  } catch (cause) {
    handleError(cause);
  }
}

async function loadCalendar() {
  if (!selected.value || !calendarMonth.value) return;
  try {
    calendar.value = await getHabitCalendar(
      selected.value.habit.id,
      calendarMonth.value,
    );
    selectedDate.value = "";
    error.value = "";
  } catch (cause) {
    handleError(cause);
  }
}

function chooseDay(day: HabitCalendar["days"][number]) {
  selectedDate.value = day.date;
  checkinCompleted.value = day.state === "completed";
  checkinNote.value = day.note ?? "";
  checkinVersion.value = day.checkin_version ?? null;
}

async function saveCheckin() {
  if (!selected.value || !selectedDate.value || busy.value) return;
  busy.value = true;
  try {
    await saveHabitCheckin(
      selected.value.habit.id,
      selectedDate.value,
      checkinCompleted.value,
      checkinNote.value,
      checkinVersion.value,
    );
    await loadCalendar();
    notice.value = "打卡已保存";
  } catch (cause) {
    handleError(cause);
  } finally {
    busy.value = false;
  }
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

function rateLabel(
  earned: number,
  possible: number,
  percent: number | null | undefined,
) {
  return percent == null
    ? "无应执行日"
    : `${earned.toFixed(1)} / ${possible.toFixed(1)} · ${percent}%`;
}

onMounted(() => void load());
</script>

<template>
  <main class="page">
    <div class="shell">
      <header>
        <div>
          <p class="eyebrow">Mindfolio / 私人管理</p>
          <h1>习惯</h1>
        </div>
        <nav>
          <RouterLink to="/">返回项目</RouterLink
          ><RouterLink to="/today">今日</RouterLink>
        </nav>
      </header>
      <p v-if="error" role="alert" class="error">{{ error }}</p>
      <p v-if="notice" role="status" class="success">{{ notice }}</p>
      <section class="card">
        <h2>{{ editing ? "编辑习惯" : "新建习惯" }}</h2>
        <form class="form" @submit.prevent="save">
          <label for="habit-name">名称</label
          ><input id="habit-name" v-model="name" required maxlength="120" />
          <label for="habit-cadence">目标</label
          ><select id="habit-cadence" v-model="cadence">
            <option value="daily">每天执行</option>
            <option value="weekly">每周指定天数</option>
          </select>
          <template v-if="cadence === 'weekly'"
            ><label for="habit-target">每周天数</label
            ><input
              id="habit-target"
              v-model.number="target"
              type="number"
              min="1"
              max="7"
              required
          /></template>
          <div class="actions">
            <Button
              html-type="submit"
              type="primary"
              theme="solid"
              :disabled="busy"
              >保存</Button
            ><Button v-if="editing" theme="outline" @click="resetForm"
              >取消编辑</Button
            >
          </div>
        </form>
      </section>
      <section class="card">
        <div class="actions">
          <h2>习惯列表</h2>
          <label
            ><input
              v-model="includeDeleted"
              type="checkbox"
              @change="
                page = 1;
                load();
              "
            />显示已删除</label
          >
        </div>
        <p v-if="habits.length === 0">还没有习惯。</p>
        <ul v-else class="list">
          <li v-for="habit in habits" :key="habit.id">
            <strong>{{ habit.name }}</strong> ·
            {{
              habit.cadence === "daily"
                ? "每天"
                : `每周 ${habit.weekly_target} 天`
            }}
            <span v-if="habit.paused"> · 已暂停</span
            ><span v-if="habit.deleted_at"> · 已删除</span>
            <div class="actions">
              <Button theme="borderless" @click="showHistory(habit.id)"
                >查看历史</Button
              ><Button theme="borderless" @click="openCalendar(habit)"
                >查看日历</Button
              ><template v-if="!habit.deleted_at"
                ><Button theme="borderless" @click="edit(habit)">编辑</Button
                ><Button
                  theme="borderless"
                  :disabled="busy"
                  @click="transition(habit, habit.paused ? 'resume' : 'pause')"
                  >{{ habit.paused ? "恢复" : "暂停" }}</Button
                ><Button
                  theme="borderless"
                  :disabled="busy"
                  @click="transition(habit, 'delete')"
                  >删除</Button
                ></template
              >
            </div>
          </li>
        </ul>
        <div class="actions">
          <Button
            v-if="page > 1"
            @click="
              page--;
              load();
            "
            >上一页</Button
          ><Button
            v-if="hasMore"
            @click="
              page++;
              load();
            "
            >下一页</Button
          >
        </div>
      </section>
      <section v-if="calendar && selected" class="card">
        <h2>{{ selected.habit.name }} · 打卡日历</h2>
        <label for="habit-month">月份</label>
        <input
          id="habit-month"
          v-model="calendarMonth"
          type="month"
          @change="loadCalendar"
        />
        <p>
          月完成率：{{
            rateLabel(
              calendar.month_rate.earned,
              calendar.month_rate.possible,
              calendar.month_rate.percent,
            )
          }}
        </p>
        <div class="calendar-grid">
          <Button
            v-for="day in calendar.days"
            :key="day.date"
            theme="outline"
            :disabled="
              day.state === 'before_creation' ||
              day.state === 'paused' ||
              day.state === 'future' ||
              day.state === 'deleted' ||
              !!selected.habit.deleted_at
            "
            @click="chooseDay(day)"
            >{{ day.date.slice(-2) }} · {{ stateLabel(day.state) }}</Button
          >
        </div>
        <h3>每周完成率</h3>
        <ul>
          <li v-for="week in calendar.weeks" :key="week.starts_on">
            {{ week.starts_on }} 至 {{ week.ends_on }}：{{
              rateLabel(week.rate.earned, week.rate.possible, week.rate.percent)
            }}
          </li>
        </ul>
        <form v-if="selectedDate" class="form" @submit.prevent="saveCheckin">
          <h3>{{ selectedDate }} 打卡</h3>
          <label
            ><input v-model="checkinCompleted" type="checkbox" />已完成</label
          >
          <label for="checkin-note">备注</label
          ><textarea
            id="checkin-note"
            v-model="checkinNote"
            maxlength="2000"
            rows="3"
          />
          <Button
            html-type="submit"
            type="primary"
            theme="solid"
            :disabled="busy"
            >保存打卡</Button
          >
        </form>
      </section>
      <section v-if="selected" class="card">
        <h2>{{ selected.habit.name }} · 设定历史</h2>
        <ul>
          <li v-for="setting in selected.settings" :key="setting.effective_on">
            {{ setting.effective_on }} 起 · {{ setting.name }} ·
            {{
              setting.cadence === "daily"
                ? "每天"
                : `每周 ${setting.weekly_target} 天`
            }}
          </li>
        </ul>
        <h3>暂停区间</h3>
        <p v-if="selected.pauses.length === 0">尚无暂停记录。</p>
        <ul v-else>
          <li v-for="pause in selected.pauses" :key="pause.start_on">
            {{ pause.start_on }} 至
            {{ pause.end_on || "恢复前" }}（结束日恢复执行）
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
.actions,
nav {
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
  max-width: 28rem;
}
input:not([type="checkbox"]),
select {
  padding: 0.6rem;
  border: 1px solid #aab3c0;
  border-radius: 0.4rem;
  font: inherit;
}
.list {
  list-style: none;
  padding: 0;
}
.list li {
  padding: 0.8rem 0;
  border-top: 1px solid #e0e5ec;
}
.calendar-grid {
  display: grid;
  grid-template-columns: repeat(7, minmax(0, 1fr));
  gap: 0.4rem;
  margin: 1rem 0;
}
.calendar-grid button {
  min-width: 0;
}
textarea {
  padding: 0.6rem;
  border: 1px solid #aab3c0;
  border-radius: 0.4rem;
  font: inherit;
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
  .calendar-grid {
    grid-template-columns: repeat(3, minmax(0, 1fr));
  }
}
</style>
