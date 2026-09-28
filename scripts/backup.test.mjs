import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import {
  mkdtemp,
  mkdir,
  readFile,
  writeFile,
  chmod,
  rm,
} from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { promisify } from "node:util";
import { fileURLToPath } from "node:url";
import test from "node:test";
import { selectRetainedSnapshots } from "./backup-retention.mjs";

const exec = promisify(execFile);
const root = resolve(fileURLToPath(new URL("..", import.meta.url)));
const hour = 3600_000;
const day = 24 * hour;

function snapshot(id, time) {
  return { id, time: new Date(time).toISOString() };
}

test("保留 24 小时内密集恢复点、跨界点、30 天内每日一点及最后一点", () => {
  const now = Date.parse("2026-09-28T12:00:00Z");
  const items = [
    snapshot("new", now - hour),
    snapshot("edge", now - day),
    snapshot("cross", now - day - 30 * 60_000),
    snapshot("same-day", now - day - hour),
    snapshot("daily", now - 2 * day),
    snapshot("old", now - 31 * day),
  ];
  const selected = selectRetainedSnapshots(items, new Date(now));
  assert.deepEqual(
    selected.keep.map((item) => item.id),
    ["new", "edge", "cross", "daily"],
  );
  assert.deepEqual(
    selected.remove.map((item) => item.id),
    ["same-day", "old"],
  );
  assert.deepEqual(
    selectRetainedSnapshots([items[5]], new Date(now)).keep.map(
      (item) => item.id,
    ),
    ["old"],
  );
});

async function fixture(t) {
  const dir = await mkdtemp(join(tmpdir(), "mindfolio-backup-test-"));
  t.after(() => rm(dir, { recursive: true, force: true }));
  const bin = join(dir, "bin");
  await mkdir(bin);
  const realRestic = (await exec("which", ["restic"])).stdout.trim();
  await writeFile(
    join(bin, "docker"),
    '#!/bin/sh\nif [ "$FAKE_DOCKER_FAIL" = 1 ]; then exit 2; fi\nif [ "$1" = cp ]; then printf copied > "$3/copied"; exit 0; fi\nfor arg do\n  case "$arg" in\n    pg_dump) printf "fake-custom-dump"; exit 0;;\n    ps) printf "fake-container-id"; exit 0;;\n    stop|start) exit 0;;\n  esac\ndone\nexit 2\n',
  );
  await writeFile(
    join(bin, "restic"),
    `#!/bin/sh\nif [ -n "$FAKE_RESTIC_CAPTURE" ]; then printf '%s\\n' "$@" > "$FAKE_RESTIC_CAPTURE"; printf '[]'; exit 0; fi\nfor arg do\n  if [ "$arg" = "$FAKE_RESTIC_FAIL" ]; then exit 3; fi\ndone\nexec '${realRestic}' "$@"\n`,
  );
  await chmod(join(bin, "docker"), 0o700);
  await chmod(join(bin, "restic"), 0o700);
  const password = join(dir, "password");
  await writeFile(password, "local-test-password", { mode: 0o600 });
  const env = {
    ...process.env,
    PATH: `${bin}:${process.env.PATH}`,
    NODE_ENV: "test",
    RESTIC_REPOSITORY: join(dir, "repository"),
    RESTIC_PASSWORD_FILE: password,
    BACKUP_TMP_DIR: join(dir, "runtime"),
    DEPLOY_ENV_FILE: join(dir, "production.env"),
  };
  await exec("restic", ["init"], { env });
  return { dir, env };
}

async function backup(args, env) {
  return exec("node", ["scripts/backup.mjs", ...args], { cwd: root, env });
}

test("OSS 命令使用虚拟主机访问和明确的地域", async (t) => {
  const { dir, env } = await fixture(t);
  const capture = join(dir, "restic-args");
  await backup(["list"], {
    ...env,
    RESTIC_REPOSITORY:
      "s3:https://s3.oss-cn-hangzhou.aliyuncs.com/private-backup/restic",
    AWS_DEFAULT_REGION: "cn-hangzhou",
    FAKE_RESTIC_CAPTURE: capture,
  });
  const args = await readFile(capture, "utf8");
  assert.match(args, /s3.bucket-lookup=dns/);
  assert.match(args, /s3.region=cn-hangzhou/);
});

