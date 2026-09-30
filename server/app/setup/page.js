"use client";

import Link from "next/link";
import { useRouter } from "next/navigation";
import { useEffect, useState } from "react";
import { api } from "../components/api";

export default function SetupPage() {
  const router = useRouter();
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [confirm, setConfirm] = useState("");
  const [notice, setNotice] = useState("");
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    api("/api/v1/setup")
      .then((data) => {
        if (!data.needs_setup) router.replace("/login");
      })
      .catch((error) => setNotice(error.message));
  }, [router]);

  async function onSubmit(event) {
    event.preventDefault();
    setBusy(true);
    setNotice("");
    try {
      await api("/api/v1/setup", {
        method: "POST",
        body: JSON.stringify({ username: username.trim(), password, confirm }),
      });
      router.replace("/login");
    } catch (error) {
      setNotice(error.message);
      setBusy(false);
    }
  }

  return (
    <div className="gate">
      <form onSubmit={onSubmit}>
        <span className="dot" />
        <h1>First account</h1>
        <p className="lede">
          This page works only while the cloud has no accounts. After that, sign in and use the same
          username and password in the desktop program.
        </p>
        {notice ? <p className="notice">{notice}</p> : null}
        <div className="stack">
          <label className="field">
            Username
            <input type="text" value={username} autoComplete="username" onChange={(event) => setUsername(event.target.value)} />
          </label>
          <label className="field">
            Password
            <input type="password" value={password} autoComplete="new-password" onChange={(event) => setPassword(event.target.value)} />
          </label>
          <label className="field">
            Confirm password
            <input type="password" value={confirm} autoComplete="new-password" onChange={(event) => setConfirm(event.target.value)} />
          </label>
          <button className="primary" type="submit" disabled={busy}>Create account</button>
          <Link href="/login">I already have an account</Link>
        </div>
      </form>
    </div>
  );
}
