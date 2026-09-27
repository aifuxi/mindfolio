<script setup lang="ts">
import { Button } from "@aifuxi/semi-ui-vue/button";
import { computed, onMounted, reactive, ref, watch } from "vue";
import { RouterLink, useRoute, useRouter } from "vue-router";
import { currentSession } from "../auth";
import { MarkdownPreview } from "../markdown";
import { ProjectRequestError, listProjects, type Project } from "../projects";
import {
  TaskRequestError,
  createTask,
  getTask,
  listSubtasks,
  listTasks,
  updateTask,
  type Task,
  type TaskFilters,
  type TaskPriority,
  type TaskStatus,
} from "../tasks";

const route = useRoute();
const router = useRouter();
const projects = ref<Project[]>([]);
const tasks = ref<Task[]>([]);
const page = ref(1);
const hasMore = ref(false);
const viewMode = ref<"list" | "board">("list");
const filterForm = reactive({
  keyword: "",
  status: "",
  priority: "",
  planned_date: "",
  due_date: "",
  tag: "",
});
const activeFilters = ref<TaskFilters>({});
const hasActiveFilters = computed(() =>
  Object.values(activeFilters.value).some(Boolean),
);
const busy = ref(false);
const error = ref("");
const notice = ref("");
const editingTask = ref<Task | null>(null);
const subtasks = ref<Task[]>([]);
const pendingCompletion = ref(false);
const createPreview = ref(false);
const editPreview = ref(false);
const childForm = reactive({
  title: "",
  status: "todo" as TaskStatus,
  planned_date: "",
  due_date: "",
});
const selectedProjectId = computed(() =>
  typeof route.query.project_id === "string" ? route.query.project_id : "",
);
const selectedProjectName = computed(
  () =>
    projects.value.find((project) => project.id === selectedProjectId.value)
      ?.name ?? "项目任务",
);

const createForm = reactive({
  title: "",
  description: "",
  project_id: selectedProjectId.value,
  status: "todo" as TaskStatus,
  priority: "",
  planned_date: "",
  due_date: "",
  in_backlog: false,
  tags: "",
});
const editForm = reactive({
  title: "",
  description: "",
  project_id: "",
  status: "todo" as TaskStatus,
  priority: "",
  planned_date: "",
  due_date: "",
  in_backlog: false,
  tags: "",
});

const statusOptions: { value: TaskStatus; label: string }[] = [
  { value: "todo", label: "待办" },
  { value: "in_progress", label: "进行中" },
  { value: "completed", label: "已完成" },
  { value: "canceled", label: "已取消" },
];
const priorityOptions: { value: TaskPriority; label: string }[] = [
  { value: "low", label: "低" },
  { value: "medium", label: "中" },
  { value: "high", label: "高" },
  { value: "urgent", label: "紧急" },
];
const boardColumns = [
  { key: "backlog", label: "待规划区" },
  ...statusOptions.map((item) => ({ key: item.value, label: item.label })),
];

function tasksInColumn(key: string) {
  return tasks.value.filter((task) =>
    key === "backlog"
      ? task.in_backlog
      : !task.in_backlog && task.status === key,
  );
}

function parseTags(value: string) {
  return value
    .split(",")
    .map((tag) => tag.trim())
    .filter(Boolean);
}

function applyFilters() {
  activeFilters.value = {
    keyword: filterForm.keyword.trim() || undefined,
    status: (filterForm.status || undefined) as TaskStatus | undefined,
    priority: (filterForm.priority || undefined) as TaskPriority | undefined,
    planned_date: filterForm.planned_date || undefined,
    due_date: filterForm.due_date || undefined,
    tag: filterForm.tag.trim() || undefined,
  };
  page.value = 1;
  void loadTasks();
}

function clearFilters() {
  Object.assign(filterForm, {
    keyword: "",
    status: "",
    priority: "",
    planned_date: "",
    due_date: "",
    tag: "",
  });
  applyFilters();
}

function statusLabel(value: TaskStatus) {
  return statusOptions.find((item) => item.value === value)?.label ?? value;
}

function priorityLabel(value: TaskPriority | null | undefined) {
  return (
    priorityOptions.find((item) => item.value === value)?.label ?? "未设优先级"
  );
}

