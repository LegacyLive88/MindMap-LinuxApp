import { NextResponse } from "next/server";
import { authed, databaseError, jsonError, readJson } from "../../../../lib/http.js";
import { createBackup, listBackups } from "../../../../lib/sync.js";

export const dynamic = "force-dynamic";

export async function GET(request) {
  const { user, error } = await authed(request);
  if (error) return error;
  try {
    return NextResponse.json({ backups: await listBackups(user.id) });
  } catch (failure) {
    return databaseError(failure);
  }
}

export async function POST(request) {
  const { user, error } = await authed(request);
  if (error) return error;
  const body = (await readJson(request)) || {};
  try {
    const backup = await createBackup(user.id, body.label);
    if (!backup) return jsonError(404, "The cloud does not have a map to back up yet.");
    return NextResponse.json(backup);
  } catch (failure) {
    return databaseError(failure);
  }
}
