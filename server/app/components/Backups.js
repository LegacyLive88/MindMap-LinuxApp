"use client";

import { useEffect, useState } from "react";
import { api } from "./api";
import Shell from "./Shell";

export default function Backups() {
  const [rows, setRows] = useState(null);
  const [label, setLabel] = useState("");
  const [notice, setNotice] = useState("");
  const [pending, setPending] = useState(null);

  async function load() {
    const data = await api("/api/v1/backups");
    setRows(data.backups || []);
  }

  useEffect(() => {
    load().catch((error) => setNotice(error.message));
  }, []);

  async function create(event) {
    event.preventDefault();
    setNotice("");
    try {
      await api("/api/v1/backups", {
        method: "POST",
        body: JSON.stringify({ label: label.trim() || "manual" }),
      });
      setLabel("");
      setNotice("Backup stored.");
      await load();
    } catch (error) {
      setNotice(error.message);
    }
  }

  async function restore(id) {
    setNotice("");
    try {
      const result = await api(`/api/v1/backups/${id}/restore`, { method: "POST" });
      setPending(null);
      setNotice(`Restored. The map is now revision ${result.revision}. The copy that was current is kept as a backup.`);
      await load();
    } catch (error) {
      setNotice(error.message);
    }
  }

  async function download(id) {
    try {
      const backup = await api(`/api/v1/backups/${id}`);
      const blob = new Blob([JSON.stringify(backup.library, null, 2)], { type: "application/json" });
      const url = URL.createObjectURL(blob);
      const link = document.createElement("a");
      link.href = url;
      link.download = `mindmap-backup-${id}.json`;
      link.click();
      URL.revokeObjectURL(url);
    } catch (error) {
      setNotice(error.message);
    }
  }

  return (
    <Shell>
      <h1>Backups</h1>
      <p className="lede">
        Every accepted upload from a desktop program stores the previous cloud copy automatically.
        You can also take a backup by hand and put one back.
      </p>
      {notice ? <p className="notice">{notice}</p> : null}
      <form className="row" onSubmit={create}>
        <label className="field" style={{ flex: 1, minWidth: 220 }}>
          Label
          <input type="text" value={label} onChange={(event) => setLabel(event.target.value)} placeholder="Before the reorganisation" />
        </label>
        <button className="primary" type="submit" style={{ alignSelf: "end" }}>Back up now</button>
      </form>
      <table className="backups">
        <thead>
          <tr>
            <th>When</th>
            <th>Label</th>
            <th>Revision</th>
            <th></th>
          </tr>
        </thead>
        <tbody>
          {(rows || []).map((row) => (
            <tr key={row.id}>
              <td>{new Date(row.created_at).toLocaleString()}</td>
              <td>{row.label}</td>
              <td>{row.revision}</td>
              <td className="row">
                <button className="quiet" type="button" onClick={() => download(row.id)}>Download</button>
                {pending === row.id ? (
                  <button className="danger" type="button" onClick={() => restore(row.id)}>Confirm restore</button>
                ) : (
                  <button className="quiet" type="button" onClick={() => setPending(row.id)}>Restore</button>
                )}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
      {rows && rows.length === 0 ? <p className="meta">No backups yet.</p> : null}
    </Shell>
  );
}
