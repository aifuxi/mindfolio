import { createHash } from "node:crypto";
import { createReadStream, createWriteStream } from "node:fs";
import {
  copyFile,
  mkdtemp,
  mkdir,
  rm,
  stat,
  writeFile,
} from "node:fs/promises";
import { tmpdir } from "node:os";
import { basename, join, resolve } from "node:path";
import { spawn } from "node:child_process";
import { Writable } from "node:stream";
import { fileURLToPath } from "node:url";
import { selectRetainedSnapshots } from "./backup-retention.mjs";

const root = resolve(fileURLToPath(new URL("..", import.meta.url)));
const deployEnv =
  process.env.DEPLOY_ENV_FILE || "/srv/mindfolio/config/production.env";
const tempRoot = process.env.BACKUP_TMP_DIR || tmpdir();
const warningMinutes = 50;
const limitMinutes = 60;

function requireConfig() {
  if (!process.env.RESTIC_REPOSITORY || !process.env.RESTIC_PASSWORD_FILE) {
    throw new Error("缺少 RESTIC_REPOSITORY 或 RESTIC_PASSWORD_FILE");
  }
  if (
    process.env.NODE_ENV !== "test" &&
    !process.env.RESTIC_REPOSITORY.startsWith("s3:https://")
  ) {
    throw new Error("生产备份仓库必须使用 HTTPS S3 Bucket");
  }
  if (
    process.env.RESTIC_REPOSITORY.startsWith("s3:") &&
    !process.env.AWS_DEFAULT_REGION
  ) {
    throw new Error("缺少 AWS_DEFAULT_REGION");
  }
}

function run(command, args, options = {}) {
  return new Promise((resolveRun, reject) => {
    const child = spawn(command, args, {
      cwd: root,
      env: { ...process.env, TZ: "UTC" },
      stdio: ["ignore", "pipe", "pipe"],
    });
    const chunks = [];
    let length = 0;
    if (options.output instanceof Writable) {
      options.output.once("error", () => {
        child.kill();
        reject(new Error(`${command} 输出写入失败`));
      });
      child.stdout.pipe(options.output, { end: false });
    } else {
      child.stdout.on("data", (chunk) => {
        if (options.output) {
          options.output.write(chunk);
          return;
        }
        length += chunk.length;
        if (length > 8 * 1024 * 1024) child.kill();
        else chunks.push(chunk);
      });
    }
    child.stderr.resume();
    child.on("error", () => reject(new Error(`${command} 无法启动`)));
    child.on("close", (code) => {
      if (code !== 0) reject(new Error(`${command} 执行失败，退出码 ${code}`));
      else resolveRun(Buffer.concat(chunks).toString("utf8"));
    });
  });
}

function restic(args, options) {
  const s3Options = process.env.RESTIC_REPOSITORY?.startsWith("s3:")
    ? [
        "-o",
        "s3.bucket-lookup=dns",
        "-o",
        `s3.region=${process.env.AWS_DEFAULT_REGION}`,
      ]
    : [];
  return run("restic", ["--retry-lock", "5m", ...s3Options, ...args], options);
}

function parseJsonLines(output, type) {
  return output
    .split("\n")
    .filter(Boolean)
    .map((line) => JSON.parse(line))
    .find((line) => line.message_type === type);
}

async function snapshots() {
  const result = JSON.parse(await restic(["--json", "snapshots"]));
  return result.filter((item) => item.tags?.includes("mindfolio-verified"));
}

async function sha256(path) {
  const hash = createHash("sha256");
  for await (const chunk of createReadStream(path)) hash.update(chunk);
  return hash.digest("hex");
}

async function dumpHash(id, path) {
  const hash = createHash("sha256");
  await restic(["--no-cache", "dump", id, path], {
    output: { write: (chunk) => hash.update(chunk) },
  });
  return hash.digest("hex");
}

