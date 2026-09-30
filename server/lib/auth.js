import crypto from "crypto";
import bcrypt from "bcryptjs";
import { getPool } from "./db.js";
import { sessionMs } from "./env.js";

const USERNAME = /^[A-Za-z0-9._-]{1,64}$/;
const WINDOW_MS = 15 * 60 * 1000;
const MAX_PER_USER = 8;
const MAX_PER_IP = 30;

let dummyHash;

async function dummy() {
  if (!dummyHash) dummyHash = await bcrypt.hash("mindmap-not-a-user", 12);
  return dummyHash;
}

export function checkUsername(username) {
  return typeof username === "string" && USERNAME.test(username);
}

export function checkPassword(password) {
  return typeof password === "string" && password.length >= 8 && password.length <= 200;
}

export function clientIp(request) {
  const forwarded = request.headers.get("x-forwarded-for");
  if (forwarded) {
    const first = forwarded.split(",")[0].trim();
    if (first) return first.slice(0, 64);
  }
  return "local";
}

export function tokenFrom(request) {
  const header = request.headers.get("authorization") || "";
  if (header.toLowerCase().startsWith("bearer ")) {
    return header.slice(7).trim();
  }
  const cookie = request.headers.get("cookie") || "";
  for (const part of cookie.split(";")) {
    const [name, ...rest] = part.trim().split("=");
    if (name === "mindmap_session") return decodeURIComponent(rest.join("="));
  }
  return "";
}

export async function userCount() {
  const [rows] = await getPool().query("SELECT COUNT(*) AS n FROM users");
  return Number(rows[0].n);
}

export async function createUser(username, password) {
  if (!checkUsername(username)) {
    throw new Error("Use a username of letters, numbers, dots, dashes, or underscores.");
  }
  if (!checkPassword(password)) {
    throw new Error("Use a password of at least 8 characters.");
  }
  const hash = await bcrypt.hash(password, 12);
  try {
    const [result] = await getPool().query(
      "INSERT INTO users (username, password_hash, created_at) VALUES (?, ?, ?)",
      [username, hash, Date.now()],
    );
    return { id: Number(result.insertId), username };
  } catch (error) {
    if (error && error.code === "ER_DUP_ENTRY") {
      throw new Error("That username is already in use.");
    }
    throw error;
  }
}

export async function login(username, password, ip) {
  const name = typeof username === "string" ? username : "";
  const secret = typeof password === "string" ? password : "";
  if (await tooManyAttempts(name, ip)) {
    return { ok: false, status: 429, error: "Too many sign-in attempts. Wait a few minutes and try again." };
  }
  const [rows] = await getPool().query(
    "SELECT id, username, password_hash FROM users WHERE username = ? LIMIT 1",
    [name],
  );
  const user = rows[0];
  const hash = user ? user.password_hash : await dummy();
  const match = secret ? await bcrypt.compare(secret, hash) : false;
  if (!user || !match) {
    await getPool().query(
      "INSERT INTO login_attempts (username, ip, attempted_at) VALUES (?, ?, ?)",
      [name.slice(0, 64), ip, Date.now()],
    );
    return { ok: false, status: 401, error: "The username or password was not accepted." };
  }
  await getPool().query("DELETE FROM login_attempts WHERE username = ?", [user.username]);
  const token = crypto.randomBytes(32).toString("hex");
  const expiresAt = Date.now() + sessionMs();
  await getPool().query(
    "INSERT INTO sessions (token, user_id, expires_at) VALUES (?, ?, ?)",
    [token, user.id, expiresAt],
  );
  return {
    ok: true,
    token,
    expiresAt,
    user: { id: Number(user.id), username: user.username },
  };
}

export async function requireUser(request) {
  const token = tokenFrom(request);
  if (!token || token.length > 128) return null;
  const [rows] = await getPool().query(
    `SELECT sessions.token, sessions.expires_at, users.id, users.username
     FROM sessions JOIN users ON users.id = sessions.user_id
     WHERE sessions.token = ? LIMIT 1`,
    [token],
  );
  const row = rows[0];
  if (!row) return null;
  if (Number(row.expires_at) < Date.now()) {
    await getPool().query("DELETE FROM sessions WHERE token = ?", [token]);
    return null;
  }
  return { id: Number(row.id), username: row.username, token };
}

export async function logout(token) {
  if (!token) return;
  await getPool().query("DELETE FROM sessions WHERE token = ?", [token]);
}

async function tooManyAttempts(username, ip) {
  const since = Date.now() - WINDOW_MS;
  const [byUser] = await getPool().query(
    "SELECT COUNT(*) AS n FROM login_attempts WHERE username = ? AND attempted_at > ?",
    [username, since],
  );
  if (Number(byUser[0].n) >= MAX_PER_USER) return true;
  const [byIp] = await getPool().query(
    "SELECT COUNT(*) AS n FROM login_attempts WHERE ip = ? AND attempted_at > ?",
    [ip, since],
  );
  return Number(byIp[0].n) >= MAX_PER_IP;
}

export function cookieSecure(request) {
  const forwarded = request.headers.get("x-forwarded-proto");
  if (forwarded) return forwarded.split(",")[0].trim() === "https";
  return request.nextUrl?.protocol === "https:";
}
