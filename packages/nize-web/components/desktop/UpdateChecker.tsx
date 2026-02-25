// @awa-component: PLAN-007-UpdateChecker
// @awa-impl: PLAN-007-6.3
// @awa-impl: PLAN-021 — ported from packages/nize-desktop/src/UpdateChecker.tsx

"use client";

import { useState, useEffect } from "react";
import { check } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";

type UpdateStatus = { kind: "idle" } | { kind: "checking" } | { kind: "available"; version: string } | { kind: "downloading"; progress: number } | { kind: "error"; message: string } | { kind: "restarting" };

// @awa-impl: PLAN-007-6.3
export function UpdateChecker() {
  const [status, setStatus] = useState<UpdateStatus>({ kind: "idle" });

  useEffect(() => {
    checkForUpdate();
  }, []);

  async function checkForUpdate() {
    setStatus({ kind: "checking" });
    try {
      const update = await check();
      if (update) {
        setStatus({ kind: "available", version: update.version });
      } else {
        setStatus({ kind: "idle" });
      }
    } catch {
      // Silently ignore update check failures (offline, no endpoint, etc.)
      setStatus({ kind: "idle" });
    }
  }

  // @awa-impl: PLAN-007-6.1
  async function installUpdate() {
    try {
      // PGlite data is just files on disk — no database dump needed before update.
      setStatus({ kind: "downloading", progress: 0 });
      const update = await check();
      if (!update) {
        setStatus({ kind: "idle" });
        return;
      }

      await update.downloadAndInstall((event) => {
        if (event.event === "Started" && event.data.contentLength) {
          setStatus({ kind: "downloading", progress: 0 });
        } else if (event.event === "Progress") {
          setStatus((prev) => {
            if (prev.kind === "downloading") {
              return { kind: "downloading", progress: prev.progress + (event.data.chunkLength ?? 0) };
            }
            return prev;
          });
        }
      });

      setStatus({ kind: "restarting" });
      await relaunch();
    } catch (e) {
      setStatus({ kind: "error", message: String(e) });
    }
  }

  if (status.kind === "idle" || status.kind === "checking") {
    return null;
  }

  return (
    <div
      style={{
        position: "fixed",
        bottom: "1rem",
        right: "1rem",
        padding: "1rem",
        background: "#1a1a2e",
        color: "#e0e0e0",
        borderRadius: "8px",
        boxShadow: "0 2px 8px rgba(0,0,0,0.3)",
        maxWidth: "320px",
        zIndex: 9999,
        fontFamily: "system-ui, sans-serif",
        fontSize: "14px",
      }}
    >
      {status.kind === "available" && (
        <>
          <p style={{ margin: "0 0 0.5rem" }}>
            <strong>Update available:</strong> v{status.version}
          </p>
          <button
            onClick={installUpdate}
            style={{
              background: "#4a9eff",
              color: "white",
              border: "none",
              padding: "0.4rem 1rem",
              borderRadius: "4px",
              cursor: "pointer",
            }}
          >
            Update now
          </button>
        </>
      )}

      {status.kind === "downloading" && <p style={{ margin: 0 }}>Downloading update…</p>}

      {status.kind === "restarting" && <p style={{ margin: 0 }}>Restarting…</p>}

      {status.kind === "error" && (
        <div>
          <p style={{ margin: "0 0 0.5rem", color: "#ff6b6b" }}>{status.message}</p>
          <button
            onClick={() => setStatus({ kind: "idle" })}
            style={{
              background: "transparent",
              color: "#999",
              border: "1px solid #555",
              padding: "0.3rem 0.8rem",
              borderRadius: "4px",
              cursor: "pointer",
            }}
          >
            Dismiss
          </button>
        </div>
      )}
    </div>
  );
}
