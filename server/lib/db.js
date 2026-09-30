import mysql from "mysql2/promise";
import { loadEnv } from "./env.js";

loadEnv();

let pool;

export function getPool() {
  if (!pool) {
    const database = process.env.MARIADB_DATABASE;
    const user = process.env.MARIADB_USER;
    if (!database || !user) {
      throw new Error("Set MARIADB_DATABASE and MARIADB_USER in the server .env file.");
    }
    pool = mysql.createPool({
      host: process.env.MARIADB_HOST || "localhost",
      port: Number(process.env.MARIADB_PORT || 3306),
      user,
      password: process.env.MARIADB_PASSWORD || "",
      database,
      waitForConnections: true,
      connectionLimit: 8,
      charset: "utf8mb4",
      timezone: "Z",
      dateStrings: true,
    });
  }
  return pool;
}

export async function closePool() {
  if (pool) {
    await pool.end();
    pool = null;
  }
}
