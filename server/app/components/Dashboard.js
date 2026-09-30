"use client";

import Link from "next/link";
import { useRouter } from "next/navigation";
import { useEffect, useState } from "react";
import { createRoot, emptyLibrary, labelFor, rootCanvases } from "../../lib/library";
import { api, saveLibrary } from "./api";
import Shell from "./Shell";

export default function Dashboard() {
  const router = useRouter();
  const [sync, setSync] = useState(null);
  const [name, setName] = useState("");
  const [notice, setNotice] = useState("");
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    let gone = false;
    api("/api/v1/sync")
      .then((data) => {
        if (!gone) setSync(data);
      })
      .catch((error) => {
        if (!gone) setNotice(error.message);
      });
    return () => {
      gone = true;
    };
  }, []);

  async function onCreate(event) {
    event.preventDefault();
    setBusy(true);
    setNotice("");
    const base = sync?.library || emptyLibrary();
    const created = createRoot(base, name.trim());
    try {
      await saveLibrary(created.library, sync?.revision || 0);
      router.push(`/map/${created.canvasId}`);
    } catch (error) {
      setNotice(error.message);
      setBusy(false);
    }
  }

  const roots = sync?.library ? rootCanvases(sync.library) : [];
  return (
    <Shell>
      <h1>Canvases</h1>
      <p className="lede">
        Each canvas starts as one circle. Edits here are stored on the cloud and come back to the
        desktop program the next time it can connect.
      </p>
      {notice ? <p className="notice">{notice}</p> : null}
      <form className="row" onSubmit={onCreate}>
        <label className="field" style={{ flex: 1, minWidth: 220 }}>
          New canvas
          <input
            type="text"
            value={name}
            onChange={(event) => setName(event.target.value)}
            placeholder="Name the centre circle"
          />
        </label>
        <button className="primary" type="submit" disabled={busy || !sync} style={{ alignSelf: "end" }}>
          Create
        </button>
      </form>
      <p className="meta">
        {sync
          ? `Cloud revision ${sync.revision}${sync.updated_at ? ` · ${new Date(sync.updated_at).toLocaleString()}` : ""}`
          : "Loading the cloud copy…"}
      </p>
      <ul className="canvas-list">
        {sync && roots.length === 0 ? <li className="meta">No canvases yet.</li> : null}
        {roots.map((canvas) => {
          const center = canvas.nodes.find((node) => node.id === canvas.center_id);
          return (
            <li key={canvas.id}>
              <Link href={`/map/${canvas.id}`}>
                <strong>{labelFor(center?.text)}</strong>
                <span>{canvas.nodes.length} ideas</span>
              </Link>
            </li>
          );
        })}
      </ul>
    </Shell>
  );
}
