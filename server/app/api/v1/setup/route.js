import { NextResponse } from "next/server";
import { createUser, userCount } from "../../../../lib/auth.js";
import { databaseError, jsonError, readJson } from "../../../../lib/http.js";

export const dynamic = "force-dynamic";

export async function GET() {
  try {
    const count = await userCount();
    return NextResponse.json({ needs_setup: count === 0 });
  } catch (error) {
    return databaseError(error);
  }
}

export async function POST(request) {
  const body = await readJson(request);
  if (!body) return jsonError(400, "Send a username and password.");
  if (body.password !== body.confirm) {
    return jsonError(400, "The two passwords do not match.");
  }
  try {
    if ((await userCount()) > 0) {
      return jsonError(403, "An account already exists. Sign in instead.");
    }
    const user = await createUser(String(body.username || "").trim(), body.password);
    return NextResponse.json({ username: user.username });
  } catch (error) {
    if (error && error.code) return databaseError(error);
    return jsonError(400, error.message || "Could not create the account.");
  }
}