async function saveFile(kind, path, startedAt) {
  const digest = await sha256(path);
  const output = await restic([
    "--json",
    "backup",
    "--time",
    startedAt.toISOString().slice(0, 19).replace("T", " "),
    "--host",
    "mindfolio",
    "--tag",
    `mindfolio-${kind}`,
    "--tag",
    `sha256-${digest}`,
    path,
  ]);
  const initialId = parseJsonLines(output, "summary")?.snapshot_id;
  if (!initialId) throw new Error("restic 未返回快照标识");
  if ((await dumpHash(initialId, path)) !== digest) {
    throw new Error("服务器外归档读回校验失败");
  }
  const tagged = await restic([
    "--json",
    "tag",
    "--add",
    "mindfolio-verified",
    initialId,
  ]);
  const id = parseJsonLines(tagged, "changed")?.new_snapshot_id;
  if (!id) throw new Error("有效恢复点标记失败");
  console.log(
    JSON.stringify({
      event: "backup_verified",
      kind,
      snapshotAt: startedAt.toISOString(),
      sha256: digest,
      id,
    }),
  );
  return id;
}

async function makeTemp() {
  await mkdir(tempRoot, { recursive: true, mode: 0o700 });
  return mkdtemp(join(tempRoot, "mindfolio-backup-"));
}

async function dumpDatabase(path) {
  const output = createWriteStream(path, { mode: 0o600 });
  try {
    await run(
      "docker",
      [
        "compose",
        "--env-file",
        deployEnv,
        "--file",
        "deploy/compose.app.yaml",
        "exec",
        "-T",
        "postgres",
        "pg_dump",
        "-U",
        "mindfolio",
        "-d",
        "mindfolio",
        "--format=custom",
        "--no-owner",
        "--no-acl",
      ],
      { output },
    );
    await new Promise((resolveWrite, reject) =>
      output.end((error) => (error ? reject(error) : resolveWrite())),
    );
    if ((await stat(path)).size === 0) throw new Error("数据库归档为空");
  } finally {
    output.destroy();
  }
}

async function databaseBackup() {
  const startedAt = new Date();
  const dir = await makeTemp();
  try {
    const path = join(dir, "mindfolio.dump");
    await dumpDatabase(path);
    return await saveFile("db", path, startedAt);
  } finally {
    await rm(dir, { recursive: true, force: true });
  }
}

async function copyStopped(composeFile, service, target, envFile = false) {
  const base = [
    "compose",
    ...(envFile ? ["--env-file", deployEnv] : []),
    "--file",
    composeFile,
  ];
  const id = (await run("docker", [...base, "ps", "-q", service])).trim();
  if (!id) throw new Error(`${service} 容器不存在`);
  await run("docker", [...base, "stop", service]);
  let copyError;
  try {
    await mkdir(target, { recursive: true, mode: 0o700 });
    const source = service === "edge" ? ["/data", "/config"] : ["/data"];
    for (const folder of source) {
      const destination = join(target, basename(folder));
      await mkdir(destination, { mode: 0o700 });
      await run("docker", ["cp", `${id}:${folder}/.`, destination]);
    }
  } catch (error) {
    copyError = error;
  } finally {
    await run("docker", [...base, "start", service]);
  }
  if (copyError) throw copyError;
}

async function stateBackup() {
  const startedAt = new Date();
  const dir = await makeTemp();
  try {
    const stage = join(dir, "state");
    await mkdir(stage, { mode: 0o700 });
    await copyStopped(
      "deploy/compose.portainer.yaml",
      "portainer",
      join(stage, "portainer"),
    );
    await copyStopped(
      "deploy/compose.edge.yaml",
      "edge",
      join(stage, "edge"),
      true,
    );
    await copyFile(deployEnv, join(stage, "production.env"));
    await copyFile(
      process.env.EDGE_CADDY_FILE || "/srv/mindfolio/edge/caddy/Caddyfile",
      join(stage, "Caddyfile"),
    );
    const revision = (await run("git", ["rev-parse", "HEAD"])).trim();
    await writeFile(
      join(stage, "manifest.json"),
      JSON.stringify({ snapshotAt: startedAt.toISOString(), revision }),
    );
    const archive = join(dir, "mindfolio-state.tar");
    const output = createWriteStream(archive, { mode: 0o600 });
    try {
      await run("tar", ["-C", stage, "-cf", "-", "."], { output });
      await new Promise((resolveWrite, reject) =>
        output.end((error) => (error ? reject(error) : resolveWrite())),
      );
    } finally {
      output.destroy();
    }
    return await saveFile("state", archive, startedAt);
  } finally {
    await rm(dir, { recursive: true, force: true });
  }
}

