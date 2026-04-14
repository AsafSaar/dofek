import { requireAuth, getDb, json } from "./_helpers.js";

export const config = { runtime: "edge" };

export default async function handler(req) {
  if (req.method !== "GET") return json({ error: "Method not allowed" }, 405);

  const denied = await requireAuth(req);
  if (denied) return denied;

  const sql = getDb();

  const now = Date.now();
  const ms7d = now - 7 * 24 * 60 * 60 * 1000;
  const ms24h = now - 24 * 60 * 60 * 1000;

  const [users, active7d, active24h, sessions, avgDur, iface] =
    await Promise.all([
      sql`SELECT COUNT(DISTINCT anonymous_id) AS c FROM telemetry_events`,
      sql`SELECT COUNT(DISTINCT anonymous_id) AS c FROM telemetry_events WHERE timestamp_ms > ${ms7d}`,
      sql`SELECT COUNT(DISTINCT anonymous_id) AS c FROM telemetry_events WHERE timestamp_ms > ${ms24h}`,
      sql`SELECT COUNT(*) AS c FROM telemetry_events WHERE event = 'session_start'`,
      sql`SELECT AVG((payload->>'duration_secs')::float) AS avg FROM telemetry_events WHERE event = 'session_end'`,
      sql`SELECT payload->>'interface' AS iface, COUNT(*) AS c FROM telemetry_events WHERE event = 'session_start' GROUP BY 1`,
    ]);

  const ifaceMap = {};
  for (const row of iface) ifaceMap[row.iface] = parseInt(row.c);

  return json({
    total_users: parseInt(users[0].c),
    active_7d: parseInt(active7d[0].c),
    active_24h: parseInt(active24h[0].c),
    total_sessions: parseInt(sessions[0].c),
    avg_duration_secs: avgDur[0].avg ? Math.round(parseFloat(avgDur[0].avg)) : null,
    tui_count: ifaceMap.tui || 0,
    gui_count: ifaceMap.gui || 0,
  });
}
