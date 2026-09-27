import { createApp } from "vue";
import "@aifuxi/semi-theme-default/index.css";
import App from "./App.vue";
import { router } from "./router";

createApp(App).use(router).mount("#app");
