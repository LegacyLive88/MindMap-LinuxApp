import { NextResponse } from "next/server";
import { logout, tokenFrom } from "../../../../lib/auth.js";
import { databaseError } from "../../../../lib/http.js";

export const dynamic = "force-dynamic";

export async function POST(request) {
  try {
    await logout(tokenFrom(request));
  } catch (error) {
    return databaseError(error);
  }
  const response = NextResponse.json({ ok: true });
  response.cookies.set("mindmap_session", "", { httpOnly: true, path: "/", maxAge: 0 });
  return response;
}
