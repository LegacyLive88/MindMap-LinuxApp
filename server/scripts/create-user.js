import { createUser } from "../lib/auth.js";
import { closePool } from "../lib/db.js";
import { loadEnv } from "../lib/env.js";

loadEnv();

const username = process.argv[2];
const password = process.argv[3];
if (!username || !password) {
  console.error("Usage: npm run create-user -- <username> <password>");
  process.exit(1);
}

try {
  const user = await createUser(username, password);
  console.log(`Created ${user.username}.`);
} catch (error) {
  console.error(error.message || error);
  process.exitCode = 1;
} finally {
  await closePool();
}
