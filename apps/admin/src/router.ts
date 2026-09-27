import { createRouter, createWebHistory } from "vue-router";
import { refreshSession } from "./auth";
import HomeView from "./views/HomeView.vue";
import HabitsView from "./views/HabitsView.vue";
import JournalView from "./views/JournalView.vue";
import HistoryView from "./views/HistoryView.vue";
import LoginView from "./views/LoginView.vue";
import TasksView from "./views/TasksView.vue";
import TodayView from "./views/TodayView.vue";
import WeeklyReviewView from "./views/WeeklyReviewView.vue";

export const router = createRouter({
  history: createWebHistory(),
  routes: [
    { path: "/login", name: "login", component: LoginView },
    {
      path: "/habits",
      name: "habits",
      component: HabitsView,
      meta: { requiresAuth: true },
    },
    {
      path: "/journal/:date?",
      name: "journal",
      component: JournalView,
      meta: { requiresAuth: true },
    },
    {
      path: "/",
      name: "home",
      component: HomeView,
      meta: { requiresAuth: true },
    },
    {
      path: "/tasks",
      name: "tasks",
      component: TasksView,
      meta: { requiresAuth: true },
    },
    {
      path: "/history",
      name: "history",
      component: HistoryView,
      meta: { requiresAuth: true },
    },
    {
      path: "/today",
      name: "today",
      component: TodayView,
      meta: { requiresAuth: true },
    },
    {
      path: "/weekly-review",
      name: "weekly-review",
      component: WeeklyReviewView,
      meta: { requiresAuth: true },
    },
  ],
});

router.beforeEach(async (to) => {
  if (to.meta.requiresAuth && !(await refreshSession())) {
    return { name: "login" };
  }
  return true;
});
