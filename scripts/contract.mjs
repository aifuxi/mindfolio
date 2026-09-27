import { spawnSync } from "node:child_process";
import { readFile, mkdir, writeFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import openapiTS, { astToString } from "openapi-typescript";

const root = fileURLToPath(new URL("../", import.meta.url));
const mode = process.argv[2];

if (mode !== "generate" && mode !== "check") {
  console.error("用法：contract.mjs {generate|check}");
  process.exit(2);
}

const rust = spawnSync(
  "cargo",
  [
    "run",
    "--quiet",
    "--locked",
    "-p",
    "mindfolio-api",
    "--bin",
    "generate_openapi",
  ],
  { cwd: root, encoding: "utf8", maxBuffer: 16 * 1024 * 1024 },
);

if (rust.status !== 0) {
  process.stderr.write(rust.stderr || "OpenAPI 生成失败\n");
  process.exit(rust.status || 1);
}

const openapi = rust.stdout;
const document = JSON.parse(openapi);
if (document.openapi !== "3.1.0") {
  throw new Error(`OpenAPI 版本必须为 3.1.0，当前为 ${document.openapi}`);
}

const types = astToString(await openapiTS(document));
const artifacts = [
  ["packages/api-contract/openapi.json", openapi],
  ["packages/api-contract/src/schema.d.ts", types],
];

if (mode === "generate") {
  for (const [path, content] of artifacts) {
    const file = new URL(`../${path}`, import.meta.url);
    await mkdir(new URL(".", file), { recursive: true });
    await writeFile(file, content);
    console.log(`已生成 ${path}`);
  }
} else {
  let drifted = false;
  for (const [path, expected] of artifacts) {
    const file = new URL(`../${path}`, import.meta.url);
    const actual = await readFile(file, "utf8").catch(() => null);
    if (actual !== expected) {
      console.error(`契约产物不一致：${path}`);
      drifted = true;
    }
  }
  if (drifted) {
    console.error("请运行 mise run contract:generate 并提交生成产物");
    process.exitCode = 1;
  } else {
    console.log("OpenAPI 与 TypeScript 类型产物一致");
  }
}