function handleError(cause: unknown) {
  if (
    (cause instanceof TaskRequestError ||
      cause instanceof ProjectRequestError) &&
    cause.status === 401
  ) {
    void router.replace({ name: "login" });
  }
  error.value = cause instanceof Error ? cause.message : "任务操作失败";
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

async function loadTasks() {
  try {
    const result = await listTasks(
      page.value,
      selectedProjectId.value || null,
      activeFilters.value,
    );
    tasks.value = result.items;
    hasMore.value = result.has_more;
    error.value = "";
  } catch (cause) {
    handleError(cause);
  }
}

onMounted(() => {
  void loadProjects();
  void loadTasks();
});

watch(
  () => route.query.project_id,
  () => {
    page.value = 1;
    editingTask.value = null;
    subtasks.value = [];
    pendingCompletion.value = false;
    createForm.project_id = selectedProjectId.value;
    createForm.in_backlog = false;
    void loadTasks();
  },
);

async function selectScope(event: Event) {
  const id = (event.target as HTMLSelectElement).value;
  await router.push({ name: "tasks", query: id ? { project_id: id } : {} });
}

function clearCreate() {
  createForm.title = "";
  createForm.description = "";
  createPreview.value = false;
  createForm.status = "todo";
  createForm.priority = "";
  createForm.planned_date = "";
  createForm.due_date = "";
  createForm.in_backlog = false;
  createForm.tags = "";
}

async function submitCreate() {
  if (busy.value) return;
  busy.value = true;
  error.value = "";
  notice.value = "";
  try {
    await createTask({
      title: createForm.title,
      description: createForm.description,
      project_id: createForm.project_id || null,
      status: createForm.status,
      priority: createForm.priority
        ? (createForm.priority as TaskPriority)
        : null,
      planned_date: createForm.planned_date || null,
      due_date: createForm.due_date || null,
      in_backlog: createForm.in_backlog,
      tags: parseTags(createForm.tags),
    });
    const target = createForm.project_id;
    clearCreate();
    if (target !== selectedProjectId.value) {
      await router.push({
        name: "tasks",
        query: target ? { project_id: target } : {},
      });
    } else {
      page.value = 1;
      await loadTasks();
    }
    notice.value = "任务已创建";
  } catch (cause) {
    handleError(cause);
  } finally {
    busy.value = false;
  }
}

async function beginEdit(task: Task) {
  editingTask.value = task;
  subtasks.value = [];
  pendingCompletion.value = false;
  editForm.title = task.title;
  editForm.description = task.description;
  editPreview.value = false;
  editForm.project_id = task.project_id ?? "";
  editForm.status = task.status;
  editForm.priority = task.priority ?? "";
  editForm.planned_date = task.planned_date ?? "";
  editForm.due_date = task.due_date ?? "";
  editForm.in_backlog = task.in_backlog;
  editForm.tags = task.tags.join(", ");
  error.value = "";
  if (!task.parent_id) {
    try {
      subtasks.value = await listSubtasks(task.id);
    } catch (cause) {
      handleError(cause);
    }
  }
}

async function returnToParent() {
  if (!editingTask.value?.parent_id) return;
  try {
    await beginEdit(await getTask(editingTask.value.parent_id));
  } catch (cause) {
    handleError(cause);
  }
}

async function submitChild() {
  const parent = editingTask.value;
  if (busy.value || !parent || parent.parent_id) return;
  busy.value = true;
  error.value = "";
  try {
    await createTask({
      parent_id: parent.id,
      project_id: parent.project_id,
      title: childForm.title,
      status: childForm.status,
      planned_date: childForm.planned_date || null,
      due_date: childForm.due_date || null,
    });
    childForm.title = "";
    childForm.status = "todo";
    childForm.planned_date = "";
    childForm.due_date = "";
    subtasks.value = await listSubtasks(parent.id);
    notice.value = "子任务已创建";
  } catch (cause) {
    handleError(cause);
  } finally {
    busy.value = false;
  }
}

function clearBacklog(form: typeof createForm) {
  if (!form.project_id) form.in_backlog = false;
}

async function refreshEditingTask() {
  if (!editingTask.value) return;
  try {
    editingTask.value = await getTask(editingTask.value.id);
    if (!editingTask.value.parent_id) {
      subtasks.value = await listSubtasks(editingTask.value.id);
    }
    pendingCompletion.value = false;
    await loadTasks();
    notice.value = "已取得最新版本，编辑内容仍保留";
  } catch (cause) {
    handleError(cause);
  }
}

async function saveEdit(confirmed = false) {
  if (busy.value || !editingTask.value) return;
  busy.value = true;
  error.value = "";
  notice.value = "";
  try {
    if (
      !confirmed &&
      !editingTask.value.parent_id &&
      editingTask.value.status !== "completed" &&
      editForm.status === "completed"
    ) {
      subtasks.value = await listSubtasks(editingTask.value.id);
      if (subtasks.value.some((task) => task.status !== "completed")) {
        pendingCompletion.value = true;
        return;
      }
    }
    await updateTask(editingTask.value, {
      title: editForm.title,
      description: editForm.description,
      project_id: editForm.project_id || null,
      status: editForm.status,
      priority: editForm.priority ? (editForm.priority as TaskPriority) : null,
      planned_date: editForm.planned_date || null,
      due_date: editForm.due_date || null,
      in_backlog: editForm.in_backlog,
      tags: parseTags(editForm.tags),
    });
    editingTask.value = null;
    subtasks.value = [];
    pendingCompletion.value = false;
    await loadTasks();
    notice.value = "任务已保存";
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
      <header class="header">
        <div>
          <p class="eyebrow">Mindfolio / 私人管理</p>
          <h1>收件箱与任务</h1>
          <p class="subtle">已登录：{{ currentSession?.username }}</p>
        </div>
        <RouterLink to="/">返回项目</RouterLink>
      </header>

      <section class="card create" aria-labelledby="create-title">
        <div>
          <h2 id="create-title">新建任务</h2>
          <p class="subtle">临时事项先放入收件箱，也可以直接归入项目。</p>
        </div>
        <form class="form" @submit.prevent="submitCreate">
          <label for="new-title">标题</label>
          <input
            id="new-title"
            v-model="createForm.title"
            required
            maxlength="200"
          />
          <label for="new-description">说明（Markdown 原文）</label>
          <textarea
            id="new-description"
            v-model="createForm.description"
            maxlength="20000"
            rows="3"
          />
          <div>
            <Button
              html-type="button"
              theme="borderless"
              @click="createPreview = !createPreview"
            >
              {{ createPreview ? "隐藏说明预览" : "预览说明" }}
            </Button>
          </div>
          <MarkdownPreview
            v-if="createPreview"
            :source="createForm.description"
            label="新建任务说明预览"
          />
          <div class="fields">
            <div>
              <label for="new-project">归属</label
              ><select
                id="new-project"
                v-model="createForm.project_id"
                @change="clearBacklog(createForm)"
              >
                <option value="">收件箱</option>
                <option
                  v-for="project in projects"
                  :key="project.id"
                  :value="project.id"
                >
                  {{ project.name
                  }}{{ project.archived_at ? "（已归档）" : "" }}
                </option>
              </select>
            </div>
            <div>
              <label for="new-status">状态</label
              ><select id="new-status" v-model="createForm.status">
                <option
                  v-for="item in statusOptions"
                  :key="item.value"
                  :value="item.value"
                >
                  {{ item.label }}
                </option>
              </select>
            </div>
            <div>
              <label for="new-priority">优先级</label
              ><select id="new-priority" v-model="createForm.priority">
                <option value="">未设置</option>
                <option
                  v-for="item in priorityOptions"
                  :key="item.value"
                  :value="item.value"
                >
                  {{ item.label }}
                </option>
              </select>
            </div>
            <div>
              <label for="new-planned">计划日期</label
              ><input
                id="new-planned"
                v-model="createForm.planned_date"
                type="date"
              />
            </div>
            <div>
              <label for="new-due">截止日期</label
              ><input id="new-due" v-model="createForm.due_date" type="date" />
            </div>
          </div>
          <label for="new-tags">任务标签（逗号分隔）</label>
          <input
            id="new-tags"
            v-model="createForm.tags"
            placeholder="例如：写作, 研究"
          />
          <label class="check"
            ><input
              v-model="createForm.in_backlog"
              type="checkbox"
              :disabled="!createForm.project_id"
            />放入待规划区</label
          >
          <div>
            <Button
              html-type="submit"
              type="primary"
              theme="solid"
              :loading="busy"
              >创建任务</Button
            >
          </div>
        </form>
      </section>

      <section class="list" aria-labelledby="list-title">
        <div class="list-heading">
          <div>
            <h2 id="list-title">
              {{ selectedProjectId ? selectedProjectName : "收件箱" }}
            </h2>
            <p class="subtle">每页最多 50 项，包含一级子任务。</p>
          </div>
          <div class="scope">
            <label for="task-scope">查看范围</label
            ><select
              id="task-scope"
              :value="selectedProjectId"
              @change="selectScope"
            >
              <option value="">收件箱</option>
              <option
                v-for="project in projects"
                :key="project.id"
                :value="project.id"
              >
                {{ project.name }}{{ project.archived_at ? "（已归档）" : "" }}
              </option>
            </select>
          </div>
        </div>
        <form
          class="card filters"
          aria-label="筛选任务"
          @submit.prevent="applyFilters"
        >
          <div class="fields">
            <div>
              <label for="filter-keyword">关键词</label
              ><input id="filter-keyword" v-model="filterForm.keyword" />
            </div>
            <div>
              <label for="filter-status">状态</label>
              <select id="filter-status" v-model="filterForm.status">
                <option value="">全部状态</option>
                <option
                  v-for="item in statusOptions"
                  :key="item.value"
                  :value="item.value"
                >
                  {{ item.label }}
                </option>
              </select>
            </div>
            <div>
              <label for="filter-priority">优先级</label>
              <select id="filter-priority" v-model="filterForm.priority">
                <option value="">全部优先级</option>
                <option
                  v-for="item in priorityOptions"
                  :key="item.value"
                  :value="item.value"
                >
                  {{ item.label }}
                </option>
              </select>
            </div>
            <div>
              <label for="filter-planned">计划日期</label
              ><input
                id="filter-planned"
                v-model="filterForm.planned_date"
                type="date"
              />
            </div>
            <div>
              <label for="filter-due">截止日期</label
              ><input
                id="filter-due"
                v-model="filterForm.due_date"
                type="date"
              />
            </div>
            <div>
              <label for="filter-tag">任务标签</label
              ><input id="filter-tag" v-model="filterForm.tag" />
            </div>
          </div>
          <div class="filter-actions">
            <Button html-type="submit" type="primary" theme="solid"
              >筛选</Button
            >
            <Button theme="borderless" @click="clearFilters">清除筛选</Button>
          </div>
        </form>
        <div v-if="selectedProjectId" class="view-switch" aria-label="项目视图">
          <Button
            :theme="viewMode === 'list' ? 'solid' : 'outline'"
            @click="viewMode = 'list'"
            >列表</Button
          >
          <Button
            :theme="viewMode === 'board' ? 'solid' : 'outline'"
            @click="viewMode = 'board'"
            >看板</Button
          >
        </div>
        <p v-if="error" class="message error" role="alert">
          {{ error }}
          <Button
            v-if="editingTask"
            theme="borderless"
            @click="refreshEditingTask"
            >取得最新版本</Button
          >
        </p>
        <p v-if="notice" class="message success" role="status">{{ notice }}</p>
        <div v-if="tasks.length === 0" class="empty">
          {{
            hasActiveFilters ? "没有符合筛选条件的任务。" : "这里还没有任务。"
          }}
        </div>
        <ul
          v-else-if="!selectedProjectId || viewMode === 'list'"
          class="task-list"
          aria-label="任务列表"
        >
          <li v-for="task in tasks" :key="task.id" class="task-card">
            <div class="task-main">
              <h3>{{ task.parent_id ? "↳ " : "" }}{{ task.title }}</h3>
              <div class="meta">
                <span>{{ statusLabel(task.status) }}</span
                ><span v-if="task.parent_id">子任务</span
                ><span v-if="task.in_backlog">待规划区</span
                ><span>{{ priorityLabel(task.priority) }}</span
                ><span v-if="task.planned_date"
                  >计划 {{ task.planned_date }}</span
                ><span v-if="task.due_date">截止 {{ task.due_date }}</span
                ><span v-for="tag in task.tags" :key="tag">#{{ tag }}</span>
              </div>
            </div>
            <Button theme="borderless" @click="beginEdit(task)">编辑</Button>
          </li>
        </ul>
        <div v-else class="board" aria-label="任务看板">
          <section
            v-for="column in boardColumns"
            :key="column.key"
            class="board-column"
            :aria-label="column.label"
          >
            <h3>
              {{ column.label }}
              <span>{{ tasksInColumn(column.key).length }}</span>
            </h3>
            <ul class="task-list">
              <li
                v-for="task in tasksInColumn(column.key)"
                :key="task.id"
                class="task-card"
              >
                <div class="task-main">
                  <h3>{{ task.parent_id ? "↳ " : "" }}{{ task.title }}</h3>
                  <div class="meta">
                    <span v-if="task.parent_id">子任务</span>
                    <span v-if="task.in_backlog">{{
                      statusLabel(task.status)
                    }}</span>
                    <span v-if="task.planned_date"
                      >计划 {{ task.planned_date }}</span
                    >
                    <span v-if="task.due_date">截止 {{ task.due_date }}</span>
                    <span v-for="tag in task.tags" :key="tag">#{{ tag }}</span>
                  </div>
                </div>
                <Button theme="borderless" @click="beginEdit(task)"
                  >编辑</Button
                >
              </li>
            </ul>
          </section>
        </div>
        <nav
          v-if="page > 1 || hasMore"
          class="pagination"
          aria-label="任务分页"
        >
          <Button
            theme="outline"
            :disabled="page === 1"
            @click="
              page--;
              loadTasks();
            "
            >上一页</Button
          ><span>第 {{ page }} 页</span
          ><Button
            theme="outline"
            :disabled="!hasMore"
            @click="
              page++;
              loadTasks();
            "
            >下一页</Button
          >
        </nav>
      </section>

      <section
        v-if="editingTask"
        class="card edit"
        aria-labelledby="edit-title"
      >
        <div class="edit-heading">
          <h2 id="edit-title">
            {{ editingTask.parent_id ? "编辑子任务" : "编辑任务" }}
          </h2>
          <Button theme="borderless" @click="editingTask = null">关闭</Button>
        </div>
        <Button
          v-if="editingTask.parent_id"
          theme="borderless"
          @click="returnToParent"
          >返回父任务</Button
        >
        <form class="form" @submit.prevent="saveEdit()">
          <label for="edit-title-input">标题</label
          ><input
            id="edit-title-input"
            v-model="editForm.title"
            required
            maxlength="200"
          />
          <label for="edit-description">说明（Markdown 原文）</label
          ><textarea
            id="edit-description"
            v-model="editForm.description"
            maxlength="20000"
            rows="5"
          />
          <div>
            <Button
              html-type="button"
              theme="borderless"
              @click="editPreview = !editPreview"
            >
              {{ editPreview ? "隐藏说明预览" : "预览说明" }}
            </Button>
          </div>
          <MarkdownPreview
            v-if="editPreview"
            :source="editForm.description"
            label="编辑任务说明预览"
          />
          <div class="fields">
            <div>
              <label for="edit-project">归属</label
              ><select
                id="edit-project"
                v-model="editForm.project_id"
                @change="clearBacklog(editForm)"
              >
                <option value="">收件箱</option>
                <option
                  v-for="project in projects"
                  :key="project.id"
                  :value="project.id"
                >
                  {{ project.name
                  }}{{ project.archived_at ? "（已归档）" : "" }}
                </option>
              </select>
            </div>
            <div>
              <label for="edit-status">状态</label
              ><select id="edit-status" v-model="editForm.status">
                <option
                  v-for="item in statusOptions"
                  :key="item.value"
                  :value="item.value"
                >
                  {{ item.label }}
                </option>
              </select>
            </div>
            <div>
              <label for="edit-priority">优先级</label
              ><select id="edit-priority" v-model="editForm.priority">
                <option value="">未设置</option>
                <option
                  v-for="item in priorityOptions"
                  :key="item.value"
                  :value="item.value"
                >
                  {{ item.label }}
                </option>
              </select>
            </div>
            <div>
              <label for="edit-planned">计划日期</label
              ><input
                id="edit-planned"
                v-model="editForm.planned_date"
                type="date"
              />
            </div>
            <div>
              <label for="edit-due">截止日期</label
              ><input id="edit-due" v-model="editForm.due_date" type="date" />
            </div>
          </div>
          <label for="edit-tags">任务标签（逗号分隔）</label>
          <input id="edit-tags" v-model="editForm.tags" />
          <label class="check"
            ><input
              v-model="editForm.in_backlog"
              type="checkbox"
              :disabled="!editForm.project_id"
            />放入待规划区</label
          >
          <div
            v-if="pendingCompletion && editForm.status === 'completed'"
            class="completion-prompt"
            role="alertdialog"
            aria-label="确认完成父任务"
          >
            <p>
              仍有
              {{
                subtasks.filter((task) => task.status !== "completed").length
              }}
              个未完成子任务。完成父任务不会改变这些子任务的状态。
            </p>
            <Button theme="outline" @click="pendingCompletion = false"
              >暂不完成</Button
            >
            <Button theme="solid" type="primary" @click="saveEdit(true)"
              >仍要完成父任务</Button
            >
          </div>
          <div>
            <Button
              html-type="submit"
              type="primary"
              theme="solid"
              :loading="busy"
              >保存任务</Button
            >
          </div>
        </form>
        <section
          v-if="!editingTask.parent_id"
          class="subtasks"
          aria-label="子任务"
        >
          <h3>子任务</h3>
          <p v-if="subtasks.length === 0" class="subtle">还没有子任务。</p>
          <ul v-else class="task-list">
            <li v-for="child in subtasks" :key="child.id" class="task-card">
              <div class="task-main">
                <h3>{{ child.title }}</h3>
                <div class="meta">
                  <span>{{ statusLabel(child.status) }}</span>
                  <span v-if="child.planned_date"
                    >计划 {{ child.planned_date }}</span
                  >
                  <span v-if="child.due_date">截止 {{ child.due_date }}</span>
                </div>
              </div>
              <Button theme="borderless" @click="beginEdit(child)"
                >编辑子任务</Button
              >
            </li>
          </ul>
          <form class="form child-form" @submit.prevent="submitChild">
            <h3>新建子任务</h3>
            <label for="child-title">子任务标题</label>
            <input
              id="child-title"
              v-model="childForm.title"
              required
              maxlength="200"
            />
            <div class="fields">
              <div>
                <label for="child-status">子任务状态</label>
                <select id="child-status" v-model="childForm.status">
                  <option
                    v-for="item in statusOptions"
                    :key="item.value"
                    :value="item.value"
                  >
                    {{ item.label }}
                  </option>
                </select>
              </div>
              <div>
                <label for="child-planned">子任务计划日期</label>
                <input
                  id="child-planned"
                  v-model="childForm.planned_date"
                  type="date"
                />
              </div>
              <div>
                <label for="child-due">子任务截止日期</label>
                <input
                  id="child-due"
                  v-model="childForm.due_date"
                  type="date"
                />
              </div>
            </div>
            <div>
              <Button html-type="submit" theme="outline" :loading="busy"
                >创建子任务</Button
              >
            </div>
          </form>
        </section>
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
.header,
.list-heading,
.edit-heading,
.task-card,
.pagination {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 1rem;
}
.header {
  border-bottom: 1px solid #dce1e9;
  padding-bottom: 2rem;
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
a {
  color: #354f9b;
  font-weight: 600;
}
.subtle {
  color: #626c7a;
  margin: 0;
  line-height: 1.5;
}
.card,
.task-card,
.empty {
  background: white;
  border: 1px solid #e0e5ec;
  border-radius: 0.8rem;
}
.create,
.edit {
  padding: 1.7rem;
  margin-top: 2rem;
}
.create {
  display: grid;
  grid-template-columns: minmax(12rem, 1fr) minmax(20rem, 2fr);
  gap: 2rem;
}
.form {
  display: grid;
  gap: 0.6rem;
}
.markdown-preview {
  border: 1px solid #d8dee8;
  border-radius: 0.5rem;
  background: #fafbfd;
  padding: 0.9rem 1rem;
  min-height: 3rem;
  overflow-wrap: anywhere;
}
.markdown-preview :deep(:first-child) {
  margin-top: 0;
}
.markdown-preview :deep(:last-child) {
  margin-bottom: 0;
}
.markdown-preview :deep(ul),
.markdown-preview :deep(ol) {
  padding-left: 1.5rem;
}
.markdown-preview :deep(pre) {
  overflow-x: auto;
  border-radius: 0.4rem;
  background: #e9edf4;
  padding: 0.8rem;
}
.markdown-preview :deep(code) {
  font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
}
.markdown-preview :deep(:not(pre) > code) {
  background: #e9edf4;
  border-radius: 0.2rem;
  padding: 0.1rem 0.25rem;
}
.markdown-preview :deep(blockquote) {
  border-left: 3px solid #9baac0;
  margin-left: 0;
  padding-left: 1rem;
  color: #4f5968;
}
.markdown-preview :deep(.markdown-image-alt) {
  color: #626c7a;
}
label {
  font-weight: 600;
}
input:not([type="checkbox"]),
textarea,
select {
  width: 100%;
  min-width: 0;
  border: 1px solid #aab3c0;
  border-radius: 0.4rem;
  padding: 0.5rem 0.7rem;
  font: inherit;
  background: white;
  color: inherit;
}
input:not([type="checkbox"]),
select {
  height: 2.6rem;
}
textarea {
  resize: vertical;
}
input:focus-visible,
textarea:focus-visible,
select:focus-visible {
  outline: 2px solid #4c64ad;
  outline-offset: 2px;
}
.fields {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 0.9rem;
  margin-top: 0.5rem;
}
.fields > div {
  display: grid;
  gap: 0.5rem;
  align-content: start;
}
.check {
  display: flex;
  gap: 0.6rem;
  align-items: center;
  margin: 0.5rem 0;
}
.list {
  margin-top: 2.5rem;
}
.list-heading {
  margin-bottom: 1rem;
}
.filters {
  padding: 1rem 1.25rem;
  margin-bottom: 1rem;
}
.filters .fields {
  grid-template-columns: repeat(3, minmax(0, 1fr));
  margin-top: 0;
}
.filter-actions,
.view-switch {
  display: flex;
  gap: 0.6rem;
  align-items: center;
  margin-top: 1rem;
}
.view-switch {
  margin-bottom: 1rem;
}
.board {
  display: grid;
  grid-template-columns: repeat(5, minmax(13rem, 1fr));
  gap: 0.75rem;
  overflow-x: auto;
  padding-bottom: 0.75rem;
}
.board-column {
  background: #e9edf4;
  border-radius: 0.7rem;
  padding: 0.75rem;
  min-height: 10rem;
}
.board-column > h3 {
  display: flex;
  justify-content: space-between;
  padding: 0.3rem 0.2rem 0.8rem;
}
.board-column .task-card {
  display: block;
  padding: 0.9rem;
}
.board-column .task-card button {
  margin-top: 0.6rem;
}
.scope {
  display: flex;
  align-items: center;
  gap: 0.7rem;
}
.scope label {
  white-space: nowrap;
  flex-shrink: 0;
}
.scope select {
  min-width: 12rem;
}
.task-list {
  list-style: none;
  padding: 0;
  margin: 0;
  display: grid;
  gap: 0.75rem;
}
.task-card {
  padding: 1.2rem 1.5rem;
  align-items: flex-start;
}
.task-main {
  min-width: 0;
}
.meta {
  display: flex;
  flex-wrap: wrap;
  gap: 0.6rem;
  margin-top: 0.6rem;
  color: #626c7a;
  font-size: 0.85rem;
}
.meta span {
  background: #eef1f6;
  border-radius: 99px;
  padding: 0.2rem 0.6rem;
}
.empty {
  padding: 2.5rem 1.5rem;
  text-align: center;
  color: #626c7a;
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
.edit {
  max-width: 48rem;
}
.subtasks {
  border-top: 1px solid #e0e5ec;
  margin-top: 1.5rem;
  padding-top: 1.5rem;
  display: grid;
  gap: 1rem;
}
.child-form {
  border-top: 1px solid #e0e5ec;
  padding-top: 1.5rem;
}
.completion-prompt {
  background: #fff4db;
  border: 1px solid #d9ad52;
  border-radius: 0.5rem;
  padding: 1rem;
}
@media (max-width: 760px) {
  .page {
    padding: 1.25rem 1rem 3rem;
  }
  .create {
    grid-template-columns: 1fr;
    gap: 1.3rem;
  }
  .header,
  .list-heading {
    align-items: flex-start;
    flex-direction: column;
  }
  .filters .fields {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }
}
@media (max-width: 450px) {
  .fields,
  .filters .fields {
    grid-template-columns: 1fr;
  }
}
</style>
