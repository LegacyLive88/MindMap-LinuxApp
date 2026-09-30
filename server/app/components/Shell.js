"use client";

import Link from "next/link";
import { usePathname, useRouter } from "next/navigation";
import { api } from "./api";

export default function Shell({ children }) {
  const path = usePathname();
  const router = useRouter();
  async function signOut() {
    await api("/api/v1/logout", { method: "POST" });
    router.replace("/login");
  }
  return (
    <div className="shell">
      <aside className="rail">
        <div className="brand"><span className="dot" /> MindMap</div>
        <p>The same maps as the desktop program, kept here when you are online.</p>
        <Link className={path === "/" ? "active" : ""} href="/">Canvases</Link>
        <Link className={path.startsWith("/backups") ? "active" : ""} href="/backups">Backups</Link>
        <div className="spacer" />
        <button className="linkish" type="button" onClick={signOut}>Log out</button>
      </aside>
      <main className="main">{children}</main>
    </div>
  );
}
