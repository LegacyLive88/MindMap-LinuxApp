"use client";

import Link from "next/link";
import { useRouter } from "next/navigation";
import { useEffect, useState } from "react";
import { api } from "../components/api";

export default function LoginPage() {
  const router = useRouter();
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [notice, setNotice] = useState("");
  const [needsSetup, setNeedsSetup] = useState(false);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    api("/api/v1/setup")
      .then((data) => setNeedsSetup(Boolean(data.needs_setup)))
      .catch(() => {});
  }, []);

  async function onSubmit(event) {
    event.preventDefault();
    setBusy(true);
    setNotice("");
    try {
      await api("/api/v1/login", {
        method: "POST",
        body: JSON.stringify({ username: username.trim(), password }),
      });
      router.replace("/");
    } catch (error) {
      setNotice(error.message);
      setBusy(false);
    }
  }

  return (
    <div className="gate">
      <form onSubmit={onSubmit}>
        <span className="dot" />
        <h1>Sign in</h1>
        <p className="lede">Use the username and password for this MindMap cloud.</p>
        {notice ? <p className="notice">{notice}</p> : null}
        <div className="stack">
          <label className="field">
            Username
            <input type="text" value={username} autoComplete="username" onChange={(event) => setUsername(event.target.value)} />
          </label>
          <label className="field">
            Password
            <input type="password" value={password} autoComplete="current-password" onChange={(event) => setPassword(event.target.value)} />
          </label>
          <button className="primary" type="submit" disabled={busy}>Sign in</button>
        </div>
        {needsSetup ? <p><Link href="/setup">Create the first account</Link></p> : null}
      </form>
    </div>
  );
}
