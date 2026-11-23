// Device tokens: "v1.<payload>.<signature>", HMAC-SHA256 signed by the Worker.
// The payload holds no personal data: a random install id, the UTC offset
// (so the quota resets at the user's midnight) and the expiry.

export interface DeviceClaims {
  /** Random install id (UUID). */
  i: string;
  /** Minutes east of UTC. */
  o: number;
  /** Issued and expiry, Unix ms. */
  iat: number;
  exp: number;
}

const encoder = new TextEncoder();
export const TOKEN_LIFETIME_MS = 365 * 24 * 60 * 60 * 1000;

function base64url(bytes: Uint8Array): string {
  let s = "";
  for (const b of bytes) s += String.fromCharCode(b);
  return btoa(s).replaceAll("+", "-").replaceAll("/", "_").replace(/=+$/, "");
}

function fromBase64url(text: string): Uint8Array {
  const s = atob(text.replaceAll("-", "+").replaceAll("_", "/"));
  return Uint8Array.from(s, (c) => c.charCodeAt(0));
}

type HmacKey = Awaited<ReturnType<typeof crypto.subtle.importKey>>;

async function key(secret: string): Promise<HmacKey> {
  return crypto.subtle.importKey("raw", encoder.encode(secret), { name: "HMAC", hash: "SHA-256" }, false, ["sign", "verify"]);
}

export async function sign(claims: DeviceClaims, secret: string): Promise<string> {
  const payload = base64url(encoder.encode(JSON.stringify(claims)));
  const signature = await crypto.subtle.sign("HMAC", await key(secret), encoder.encode(`v1.${payload}`));
  return `v1.${payload}.${base64url(new Uint8Array(signature))}`;
}

/** The claims if the token is ours, intact and unexpired; otherwise null. */
export async function verify(token: string, secret: string, now: number): Promise<DeviceClaims | null> {
  const parts = token.split(".");
  if (parts.length !== 3 || parts[0] !== "v1" || token.length > 1024) return null;
  try {
    const ok = await crypto.subtle.verify(
      "HMAC",
      await key(secret),
      fromBase64url(parts[2]),
      encoder.encode(`v1.${parts[1]}`),
    );
    if (!ok) return null;
    const claims = JSON.parse(new TextDecoder().decode(fromBase64url(parts[1]))) as DeviceClaims;
    if (typeof claims.i !== "string" || typeof claims.exp !== "number" || claims.exp <= now) return null;
    return claims;
  } catch {
    return null;
  }
}
