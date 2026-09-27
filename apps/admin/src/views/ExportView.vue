<script setup lang="ts">
import { Button } from "@aifuxi/semi-ui-vue/button";
import { ref } from "vue";
import { RouterLink, useRouter } from "vue-router";
import { ExportRequestError, downloadPrivateExport } from "../export";

const router = useRouter();
const busy = ref(false);
const error = ref("");
const notice = ref("");

async function download() {
  if (busy.value) return;
  busy.value = true;
  error.value = "";
  notice.value = "";
  try {
    const name = await downloadPrivateExport();
    notice.value = `已开始下载 ${name}`;
  } catch (cause) {
    if (cause instanceof ExportRequestError && cause.status === 401)
      void router.replace({ name: "login" });
    error.value = cause instanceof Error ? cause.message : "导出失败";
  } finally {
    busy.value = false;
  }
}
</script>

<template>
  <main class="page">
    <div class="shell">
      <header>
        <div>
          <p class="eyebrow">Mindfolio / 私人管理</p>
          <h1>手动导出</h1>
        </div>
        <RouterLink to="/">返回项目</RouterLink>
      </header>
      <section class="card">
        <p>
          下载当前第一阶段的项目、任务、完成历史、习惯、打卡与每日记录，格式为
          UTF-8 JSON。
        </p>
        <Button
          type="primary"
          theme="solid"
          :disabled="busy"
          @click="download"
          >{{ busy ? "正在生成…" : "下载私人数据" }}</Button
        >
        <p v-if="error" role="alert" class="error">{{ error }}</p>
        <p v-if="notice" role="status" class="success">{{ notice }}</p>
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
  max-width: 60rem;
  margin: auto;
}
header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 1rem;
  border-bottom: 1px solid #dce1e9;
  margin-bottom: 1.5rem;
}
.eyebrow {
  color: #4c64ad;
  font-weight: 700;
}
.card {
  padding: 1.5rem;
  background: white;
  border: 1px solid #e0e5ec;
  border-radius: 0.8rem;
}
a {
  color: #354f9b;
  font-weight: 600;
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
}
</style>
