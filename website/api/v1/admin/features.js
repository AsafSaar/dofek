import { requireAuth, getDb, json } from "./_helpers.js";

export const config = { runtime: "edge" };

export default async function handler(req) {
  if (req.method !== "GET") return json({ error: "Method not allowed" }, 405);

  const denied = await requireAuth(req);
  if (denied) return denied;

  const sql = getDb();

  const [tabs, gpuPaths, filters, panels, modes, plugins] = await Promise.all([
    sql`SELECT payload->>'tab' AS name, COUNT(*)::int AS count
        FROM telemetry_events WHERE event = 'tab_switch'
        GROUP BY 1 ORDER BY 2 DESC`,
    sql`SELECT payload->>'path' AS name, COUNT(*)::int AS count
        FROM telemetry_events WHERE event = 'gpu_path'
        GROUP BY 1 ORDER BY 2 DESC`,
    sql`SELECT payload->>'filter' AS name, COUNT(*)::int AS count
        FROM telemetry_events WHERE event = 'filter_change'
        GROUP BY 1 ORDER BY 2 DESC`,
    sql`SELECT payload->>'panel' AS name, COUNT(*)::int AS count
        FROM telemetry_events WHERE event = 'panel_switch'
        GROUP BY 1 ORDER BY 2 DESC`,
    sql`SELECT payload->>'mode' AS name, COUNT(*)::int AS count
        FROM telemetry_events WHERE event = 'chart_mode_toggle'
        GROUP BY 1 ORDER BY 2 DESC`,
    sql`SELECT payload->>'plugin_name' AS name, COUNT(DISTINCT anonymous_id)::int AS count
        FROM telemetry_events WHERE event = 'plugin_used'
        GROUP BY 1 ORDER BY 2 DESC`,
  ]);

  return json({
    tabs,
    gpu_paths: gpuPaths,
    filters,
    panels,
    chart_modes: modes,
    plugins,
  });
}
