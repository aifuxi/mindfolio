import { createRouter, createWebHistory } from "vue-router";
import { refreshSession } from "./auth";
import HomeView from "./views/HomeView.vue";
import LoginView from "./views/LoginView.vue";

export const router = createRouter({
  history: createWebHistory(),
  routes: [
    { path: "/login", name: "login", component: LoginView },
    {
      path: "/",
      name: "home",
      component: HomeView,
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
