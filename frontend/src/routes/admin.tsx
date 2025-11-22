import { createFileRoute, redirect, useNavigate } from "@tanstack/react-router";
import { useState, useEffect } from "react";
import { ok } from "@atcute/client";
import { Trash2, Shield, AlertTriangle } from "lucide-react";
import { useQt } from "../lib/qt-provider";
import { Header, useScrollDetection } from "../components/Header";
import { Footer } from "../components/Footer";

export const Route = createFileRoute("/admin")({
  component: AdminPanel,
});

interface BlacklistedCid {
  cid: string;
  reason: string;
  reasonDetails?: string;
  contentType: string;
  moderatorDid: string;
  blacklistedAt: string;
}

const REASONS = [
  { value: "nudity", label: "Nudity" },
  { value: "gore", label: "Gore/Violence" },
  { value: "harassment", label: "Harassment" },
  { value: "spam", label: "Spam" },
  { value: "copyright", label: "Copyright Violation" },
  { value: "other", label: "Other" },
] as const;

const CONTENT_TYPES = [
  { value: "emoji_blob", label: "Emoji Blob" },
  { value: "avatar", label: "Avatar" },
  { value: "banner", label: "Banner" },
] as const;

function AdminPanel() {
  const { isLoggedIn, client } = useQt();
  const navigate = useNavigate();
  const isScrolled = useScrollDetection(20);

  const [isAdmin, setIsAdmin] = useState(false);
  const [loading, setLoading] = useState(true);
  const [blacklisted, setBlacklisted] = useState<BlacklistedCid[]>([]);
  const [loadingList, setLoadingList] = useState(true);

  // Form state
  const [cid, setCid] = useState("");
  const [reason, setReason] = useState<string>("other");
  const [reasonDetails, setReasonDetails] = useState("");
  const [contentType, setContentType] = useState<string>("emoji_blob");
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState<string | null>(null);

  useEffect(() => {
    checkAdminStatus();
  }, [isLoggedIn]);

  useEffect(() => {
    if (isAdmin) {
      fetchBlacklisted();
    }
  }, [isAdmin]);

  const checkAdminStatus = async () => {
    if (!isLoggedIn) {
      navigate({ to: "/" });
      return;
    }

    setLoading(true);
    try {
      const data = await ok(
        client.get("vg.nat.istat.moderation.isAdmin", {}),
      );

      if (!data.isAdmin) {
        alert("You are not authorized to access this page");
        navigate({ to: "/" });
        return;
      }

      setIsAdmin(true);
    } catch (err) {
      console.error("Failed to check admin status:", err);
      alert("Failed to verify admin status");
      navigate({ to: "/" });
    } finally {
      setLoading(false);
    }
  };

  const fetchBlacklisted = async () => {
    setLoadingList(true);
    try {
      const data = await ok(
        client.get("vg.nat.istat.moderation.listBlacklisted", {}),
      );
      setBlacklisted(data.blacklisted);
    } catch (err) {
      console.error("Failed to fetch blacklisted CIDs:", err);
      setError("Failed to load blacklisted content");
    } finally {
      setLoadingList(false);
    }
  };

  const handleBlacklist = async (e: React.FormEvent) => {
    e.preventDefault();
    setError(null);
    setSuccess(null);

    if (!cid.trim()) {
      setError("CID is required");
      return;
    }

    setSubmitting(true);
    try {
      await ok(
        client.call("vg.nat.istat.moderation.blacklistCid", {
          data: {
            cid: cid.trim(),
            reason,
            reasonDetails: reasonDetails.trim() || undefined,
            contentType,
          },
        }),
      );

      setSuccess("CID blacklisted successfully");
      setCid("");
      setReasonDetails("");
      fetchBlacklisted();
    } catch (err) {
      console.error("Failed to blacklist CID:", err);
      setError(
        err instanceof Error ? err.message : "Failed to blacklist CID",
      );
    } finally {
      setSubmitting(false);
    }
  };

  const handleRemoveBlacklist = async (cidToRemove: string) => {
    if (!confirm(`Remove blacklist for CID: ${cidToRemove}?`)) {
      return;
    }

    try {
      await ok(
        client.call("vg.nat.istat.moderation.removeBlacklist", {
          data: { cid: cidToRemove },
        }),
      );

      setSuccess("Blacklist removed successfully");
      fetchBlacklisted();
    } catch (err) {
      console.error("Failed to remove blacklist:", err);
      setError(
        err instanceof Error ? err.message : "Failed to remove blacklist",
      );
    }
  };

  if (loading) {
    return (
      <div className="min-h-screen relative">
        <Header isScrolled={isScrolled} />
        <main className="max-w-4xl mx-auto px-6 py-12">
          <div className="flex items-center justify-center min-h-[400px]">
            <div className="loading-spinner" />
          </div>
        </main>
        <Footer />
      </div>
    );
  }

  if (!isAdmin) {
    return null;
  }

  return (
    <div className="min-h-screen relative">
      <Header isScrolled={isScrolled} />

      <main className="max-w-4xl mx-auto px-6 py-12 relative z-10">
        <div className="flex items-center gap-3 mb-8">
          <Shield
            className="text-[rgb(var(--primary))]"
            size={32}
            strokeWidth={2}
          />
          <h1 className="text-3xl font-cursive text-[rgb(var(--foreground))]">
            moderation panel
          </h1>
        </div>

        {/* Alerts */}
        {error && (
          <div
            className="mb-6 p-4 rounded-lg border"
            style={{
              background: "rgba(var(--destructive), 0.1)",
              borderColor: "rgba(var(--destructive), 0.3)",
            }}
          >
            <div className="flex items-center gap-2">
              <AlertTriangle
                size={16}
                className="text-[rgb(var(--destructive))]"
              />
              <p className="text-sm text-[rgb(var(--destructive))]">{error}</p>
            </div>
          </div>
        )}

        {success && (
          <div
            className="mb-6 p-4 rounded-lg border"
            style={{
              background: "rgba(var(--primary), 0.1)",
              borderColor: "rgba(var(--primary), 0.3)",
            }}
          >
            <p className="text-sm text-[rgb(var(--primary))]">{success}</p>
          </div>
        )}

        {/* Blacklist Form */}
        <div
          className="p-6 rounded-lg border mb-8"
          style={{
            background: "rgba(var(--card), 0.6)",
            borderColor: "rgba(var(--border), 0.3)",
            backdropFilter: "blur(20px)",
          }}
        >
          <h2 className="text-xl font-serif mb-4 text-[rgb(var(--foreground))]">
            blacklist content
          </h2>
          <form onSubmit={handleBlacklist} className="space-y-4">
            <div>
              <label className="block text-sm font-serif mb-2 text-[rgb(var(--muted-foreground))]">
                CID
              </label>
              <input
                type="text"
                value={cid}
                onChange={(e) => setCid(e.target.value)}
                placeholder="bafyrei..."
                className="w-full px-4 py-2 text-sm rounded-lg bg-[rgb(var(--background))] text-[rgb(var(--foreground))] border"
                style={{ borderColor: "rgb(var(--input))" }}
                disabled={submitting}
              />
            </div>

            <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
              <div>
                <label className="block text-sm font-serif mb-2 text-[rgb(var(--muted-foreground))]">
                  Reason
                </label>
                <select
                  value={reason}
                  onChange={(e) => setReason(e.target.value)}
                  className="w-full px-4 py-2 text-sm rounded-lg bg-[rgb(var(--background))] text-[rgb(var(--foreground))] border"
                  style={{ borderColor: "rgb(var(--input))" }}
                  disabled={submitting}
                >
                  {REASONS.map((r) => (
                    <option key={r.value} value={r.value}>
                      {r.label}
                    </option>
                  ))}
                </select>
              </div>

              <div>
                <label className="block text-sm font-serif mb-2 text-[rgb(var(--muted-foreground))]">
                  Content Type
                </label>
                <select
                  value={contentType}
                  onChange={(e) => setContentType(e.target.value)}
                  className="w-full px-4 py-2 text-sm rounded-lg bg-[rgb(var(--background))] text-[rgb(var(--foreground))] border"
                  style={{ borderColor: "rgb(var(--input))" }}
                  disabled={submitting}
                >
                  {CONTENT_TYPES.map((ct) => (
                    <option key={ct.value} value={ct.value}>
                      {ct.label}
                    </option>
                  ))}
                </select>
              </div>
            </div>

            <div>
              <label className="block text-sm font-serif mb-2 text-[rgb(var(--muted-foreground))]">
                Additional Details (optional)
              </label>
              <textarea
                value={reasonDetails}
                onChange={(e) => setReasonDetails(e.target.value)}
                placeholder="Provide additional context..."
                rows={3}
                className="w-full px-4 py-2 text-sm rounded-lg bg-[rgb(var(--background))] text-[rgb(var(--foreground))] border resize-none"
                style={{ borderColor: "rgb(var(--input))" }}
                disabled={submitting}
              />
            </div>

            <button
              type="submit"
              disabled={submitting}
              className="px-6 py-2 text-sm rounded-full bg-[rgb(var(--primary))] text-[rgb(var(--primary-foreground))] hover:opacity-90 transition-all duration-200 disabled:opacity-50"
            >
              {submitting ? "blacklisting..." : "blacklist CID"}
            </button>
          </form>
        </div>

        {/* Blacklisted CIDs List */}
        <div
          className="p-6 rounded-lg border"
          style={{
            background: "rgba(var(--card), 0.6)",
            borderColor: "rgba(var(--border), 0.3)",
            backdropFilter: "blur(20px)",
          }}
        >
          <h2 className="text-xl font-serif mb-4 text-[rgb(var(--foreground))]">
            blacklisted content ({blacklisted.length})
          </h2>

          {loadingList ? (
            <div className="flex items-center justify-center py-12">
              <div className="loading-spinner" />
            </div>
          ) : blacklisted.length === 0 ? (
            <p className="text-center py-12 text-[rgb(var(--muted-foreground))]">
              no blacklisted content
            </p>
          ) : (
            <div className="space-y-3">
              {blacklisted.map((item) => (
                <div
                  key={item.cid}
                  className="p-4 rounded-lg border flex items-start justify-between gap-4"
                  style={{
                    background: "rgba(var(--background), 0.5)",
                    borderColor: "rgba(var(--border), 0.3)",
                  }}
                >
                  <div className="flex-1 min-w-0">
                    <div className="flex items-center gap-2 mb-2">
                      <span
                        className="px-2 py-1 text-xs rounded-full"
                        style={{
                          background: "rgba(var(--destructive), 0.2)",
                          color: "rgb(var(--destructive))",
                        }}
                      >
                        {item.reason}
                      </span>
                      <span
                        className="px-2 py-1 text-xs rounded-full"
                        style={{
                          background: "rgba(var(--primary), 0.2)",
                          color: "rgb(var(--primary))",
                        }}
                      >
                        {item.contentType.replace("_", " ")}
                      </span>
                    </div>
                    <p className="text-xs font-mono text-[rgb(var(--foreground))] break-all mb-2">
                      {item.cid}
                    </p>
                    {item.reasonDetails && (
                      <p className="text-xs text-[rgb(var(--muted-foreground))] mb-2">
                        {item.reasonDetails}
                      </p>
                    )}
                    <p className="text-xs text-[rgb(var(--muted-foreground))]">
                      {new Date(item.blacklistedAt).toLocaleString()}
                    </p>
                  </div>
                  <button
                    onClick={() => handleRemoveBlacklist(item.cid)}
                    className="p-2 rounded-lg hover:bg-[rgba(var(--destructive),0.1)] text-[rgb(var(--muted-foreground))] hover:text-[rgb(var(--destructive))] transition-colors"
                    title="Remove blacklist"
                  >
                    <Trash2 size={16} />
                  </button>
                </div>
              ))}
            </div>
          )}
        </div>
      </main>

      <Footer />
    </div>
  );
}
