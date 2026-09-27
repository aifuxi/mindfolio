import js from "@eslint/js";
import eslintConfigPrettier from "eslint-config-prettier";
import pluginVue from "eslint-plugin-vue";
import globals from "globals";
import tseslint from "typescript-eslint";

export default [
  {
    ignores: [
      "**/dist/**",
      "**/node_modules/**",
      "**/coverage/**",
      "**/target/**",
    ],
  },
  js.configs.recommended,
  ...tseslint.configs.recommended,
  ...pluginVue.configs["flat/recommended-error"],
  {
    files: ["apps/admin/src/**/*.{ts,vue}"],
    languageOptions: {
      globals: globals.browser,
      parserOptions: { parser: tseslint.parser },
    },
    rules: { "no-console": "warn" },
  },
  {
    files: [
      "eslint.config.js",
      "apps/admin/vite.config.ts",
      "scripts/contract.mjs",
    ],
    languageOptions: { globals: globals.node },
  },
  eslintConfigPrettier,
];
