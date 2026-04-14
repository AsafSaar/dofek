import { signToken, json } from "./_helpers.js";

export const config = { runtime: "edge" };

export default async function handler(req) {
  if (req.method === "OPTIONS") {
    return new Response(null, { status: 204 });
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

  const { password } = body;
  if (!password || typeof password !== "string") {
    return json({ error: "Password required" }, 400);
  }

  const expected = process.env.ADMIN_PASSWORD;
  if (!expected || password !== expected) {
    return json({ error: "Invalid password" }, 401);
  }

  // 24-hour expiry
  const token = await signToken({ exp: Date.now() + 24 * 60 * 60 * 1000 });

  return json({ token });
}
