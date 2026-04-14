import { requireAuth, getDb, json } from "./_helpers.js";

export const config = { runtime: "edge" };

export default async function handler(req) {
  if (req.method !== "GET") return json({ error: "Method not allowed" }, 405);

  const denied = await requireAuth(req);
  if (denied) return denied;

  const url = new URL(req.url);
  const days = Math.min(parseInt(url.searchParams.get("days")) || 30, 365);
  const since = Date.now() - days * 24 * 60 * 60 * 1000;

  const sql = getDb();

  const [dauRows, sessRows] = await Promise.all([
    sql`SELECT TO_CHAR(TO_TIMESTAMP(timestamp_ms / 1000), 'YYYY-MM-DD') AS day,
                COUNT(DISTINCT anonymous_id) AS c
         FROM telemetry_events
         WHERE timestamp_ms > ${since}
         GROUP BY 1 ORDER BY 1`,
    sql`SELECT TO_CHAR(TO_TIMESTAMP(timestamp_ms / 1000), 'YYYY-MM-DD') AS day,
                COUNT(*) AS c
         FROM telemetry_events
         WHERE event = 'session_start' AND timestamp_ms > ${since}
         GROUP BY 1 ORDER BY 1`,
  ]);

  const formatLabel = (d) => d.slice(5); // "MM-DD"

  return json({
    dau: dauRows.map((r) => ({ label: formatLabel(r.day), value: parseInt(r.c) })),
    sessions: sessRows.map((r) => ({ label: formatLabel(r.day), value: parseInt(r.c) })),
  });
}
