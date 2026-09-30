import { NextResponse } from "next/server";
import { authed, databaseError, jsonError, readJson } from "../../../../lib/http.js";
import { assertLibrary, pushLibrary, readLibrary } from "../../../../lib/sync.js";

export const dynamic = "force-dynamic";

export async function GET(request) {
  const { user, error } = await authed(request);
  if (error) return error;
  try {
    return NextResponse.json(await readLibrary(user.id));
  } catch (failure) {
    return databaseError(failure);
  }
}

export async function POST(request) {
  const { user, error } = await authed(request);
  if (error) return error;
  const body = await readJson(request);
  if (!body || body.library == null) return jsonError(400, "Send the map.");
  let library;
  try {
    library = assertLibrary(body.library);
  } catch (failure) {
    return jsonError(400, failure.message);
  }
  try {
    const result = await pushLibrary({
      userId: user.id,
      baseRevision: Number(body.base_revision ?? 0),
      library,
      force: Boolean(body.force),
      clientId: typeof body.client_id === "string" ? body.client_id : "web",
    });
    const status = result.status === "conflict" ? 409 : 200;
    return NextResponse.json(result, { status });
  } catch (failure) {
    return databaseError(failure);
  }
}