test("数据库归档在读回校验后才列为有效恢复点", async (t) => {
  const { env } = await fixture(t);
  const result = await backup(["run"], env);
  const record = JSON.parse(
    result.stdout.split("\n").find((line) => line.includes("backup_verified")),
  );
  assert.equal(record.kind, "db");
  assert.match(record.sha256, /^[a-f0-9]{64}$/);
  const listed = await backup(["list"], env);
  assert.match(listed.stdout, new RegExp(record.id));
  const verified = await backup(["verify"], {
    ...env,
    BACKUP_SNAPSHOT_ID: record.id,
  });
  assert.match(verified.stdout, /backup_verify_ok/);
});

test("入口状态归档包含配置、版本与卷内容并可读回校验", async (t) => {
  const { dir, env } = await fixture(t);
  const caddyDir = join(dir, "caddy");
  await mkdir(join(caddyDir, "sites"), { recursive: true });
  await writeFile(join(caddyDir, "Caddyfile"), "import sites/*.caddy");
  await writeFile(
    join(caddyDir, "sites", "portainer.caddy"),
    "example.invalid { respond ok }",
  );
  await writeFile(env.DEPLOY_ENV_FILE, "API_IMAGE=sha256:test");
  const result = await backup(["state"], { ...env, EDGE_CADDY_DIR: caddyDir });
  const record = JSON.parse(
    result.stdout.split("\n").find((line) => line.includes("backup_verified")),
  );
  assert.equal(record.kind, "state");
  const verified = await backup(["verify"], {
    ...env,
    BACKUP_SNAPSHOT_ID: record.id,
  });
  assert.match(verified.stdout, /backup_verify_ok/);
  const snapshots = JSON.parse(
    (await exec("restic", ["snapshots", "--json"], { env })).stdout,
  );
  const path = snapshots.find((item) => item.id === record.id).paths[0];
  const archive = join(dir, "state.tar");
  const dumped = await exec("restic", ["dump", record.id, path], {
    env,
    encoding: "buffer",
  });
  await writeFile(archive, dumped.stdout);
  const files = (await exec("tar", ["-tf", archive])).stdout;
  assert.match(files, /\.\/caddy\/Caddyfile/);
  assert.match(files, /\.\/caddy\/sites\/portainer\.caddy/);
});

test("导出、上传和读回失败均重试、明确退出且不标记有效快照", async (t) => {
  const { env } = await fixture(t);
  for (const fault of ["FAKE_DOCKER_FAIL", "backup", "dump"]) {
    const faultEnv = {
      ...env,
      AWS_SECRET_ACCESS_KEY: "备份凭据不应出现在日志中",
    };
    if (fault === "FAKE_DOCKER_FAIL") faultEnv.FAKE_DOCKER_FAIL = "1";
    else faultEnv.FAKE_RESTIC_FAIL = fault;
    const failure = await backup(["run"], faultEnv).then(
      () => null,
      (error) => error,
    );
    assert(failure);
    const output = `${failure.stdout}${failure.stderr}`;
    assert.equal(
      (output.match(/"event":"backup_attempt_failed"/g) ?? []).length,
      3,
    );
    assert.match(output, /"event":"backup_error"/);
    assert.doesNotMatch(output, /备份凭据不应出现在日志中/);
    const listed = await backup(["list"], env);
    assert.equal(listed.stdout.trim(), "");
  }
});

test("过旧恢复点记录异常并以失败状态退出", async (t) => {
  const { dir, env } = await fixture(t);
  const path = join(dir, "old.dump");
  await writeFile(path, "old-data");
  const oldTime = new Date(Date.now() - 61 * 60_000)
    .toISOString()
    .slice(0, 19)
    .replace("T", " ");
  await exec(
    "restic",
    [
      "backup",
      "--time",
      oldTime,
      "--tag",
      "mindfolio-db",
      "--tag",
      "mindfolio-verified",
      path,
    ],
    { env: { ...env, TZ: "UTC" } },
  );
  const failure = await backup(["age"], env).then(
    () => null,
    (error) => error,
  );
  assert(failure);
  assert.match(failure.stderr, /"event":"backup_age_stale"/);
  assert.match(failure.stderr, /"severity":"critical"/);
  assert.match(failure.stderr, /数据库恢复点过旧/);
});
