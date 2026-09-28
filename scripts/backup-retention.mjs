const HOUR = 60 * 60 * 1000;
const DAY = 24 * HOUR;

export function selectRetainedSnapshots(snapshots, now = new Date()) {
  const current = now.getTime();
  const recentBoundary = current - DAY;
  const historyBoundary = current - 30 * DAY;
  const ordered = [...snapshots].sort(
    (a, b) => Date.parse(b.time) - Date.parse(a.time),
  );
  const kept = new Set();
  const daily = new Set();
  let boundaryKept = false;

  for (const snapshot of ordered) {
    const time = Date.parse(snapshot.time);
    if (!Number.isFinite(time) || time > current) {
      throw new Error("恢复点时间无效");
    }
    if (time >= recentBoundary) {
      kept.add(snapshot.id);
      continue;
    }
    if (!boundaryKept) {
      kept.add(snapshot.id);
      boundaryKept = true;
    }
    if (time >= historyBoundary) {
      const date = new Date(time).toISOString().slice(0, 10);
      if (!daily.has(date)) {
        daily.add(date);
        kept.add(snapshot.id);
      }
    }
  }
  if (ordered.length > 0 && kept.size === 0) kept.add(ordered[0].id);
  return {
    keep: ordered.filter((item) => kept.has(item.id)),
    remove: ordered.filter((item) => !kept.has(item.id)),
  };
}
