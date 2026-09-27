<script setup lang="ts">
import { Button } from "@aifuxi/semi-ui-vue/button";
import { onMounted, onUnmounted, ref } from "vue";
import { RouterLink, useRouter } from "vue-router";
import { currentSession, refreshSession, signOut } from "../auth";
import {
  ProjectRequestError,
  changeProject,
  createProject,
  deleteProject,
  listProjects,
  renameProject,
  type Project,
} from "../projects";

const router = useRouter();
const projects = ref<Project[]>([]);
const page = ref(1);
const hasMore = ref(false);
const includeArchived = ref(false);
const newName = ref("");
const editName = ref("");
const editingId = ref<string | null>(null);
const deletingId = ref<string | null>(null);
const busy = ref(false);
const error = ref("");
const notice = ref("");
let interval: ReturnType<typeof setInterval> | undefined;

async function checkSession() {
  if (!(await refreshSession())) await router.replace({ name: "login" });
}

function handleError(cause: unknown) {
  if (cause instanceof ProjectRequestError && cause.status === 401) {
    void router.replace({ name: "login" });
  }
  error.value = cause instanceof Error ? cause.message : "项目操作失败";
}

async function load() {
  try {
    const result = await listProjects(page.value, includeArchived.value);
    projects.value = result.items;
    hasMore.value = result.has_more;
    error.value = "";
  } catch (cause) {
    handleError(cause);
  }
}

