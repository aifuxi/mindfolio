<script setup lang="ts">
import { ref } from "vue";
import { Button } from "@aifuxi/semi-ui-vue/button";
import { checkApiStatus } from "./api-status";

const status = ref("尚未检查");

async function checkApi() {
  status.value = "检查中";
  status.value = await checkApiStatus(fetch);
}
</script>

<template>
  <main>
    <h1>Mindfolio</h1>
    <p>管理端最小工程</p>
    <Button @click="checkApi">检查 API</Button>
    <p role="status">{{ status }}</p>
  </main>
</template>

<style scoped>
main {
  max-width: 42rem;
  margin: 5rem auto;
  padding: 0 2rem;
}
</style>