function validKind(items, kind) {
  return items
    .filter((item) => item.tags?.includes(`mindfolio-${kind}`))
    .sort((a, b) => Date.parse(b.time) - Date.parse(a.time));
}

async function checkAge() {
  const latest = validKind(await snapshots(), "db")[0];
  const ageMinutes = latest
    ? Math.floor((Date.now() - Date.parse(latest.time)) / 60000)
    : null;
  if (ageMinutes === null || ageMinutes >= warningMinutes) {
    const severity =
      ageMinutes === null || ageMinutes >= limitMinutes
        ? "critical"
        : "warning";
    console.error(
      JSON.stringify({
        event: "backup_age_stale",
        severity,
        ageMinutes,
        snapshotAt: latest?.time ?? null,
      }),
    );
    throw new Error(`数据库恢复点过旧：${ageMinutes ?? "无"} 分钟`);
  }
  console.log(
    JSON.stringify({
      event: "backup_age_ok",
      ageMinutes,
      snapshotAt: latest.time,
      id: latest.id,
    }),
  );
}

async function retryBackup(action) {
  let failure;
  for (let attempt = 1; attempt <= 3; attempt++) {
    try {
      await action();
      return;
    } catch (error) {
      failure = error;
      console.error(
        JSON.stringify({
          event: "backup_attempt_failed",
          attempt,
          message: error.message,
        }),
      );
      if (attempt < 3)
        await new Promise((resolveWait) =>
          setTimeout(
            resolveWait,
            (process.env.NODE_ENV === "test" ? 10 : 60000) * attempt,
          ),
        );
    }
  }
  throw failure;
}

async function prune() {
  const items = await snapshots();
  const db = validKind(items, "db");
  if (db.length === 0) throw new Error("没有有效数据库恢复点，拒绝清理");
  const state = validKind(items, "state");
  const dbSelection = selectRetainedSnapshots(db);
  const stateSelection = selectRetainedSnapshots(state);
  const removal = [...dbSelection.remove, ...stateSelection.remove];
  for (const item of removal) await restic(["forget", item.id]);
  if (removal.length > 0) await restic(["prune"]);
  console.log(
    JSON.stringify({
      event: "backup_pruned",
      removed: removal.length,
      retained: dbSelection.keep.length + stateSelection.keep.length,
    }),
  );
}

async function list() {
  for (const item of await snapshots()) {
    console.log(
      JSON.stringify({
        id: item.id,
        snapshotAt: item.time,
        kind: item.tags.find(
          (tag) => tag === "mindfolio-db" || tag === "mindfolio-state",
        ),
        sha256: item.tags.find((tag) => tag.startsWith("sha256-"))?.slice(7),
      }),
    );
  }
}

async function verify(id) {
  const item = (await snapshots()).find((entry) => entry.id === id);
  if (!item) throw new Error("找不到有效恢复点");
  const digest = item.tags.find((tag) => tag.startsWith("sha256-"))?.slice(7);
  if (
    !digest ||
    item.paths?.length !== 1 ||
    (await dumpHash(item.id, item.paths[0])) !== digest
  ) {
    throw new Error("恢复点完整性校验失败");
  }
  console.log(
    JSON.stringify({
      event: "backup_verify_ok",
      id,
      snapshotAt: item.time,
      sha256: digest,
    }),
  );
}

async function main() {
  requireConfig();
  switch (process.argv[2]) {
    case "init":
      await restic(["init"]);
      break;
    case "run":
      await retryBackup(databaseBackup);
      await checkAge();
      break;
    case "state":
      await retryBackup(stateBackup);
      break;
    case "age":
      await checkAge();
      break;
    case "prune":
      await prune();
      break;
    case "list":
      await list();
      break;
    case "verify":
      await verify(process.env.BACKUP_SNAPSHOT_ID);
      break;
    default:
      throw new Error(
        "用法：backup.mjs {init|run|state|age|prune|list|verify}",
      );
  }
}

main().catch((error) => {
  console.error(
    JSON.stringify({ event: "backup_error", message: error.message }),
  );
  process.exitCode = 1;
});