onMounted(() => {
  void load();
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

async function submitCreate() {
  if (busy.value || !newName.value.trim()) return;
  busy.value = true;
  error.value = "";
  notice.value = "";
  try {
    await createProject(newName.value);
    newName.value = "";
    page.value = 1;
    await load();
    notice.value = "项目已创建";
  } catch (cause) {
    handleError(cause);
  } finally {
    busy.value = false;
  }
}

function startEdit(project: Project) {
  editingId.value = project.id;
  editName.value = project.name;
  error.value = "";
}

async function submitEdit(project: Project) {
  if (busy.value || !editName.value.trim()) return;
  busy.value = true;
  error.value = "";
  notice.value = "";
  try {
    await renameProject(project, editName.value);
    editingId.value = null;
    await load();
    notice.value = "项目名称已更新";
  } catch (cause) {
    handleError(cause);
  } finally {
    busy.value = false;
  }
}

async function applyAction(
  project: Project,
  action: "complete" | "archive" | "restore",
) {
  if (busy.value) return;
  busy.value = true;
  error.value = "";
  notice.value = "";
  try {
    await changeProject(project, action);
    await load();
    notice.value =
      action === "complete"
        ? "项目已标记完成"
        : action === "archive"
          ? "项目已归档"
          : "项目已恢复";
  } catch (cause) {
    handleError(cause);
  } finally {
    busy.value = false;
  }
}

async function confirmDelete(project: Project) {
  if (busy.value) return;
  busy.value = true;
  error.value = "";
  notice.value = "";
  try {
    await deleteProject(project);
    deletingId.value = null;
    await load();
    notice.value = "项目及其当前任务已删除，已有完成历史仍保留";
  } catch (cause) {
    if (cause instanceof ProjectRequestError && cause.status === 409) {
      deletingId.value = null;
      await load();
    }
    handleError(cause);
  } finally {
    busy.value = false;
  }
}

async function switchArchive() {
  includeArchived.value = !includeArchived.value;
  page.value = 1;
  await load();
}

async function logout() {
  if (busy.value) return;
  busy.value = true;
  error.value = "";
  try {
    await signOut();
    await router.replace({ name: "login" });
  } catch (cause) {
    handleError(cause);
  } finally {
    busy.value = false;
  }
}
</script>

<template>
  <main class="page">
    <div class="shell">
      <header class="topbar">
        <div>
          <p class="eyebrow">Mindfolio / 私人管理</p>
          <h1>私人管理端</h1>
          <p class="subtle">围绕共同成果整理项目</p>
        </div>
        <div class="account">
          <span>已登录：{{ currentSession?.username }}</span>
          <Button theme="borderless" :disabled="busy" @click="logout"
            >退出登录</Button
          >
        </div>
      </header>
      <RouterLink class="inbox-link" to="/today">今日任务</RouterLink>
      <RouterLink class="inbox-link" to="/habits">管理习惯</RouterLink>
      <RouterLink class="inbox-link" to="/journal">每日记录</RouterLink>
      <RouterLink class="inbox-link" to="/history">查看完成历史</RouterLink>
      <section class="card create-card" aria-labelledby="create-title">
        <div>
          <h2 id="create-title">新建项目</h2>
          <p class="subtle">用一个清楚的名称概括要达成的成果。</p>
        </div>
        <form class="form" @submit.prevent="submitCreate">
          <label for="project-name">项目名称</label>
          <div class="input-row">
            <input
              id="project-name"
              v-model="newName"
              required
              maxlength="120"
              autocomplete="off"
              placeholder="例如：整理个人网站"
            />
            <Button
              html-type="submit"
              type="primary"
              theme="solid"
              :loading="busy"
              >创建项目</Button
            >
          </div>
        </form>
      </section>
      <RouterLink class="inbox-link" :to="{ name: 'tasks' }"
        >打开收件箱与任务 →</RouterLink
      >
      <section class="list-section" aria-labelledby="list-title">
        <div class="section-heading">
          <div>
            <h2 id="list-title">
              {{ includeArchived ? "全部项目" : "当前项目" }}
            </h2>
            <p class="subtle">
              {{ includeArchived ? "包含已归档项目" : "归档项目暂不显示" }}
            </p>
          </div>
          <Button theme="outline" @click="switchArchive">{{
            includeArchived ? "只看当前" : "查看全部"
          }}</Button>
        </div>
        <p v-if="error" class="message error" role="alert">
          {{ error }}
          <Button v-if="editingId" theme="borderless" @click="load"
            >刷新列表</Button
          >
        </p>
        <p v-if="notice" class="message success" role="status">{{ notice }}</p>
        <div v-if="projects.length === 0" class="empty">
          {{
            includeArchived
              ? "还没有项目"
              : "当前没有项目，可以从上方创建一个。"
          }}
        </div>
        <ul v-else class="project-list">
          <li
            v-for="project in projects"
            :key="project.id"
            class="project-card"
          >
            <div class="project-main">
              <div class="project-title">
                <h3>{{ project.name }}</h3>
                <span v-if="project.archived_at" class="badge">已归档</span>
                <span v-else-if="project.completed_at" class="badge done"
                  >已完成</span
                >
                <span v-else class="badge active">未完成</span>
              </div>
              <p v-if="project.completed_at" class="subtle">
                完成时间：{{
                  new Date(project.completed_at).toLocaleString("zh-CN")
                }}
              </p>
              <form
                v-if="editingId === project.id"
                class="form edit-form"
                @submit.prevent="submitEdit(project)"
              >
                <label :for="`edit-${project.id}`">修改项目名称</label>
                <div class="input-row">
                  <input
                    :id="`edit-${project.id}`"
                    v-model="editName"
                    maxlength="120"
                    required
                  />
                  <Button
                    html-type="submit"
                    type="primary"
                    theme="solid"
                    :loading="busy"
                    >保存</Button
                  >
                  <Button theme="borderless" @click="editingId = null"
                    >取消</Button
                  >
                </div>
              </form>
            </div>
            <div class="actions">
              <RouterLink
                :to="{ name: 'tasks', query: { project_id: project.id } }"
                >查看任务</RouterLink
              >
              <Button
                v-if="!project.archived_at"
                theme="borderless"
                @click="startEdit(project)"
                >改名</Button
              >
              <Button
                v-if="!project.archived_at && !project.completed_at"
                theme="borderless"
                @click="applyAction(project, 'complete')"
                >标记完成</Button
              >
              <Button
                v-if="project.archived_at"
                theme="borderless"
                @click="applyAction(project, 'restore')"
                >恢复</Button
              >
              <Button
                v-else
                theme="borderless"
                @click="applyAction(project, 'archive')"
                >归档</Button
              >
              <Button theme="borderless" @click="deletingId = project.id"
                >删除</Button
              >
            </div>
            <div
              v-if="deletingId === project.id"
              class="delete-confirm"
              role="alertdialog"
              aria-label="确认删除项目"
            >
              <p>
                删除“{{
                  project.name
                }}”会移除项目及其当前任务、子任务；已有完成历史和当时的名称快照仍保留。归档项目可恢复，删除后无法恢复当前内容。
              </p>
              <Button theme="outline" @click="deletingId = null">取消</Button>
              <Button
                type="danger"
                theme="solid"
                :loading="busy"
                @click="confirmDelete(project)"
                >确认删除项目</Button
              >
            </div>
          </li>
        </ul>
        <nav
          v-if="page > 1 || hasMore"
          class="pagination"
          aria-label="项目分页"
        >
          <Button
            :disabled="page === 1"
            theme="outline"
            @click="
              page--;
              load();
            "
            >上一页</Button
          >
          <span>第 {{ page }} 页</span>
          <Button
            :disabled="!hasMore"
            theme="outline"
            @click="
              page++;
              load();
            "
            >下一页</Button
          >
        </nav>
      </section>
    </div>
  </main>
</template>

<style scoped>
.page {
  min-height: 100vh;
  background: #f5f6f8;
  color: #21252b;
  padding: 2rem 1.5rem 5rem;
}
.shell {
  max-width: 70rem;
  margin: 0 auto;
}
.topbar,
.section-heading,
.project-card,
.project-title,
.actions,
.account,
.input-row,
.pagination {
  display: flex;
  align-items: center;
  gap: 1rem;
}
.inbox-link {
  display: inline-block;
  margin-top: 1.5rem;
  color: #354f9b;
  font-weight: 600;
}
.actions a {
  color: #354f9b;
  font-weight: 600;
  text-decoration: none;
}
.actions a:hover,
.inbox-link:hover {
  text-decoration: underline;
}
.topbar,
.section-heading,
.project-card {
  justify-content: space-between;
}
.topbar {
  padding-bottom: 2rem;
  border-bottom: 1px solid #dce1e9;
}
.eyebrow {
  color: #4c64ad;
  font-size: 0.8rem;
  font-weight: 700;
  letter-spacing: 0.08em;
}
h1 {
  font-size: 2.15rem;
  margin: 0.4rem 0;
}
h2 {
  font-size: 1.35rem;
  margin: 0 0 0.4rem;
}
h3 {
  font-size: 1.08rem;
  margin: 0;
  overflow-wrap: anywhere;
}
.subtle,
.account {
  color: #626c7a;
}
.subtle {
  margin: 0;
  line-height: 1.5;
}
.create-card {
  display: grid;
  grid-template-columns: minmax(12rem, 1fr) minmax(17rem, 2fr);
  gap: 2rem;
  margin-top: 2rem;
  padding: 1.7rem;
}
.card,
.project-card,
.empty {
  background: white;
  border: 1px solid #e0e5ec;
  border-radius: 0.8rem;
}
.form {
  display: grid;
  gap: 0.6rem;
}
label {
  font-weight: 600;
}
.input-row {
  align-items: stretch;
}
input {
  min-width: 0;
  flex: 1;
  height: 2.5rem;
  border: 1px solid #aab3c0;
  border-radius: 0.4rem;
  padding: 0.45rem 0.75rem;
  font: inherit;
}
input:focus-visible {
  outline: 2px solid #4c64ad;
  outline-offset: 2px;
}
.list-section {
  margin-top: 2.5rem;
}
.section-heading {
  margin-bottom: 1rem;
}
.project-list {
  list-style: none;
  margin: 0;
  padding: 0;
  display: grid;
  gap: 0.75rem;
}
.project-card {
  padding: 1.25rem 1.5rem;
  align-items: flex-start;
  flex-wrap: wrap;
}
.project-main {
  min-width: 0;
  flex: 1;
}
.project-title {
  justify-content: flex-start;
  flex-wrap: wrap;
  margin-bottom: 0.4rem;
}
.badge {
  font-size: 0.75rem;
  font-weight: 600;
  border-radius: 99px;
  padding: 0.25rem 0.6rem;
  background: #eceff3;
  color: #4f5968;
}
.badge.done {
  background: #e4f4e9;
  color: #24613b;
}
.badge.active {
  background: #e8edfb;
  color: #354f9b;
}
.actions {
  justify-content: flex-end;
  flex-wrap: wrap;
  gap: 0.2rem;
}
.edit-form {
  max-width: 34rem;
  margin-top: 1rem;
}
.delete-confirm {
  flex-basis: 100%;
  padding: 1rem;
  border-radius: 0.5rem;
  background: #fff4f2;
  color: #7f2a20;
}
.delete-confirm p {
  margin-top: 0;
}
.empty {
  padding: 2.5rem 1.5rem;
  color: #626c7a;
  text-align: center;
}
.message {
  padding: 0.8rem 1rem;
  border-radius: 0.5rem;
}
.error {
  background: #fcebed;
  color: #91232a;
}
.success {
  background: #e4f4e9;
  color: #24613b;
}
.pagination {
  justify-content: center;
  margin-top: 1.25rem;
}
@media (max-width: 700px) {
  .page {
    padding: 1.25rem 1rem 3rem;
  }
  .topbar,
  .project-card {
    align-items: flex-start;
    flex-direction: column;
  }
  .account {
    width: 100%;
    justify-content: space-between;
  }
  .create-card {
    grid-template-columns: 1fr;
    gap: 1.3rem;
  }
  .input-row {
    flex-wrap: wrap;
  }
  .input-row input {
    flex-basis: 100%;
  }
  .actions {
    justify-content: flex-start;
  }
}
</style>
