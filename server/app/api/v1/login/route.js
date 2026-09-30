import { NextResponse } from "next/server";
import { clientIp, login } from "../../../../lib/auth.js";
import { databaseError, jsonError, readJson, sessionCookie } from "../../../../lib/http.js";

export const dynamic = "force-dynamic";

export async function POST(request) {
  const body = await readJson(request);
  if (!body) return jsonError(400, "Send a username and password.");
  try {
    const result = await login(body.username, body.password, clientIp(request));
    if (!result.ok) return jsonError(result.status, result.error);
    const response = NextResponse.json({
      token: result.token,
      expires_at: new Date(result.expiresAt).toISOString(),
      username: result.user.username,
    });
    const cookie = sessionCookie(request, result.token, result.expiresAt);
    response.cookies.set(cookie.name, cookie.value, cookie.options);
    return response;
  } catch (error) {
    return databaseError(error);
  }
}
