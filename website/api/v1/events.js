import { neon } from "@neondatabase/serverless";

export const config = { runtime: "edge" };

const UUID_RE =
  /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i;

const MAX_BATCH_SIZE = 500;

export default async function handler(req) {
  // CORS preflight
  if (req.method === "OPTIONS") {
    return new Response(null, { status: 204, headers: corsHeaders() });
  }

  if (req.method !== "POST") {
    return json({ error: "Method not allowed" }, 405);
  }

  let body;
  try {
    body = await req.json();
  } catch {
    return json({ error: "Invalid JSON" }, 400);
  }

  // Validate shape
  const { anonymous_id, batch } = body;

  if (typeof anonymous_id !== "string" || !UUID_RE.test(anonymous_id)) {
    return json({ error: "Invalid anonymous_id" }, 400);
  }

  if (!Array.isArray(batch) || batch.length === 0) {
    return json({ error: "Batch must be a non-empty array" }, 400);
  }

  if (batch.length > MAX_BATCH_SIZE) {
    return json({ error: `Batch exceeds max size of ${MAX_BATCH_SIZE}` }, 400);
  }

  // Validate each event in the batch
  for (const entry of batch) {
    if (typeof entry.event !== "string" || !entry.event) {
      return json({ error: "Each entry must have an event string" }, 400);
    }
    if (typeof entry.timestamp_ms !== "number") {
      return json({ error: "Each entry must have a numeric timestamp_ms" }, 400);
    }
  }

  // Insert into Neon
  try {
    const sql = neon(process.env.DATABASE_URL);

    // Build parameterized batch insert
    const values = [];
    const params = [];
    let paramIdx = 1;

    for (const entry of batch) {
      const { event, timestamp_ms, ...payload } = entry;
      values.push(
        `($${paramIdx++}, $${paramIdx++}, $${paramIdx++}, $${paramIdx++})`
      );
      params.push(anonymous_id, event, JSON.stringify(payload), timestamp_ms);
    }

    await sql(
      `INSERT INTO telemetry_events (anonymous_id, event, payload, timestamp_ms)
       VALUES ${values.join(", ")}`,
      params
    );

    return json({ ok: true, inserted: batch.length }, 200);
  } catch (err) {
    console.error("Telemetry insert error:", err);
    return json({ error: "Internal server error" }, 500);
  }
}

function json(data, status) {
  return new Response(JSON.stringify(data), {
    status,
    headers: {
      "Content-Type": "application/json",
      ...corsHeaders(),
    },
  });
}

function corsHeaders() {
  return {
    "Access-Control-Allow-Origin": "*",
    "Access-Control-Allow-Methods": "POST, OPTIONS",
    "Access-Control-Allow-Headers": "Content-Type",
  };
}
