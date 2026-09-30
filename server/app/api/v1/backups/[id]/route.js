import { NextResponse } from "next/server";
import { authed, databaseError, jsonError } from "../../../../../lib/http.js";
import { readBackup } from "../../../../../lib/sync.js";

export const dynamic = "force-dynamic";

export async function GET(request, { params }) {
  const { user, error } = await authed(request);
  if (error) return error;
  const id = Number(params.id);
  if (!Number.isInteger(id) || id < 1) return jsonError(404, "That backup was not found.");
  try {
    const backup = await readBackup(user.id, id);
    if (!backup) return jsonError(404, "That backup was not found.");
    return NextResponse.json(backup);
  } catch (failure) {
    return databaseError(failure);
  }
}
