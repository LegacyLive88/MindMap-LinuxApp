import { NextResponse } from "next/server";
import { cookieSecure, requireUser } from "./auth.js";

export function jsonError(status, error) {
  return NextResponse.json({ error }, { status });
}

export async function readJson(request) {
  try {
    return await request.json();
  } catch {
    return null;
  }
}

export async function authed(request) {
  try {
    const user = await requireUser(request);
    if (!user) return { user: null, error: jsonError(401, "Sign in again.") };
    return { user, error: null };
  } catch (error) {
    return { user: null, error: databaseError(error) };
  }
}

export function databaseError(error) {
  const message = error && error.code === "ER_NO_SUCH_TABLE"
    ? "The database tables are missing. Run npm run setup on the server."
    : "The database could not be reached.";
  return jsonError(500, message);
}

export function sessionCookie(request, token, expiresAt) {
  const maxAge = Math.max(0, Math.floor((expiresAt - Date.now()) / 1000));
  return {
    name: "mindmap_session",
    value: token,
    options: {
      httpOnly: true,
      sameSite: "lax",
      secure: cookieSecure(request),
      path: "/",
      maxAge,
    },
  };
}
