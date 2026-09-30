import crypto from "crypto";
import { getPool } from "./db.js";

const MAX_LIBRARY = 12_000_000;

export function libraryHash(library) {
  return crypto.createHash("sha256").update(JSON.stringify(library)).digest("hex");
}

export function assertLibrary(library) {
  if (!library || typeof library !== "object" || Array.isArray(library)) {
    throw new Error("The map was not a JSON object.");
  }
  if (typeof library.version !== "number") {
    throw new Error("The map is missing a version.");
  }
  if (!Array.isArray(library.root_order)) {
    throw new Error("The map is missing root_order.");
  }
  if (!library.canvases || typeof library.canvases !== "object" || Array.isArray(library.canvases)) {
    throw new Error("The map is missing canvases.");
  }
  if (JSON.stringify(library).length > MAX_LIBRARY) {
    throw new Error("The map is too large.");
  }
  return library;
}

export async function readLibrary(userId) {
  const [rows] = await getPool().query(
    "SELECT revision, content_hash, library_json, updated_at FROM documents WHERE user_id = ? LIMIT 1",
    [userId],
  );
  return present(rows[0]);
}

export async function pushLibrary({ userId, baseRevision, library, force, clientId }) {
  const hash = libraryHash(library);
  const conn = await getPool().getConnection();
  try {
    await conn.beginTransaction();
    const current = await lockDocument(conn, userId);
    if (!current) {
      await conn.query(
        "INSERT INTO documents (user_id, revision, content_hash, library_json, updated_at) VALUES (?, 1, ?, ?, ?)",
        [userId, hash, JSON.stringify(library), Date.now()],
      );
      await conn.commit();
      return { status: "accepted", revision: 1, hash };
    }
    if (current.hash === hash) {
      await conn.commit();
      return { status: "accepted", revision: current.revision, hash };
    }
    if (!force && Number(baseRevision) !== current.revision) {
      await conn.commit();
      return {
        status: "conflict",
        revision: current.revision,
        hash: current.hash,
        library: current.library,
        updated_at: current.updatedAt,
      };
    }
    const who = String(clientId || "web").slice(0, 40);
    const label = force ? `auto:replace:${who}` : `auto:sync:${who}`;
    await insertBackup(conn, userId, current, label);
    const revision = current.revision + 1;
    await conn.query(
      "UPDATE documents SET revision = ?, content_hash = ?, library_json = ?, updated_at = ? WHERE user_id = ?",
      [revision, hash, JSON.stringify(library), Date.now(), userId],
    );
    await pruneAuto(conn, userId);
    await conn.commit();
    return { status: "accepted", revision, hash };
  } catch (error) {
    await conn.rollback();
    throw error;
  } finally {
    conn.release();
  }
}

export async function createBackup(userId, label) {
  const conn = await getPool().getConnection();
  try {
    await conn.beginTransaction();
    const current = await lockDocument(conn, userId);
    if (!current) {
      await conn.rollback();
      return null;
    }
    const id = await insertBackup(conn, userId, current, cleanLabel(label) || "manual");
    await conn.commit();
    return { id, revision: current.revision, created_at: Date.now() };
  } catch (error) {
    await conn.rollback();
    throw error;
  } finally {
    conn.release();
  }
}

export async function listBackups(userId) {
  const [rows] = await getPool().query(
    "SELECT id, revision, label, created_at FROM backups WHERE user_id = ? ORDER BY id DESC LIMIT 100",
    [userId],
  );
  return rows.map((row) => ({
    id: Number(row.id),
    revision: Number(row.revision),
    label: row.label,
    created_at: Number(row.created_at),
  }));
}

export async function readBackup(userId, id) {
  const [rows] = await getPool().query(
    "SELECT id, revision, label, content_hash, library_json, created_at FROM backups WHERE user_id = ? AND id = ? LIMIT 1",
    [userId, id],
  );
  const row = rows[0];
  if (!row) return null;
  return {
    id: Number(row.id),
    revision: Number(row.revision),
    label: row.label,
    hash: row.content_hash,
    library: JSON.parse(row.library_json),
    created_at: Number(row.created_at),
  };
}

export async function restoreBackup(userId, id) {
  const conn = await getPool().getConnection();
  try {
    await conn.beginTransaction();
    const [rows] = await conn.query(
      "SELECT revision, content_hash, library_json FROM backups WHERE user_id = ? AND id = ? LIMIT 1 FOR UPDATE",
      [userId, id],
    );
    const backup = rows[0];
    if (!backup) {
      await conn.rollback();
      return null;
    }
    const library = JSON.parse(backup.library_json);
    const current = await lockDocument(conn, userId);
    if (current) {
      await insertBackup(conn, userId, current, "auto:before-restore");
      const revision = current.revision + 1;
      await conn.query(
        "UPDATE documents SET revision = ?, content_hash = ?, library_json = ?, updated_at = ? WHERE user_id = ?",
        [revision, backup.content_hash, backup.library_json, Date.now(), userId],
      );
      await pruneAuto(conn, userId);
      await conn.commit();
      return { status: "accepted", revision, hash: backup.content_hash, library };
    }
    await conn.query(
      "INSERT INTO documents (user_id, revision, content_hash, library_json, updated_at) VALUES (?, 1, ?, ?, ?)",
      [userId, backup.content_hash, backup.library_json, Date.now()],
    );
    await conn.commit();
    return { status: "accepted", revision: 1, hash: backup.content_hash, library };
  } catch (error) {
    await conn.rollback();
    throw error;
  } finally {
    conn.release();
  }
}

function present(row) {
  if (!row) return { revision: 0, hash: "", library: null, updated_at: null };
  return {
    revision: Number(row.revision),
    hash: row.content_hash,
    library: JSON.parse(row.library_json),
    updated_at: Number(row.updated_at),
  };
}

async function lockDocument(conn, userId) {
  const [rows] = await conn.query(
    "SELECT revision, content_hash, library_json, updated_at FROM documents WHERE user_id = ? LIMIT 1 FOR UPDATE",
    [userId],
  );
  const row = rows[0];
  if (!row) return null;
  return {
    revision: Number(row.revision),
    hash: row.content_hash,
    library: JSON.parse(row.library_json),
    updatedAt: Number(row.updated_at),
  };
}

async function insertBackup(conn, userId, current, label) {
  const [result] = await conn.query(
    "INSERT INTO backups (user_id, revision, label, content_hash, library_json, created_at) VALUES (?, ?, ?, ?, ?, ?)",
    [userId, current.revision, label.slice(0, 160), current.hash, JSON.stringify(current.library), Date.now()],
  );
  return Number(result.insertId);
}

async function pruneAuto(conn, userId) {
  await conn.query(
    `DELETE FROM backups
     WHERE user_id = ? AND label LIKE 'auto:%' AND id NOT IN (
       SELECT id FROM (
         SELECT id FROM backups WHERE user_id = ? AND label LIKE 'auto:%' ORDER BY id DESC LIMIT 40
       ) keepers
     )`,
    [userId, userId],
  );
}

function cleanLabel(label) {
  if (typeof label !== "string") return "";
  return label.replace(/\s+/g, " ").trim().slice(0, 160);
}
