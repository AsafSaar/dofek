import { neon } from "@neondatabase/serverless";

/** Validate Bearer token from Authorization header. Returns true if valid. */
export async function validateToken(req) {
  const auth = req.headers.get("Authorization") || "";
  const token = auth.replace(/^Bearer\s+/i, "");
  if (!token) return false;

  try {
    const [payloadB64, sigB64] = token.split(".");
    if (!payloadB64 || !sigB64) return false;

    const secret = process.env.ADMIN_SECRET;
    if (!secret) return false;

    const key = await crypto.subtle.importKey(
      "raw",
      new TextEncoder().encode(secret),
      { name: "HMAC", hash: "SHA-256" },
      false,
      ["verify"]
    );

    const valid = await crypto.subtle.verify(
      "HMAC",
      key,
      base64UrlToBuffer(sigB64),
      new TextEncoder().encode(payloadB64)
    );

    if (!valid) return false;

    const payload = JSON.parse(atob(payloadB64));
    if (!payload.exp || Date.now() > payload.exp) return false;

    return true;
  } catch {
    return false;
  }
}

/** Sign a payload and return a token string. */
export async function signToken(payload) {
  const secret = process.env.ADMIN_SECRET;
  const key = await crypto.subtle.importKey(
    "raw",
    new TextEncoder().encode(secret),
    { name: "HMAC", hash: "SHA-256" },
    false,
    ["sign"]
  );

  const payloadB64 = btoa(JSON.stringify(payload));
  const sig = await crypto.subtle.sign(
    "HMAC",
    key,
    new TextEncoder().encode(payloadB64)
  );

  return `${payloadB64}.${bufferToBase64Url(sig)}`;
}

/** Get a Neon SQL tagged-template function. */
export function getDb() {
  return neon(process.env.DATABASE_URL);
}

/** JSON response helper. */
export function json(data, status = 200) {
  return new Response(JSON.stringify(data), {
    status,
    headers: { "Content-Type": "application/json" },
  });
}

/** Return 401 if token is invalid. Returns null if valid (caller proceeds). */
export async function requireAuth(req) {
  if (!(await validateToken(req))) {
    return json({ error: "Unauthorized" }, 401);
  }
  return null;
}

// -- base64url helpers --
function base64UrlToBuffer(b64url) {
  const b64 = b64url.replace(/-/g, "+").replace(/_/g, "/");
  const bin = atob(b64);
  const buf = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) buf[i] = bin.charCodeAt(i);
  return buf.buffer;
}

function bufferToBase64Url(buf) {
  const bytes = new Uint8Array(buf);
  let bin = "";
  for (const b of bytes) bin += String.fromCharCode(b);
  return btoa(bin).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
}
