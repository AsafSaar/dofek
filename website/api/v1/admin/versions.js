import { requireAuth, getDb, json } from "./_helpers.js";

export const config = { runtime: "edge" };

export default async function handler(req) {
  if (req.method !== "GET") return json({ error: "Method not allowed" }, 405);

  const denied = await requireAuth(req);
  if (denied) return denied;

  const sql = getDb();

  const [appVersions, osVersions] = await Promise.all([
    sql`SELECT payload->>'app_version' AS name, COUNT(DISTINCT anonymous_id)::int AS count
        FROM telemetry_events WHERE event = 'session_start'
        GROUP BY 1 ORDER BY 1 DESC`,
    sql`SELECT payload->>'os_version' AS name, COUNT(DISTINCT anonymous_id)::int AS count
        FROM telemetry_events WHERE event = 'session_start'
        GROUP BY 1 ORDER BY 2 DESC`,
  ]);

  return json({
    app_versions: appVersions,
    os_versions: osVersions,
  });
}
