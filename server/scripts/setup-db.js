import fs from "fs";
import path from "path";
import { fileURLToPath } from "url";
import { closePool, getPool } from "../lib/db.js";
import { loadEnv } from "../lib/env.js";

loadEnv();

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const sql = fs.readFileSync(path.join(root, "schema.sql"), "utf8");
const statements = sql
  .split("\n")
  .filter((line) => !line.trim().startsWith("--"))
  .join("\n")
  .split(";")
  .map((statement) => statement.trim())
  .filter(Boolean);

const pool = getPool();
for (const statement of statements) {
  await pool.query(statement);
}
console.log(`Applied ${statements.length} statements to ${process.env.MARIADB_DATABASE}.`);
await closePool();
