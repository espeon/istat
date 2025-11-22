import { createFileRoute, useNavigate } from "@tanstack/react-router";
import { useState, useEffect, useMemo } from "react";
import { ok } from "@atcute/client";
import { Trash2, Shield, Search, Copy, TrendingUp } from "lucide-react";
import { useQt } from "../lib/qt-provider";
import { useToast } from "../lib/toast";
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
  const { isLoggedIn, client, did } = useQt();
  const navigate = useNavigate();
  const toast = useToast();
  const isScrolled = useScrollDetection(20);

  const [isAdmin, setIsAdmin] = useState(false);
  const [loading, setLoading] = useState(true);
  const [blacklisted, setBlacklisted] = useState<BlacklistedCid[]>([]);
  const [loadingList, setLoadingList] = useState(true);

  // Search/filter state
  const [searchQuery, setSearchQuery] = useState("");
  const [filterReason, setFilterReason] = useState<string>("all");
  const [filterContentType, setFilterContentType] = useState<string>("all");

  // Form state
  const [cid, setCid] = useState("");
  const [reason, setReason] = useState<string>("other");
  const [reasonDetails, setReasonDetails] = useState("");
  const [contentType, setContentType] = useState<string>("emoji_blob");
  const [submitting, setSubmitting] = useState(false);

  useEffect(() => {
    checkAdminStatus();
  }, [isLoggedIn]);

  useEffect(() => {
    if (isAdmin) {
      fetchBlacklisted();
    }
  }, [isAdmin]);

  // Filtered blacklisted items
  const filteredBlacklisted = useMemo(() => {
    return blacklisted.filter((item) => {
      const matchesSearch =
        searchQuery === "" ||
        item.cid.toLowerCase().includes(searchQuery.toLowerCase()) ||
        item.reasonDetails?.toLowerCase().includes(searchQuery.toLowerCase());

      const matchesReason = filterReason === "all" || item.reason === filterReason;
      const matchesContentType = filterContentType === "all" || item.contentType === filterContentType;

      return matchesSearch && matchesReason && matchesContentType;
    });
  }, [blacklisted, searchQuery, filterReason, filterContentType]);

  // Stats
  const stats = useMemo(() => {
    const now = new Date();
    const today = new Date(now.getFullYear(), now.getMonth(), now.getDate());

    return {
      total: blacklisted.length,
      today: blacklisted.filter(
        (item) => new Date(item.blacklistedAt) >= today
      ).length,
      byReason: blacklisted.reduce((acc, item) => {
        acc[item.reason] = (acc[item.reason] || 0) + 1;
        return acc;
      }, {} as Record<string, number>),
    };
  }, [blacklisted]);

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
        toast.error("You are not authorized to access this page");
        navigate({ to: "/" });
        return;
      }

      setIsAdmin(true);
    } catch (err) {
      console.error("Failed to check admin status:", err);
      toast.error("Failed to verify admin status");
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
      toast.error("Failed to load blacklisted content");
    } finally {
      setLoadingList(false);
    }
  };

  const handleBlacklist = async (e: React.FormEvent) => {
    e.preventDefault();

    if (!cid.trim()) {
      toast.error("CID is required");
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

      toast.success("CID blacklisted successfully");
      setCid("");
      setReasonDetails("");
      fetchBlacklisted();
    } catch (err) {
      console.error("Failed to blacklist CID:", err);
      toast.error(
        err instanceof Error ? err.message : "Failed to blacklist CID",
      );
    } finally {
      setSubmitting(false);
    }
  };

  const handleRemoveBlacklist = async (cidToRemove: string) => {
    try {
      await ok(
        client.call("vg.nat.istat.moderation.removeBlacklist", {
          data: { cid: cidToRemove },
        }),
      );

      toast.success("Blacklist removed successfully");
      fetchBlacklisted();
    } catch (err) {
      console.error("Failed to remove blacklist:", err);
      toast.error(
        err instanceof Error ? err.message : "Failed to remove blacklist",
      );
    }
  };

  const copyCid = async (cidToCopy: string) => {
    try {
      await navigator.clipboard.writeText(cidToCopy);
      toast.success("CID copied to clipboard");
    } catch (err) {
      toast.error("Failed to copy CID");
    }
  };

  const fillExampleUri = () => {
    if (did) {
      setCid(`Example: at://${did}/vg.nat.istat.moji.emoji/3lbxyz123abc`);
      toast.info("Paste the actual CID from content you want to blacklist");
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
        {/* Header */}
        <div className="flex items-center justify-between mb-8">
          <div className="flex items-center gap-3">
            <Shield
              className="text-[rgb(var(--primary))]"
              size={32}
              strokeWidth={2}
            />
            <h1 className="text-3xl font-cursive text-[rgb(var(--foreground))]">
              moderation panel
            </h1>
          </div>
        </div>

        {/* Stats */}
        <div className="grid grid-cols-1 md:grid-cols-3 gap-4 mb-8">
          <div
            className="p-4 rounded-lg border"
            style={{
              background: "rgba(var(--card), 0.6)",
              borderColor: "rgba(var(--border), 0.3)",
              backdropFilter: "blur(20px)",
            }}
          >
            <div className="flex items-center gap-2 mb-1">
              <TrendingUp size={16} className="text-[rgb(var(--primary))]" />
              <p className="text-xs font-serif text-[rgb(var(--muted-foreground))]">
                Total Blacklisted
              </p>
            </div>
            <p className="text-2xl font-cursive text-[rgb(var(--foreground))]">
              {stats.total}
            </p>
          </div>

          <div
            className="p-4 rounded-lg border"
            style={{
              background: "rgba(var(--card), 0.6)",
              borderColor: "rgba(var(--border), 0.3)",
              backdropFilter: "blur(20px)",
            }}
          >
            <p className="text-xs font-serif text-[rgb(var(--muted-foreground))] mb-1">
              Blacklisted Today
            </p>
            <p className="text-2xl font-cursive text-[rgb(var(--foreground))]">
              {stats.today}
            </p>
          </div>

          <div
            className="p-4 rounded-lg border"
            style={{
              background: "rgba(var(--card), 0.6)",
              borderColor: "rgba(var(--border), 0.3)",
              backdropFilter: "blur(20px)",
            }}
          >
            <p className="text-xs font-serif text-[rgb(var(--muted-foreground))] mb-1">
              Top Reason
            </p>
            <p className="text-lg font-serif text-[rgb(var(--foreground))]">
              {Object.entries(stats.byReason).sort(([,a], [,b]) => b - a)[0]?.[0] || "—"}
            </p>
          </div>
        </div>

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
              <div className="flex items-center justify-between mb-2">
                <label className="block text-sm font-serif text-[rgb(var(--muted-foreground))]">
                  CID or AT-URI
                </label>
                <button
                  type="button"
                  onClick={fillExampleUri}
                  className="text-xs text-[rgb(var(--primary))] hover:underline"
                >
                  show example
                </button>
              </div>
              <input
                type="text"
                value={cid}
                onChange={(e) => setCid(e.target.value)}
                placeholder="bafyrei... or at://did:plc:xyz/collection/rkey"
                className="w-full px-4 py-2 text-sm rounded-lg bg-[rgb(var(--background))] text-[rgb(var(--foreground))] border font-mono"
                style={{ borderColor: "rgb(var(--input))" }}
                disabled={submitting}
              />
              <p className="text-xs text-[rgb(var(--muted-foreground))] mt-1">
                Right-click on an emoji or avatar and "Copy Image Address" to get the CID
              </p>
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

        {/* Search and Filters */}
        <div
          className="p-4 rounded-lg border mb-6"
          style={{
            background: "rgba(var(--card), 0.6)",
            borderColor: "rgba(var(--border), 0.3)",
            backdropFilter: "blur(20px)",
          }}
        >
          <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
            <div className="relative">
              <Search
                className="absolute left-3 top-1/2 -translate-y-1/2 text-[rgb(var(--muted-foreground))]"
                size={16}
              />
              <input
                type="text"
                value={searchQuery}
                onChange={(e) => setSearchQuery(e.target.value)}
                placeholder="Search by CID or details..."
                className="w-full pl-10 pr-4 py-2 text-sm rounded-lg bg-[rgb(var(--background))] text-[rgb(var(--foreground))] border"
                style={{ borderColor: "rgb(var(--input))" }}
              />
            </div>

            <select
              value={filterReason}
              onChange={(e) => setFilterReason(e.target.value)}
              className="px-4 py-2 text-sm rounded-lg bg-[rgb(var(--background))] text-[rgb(var(--foreground))] border"
              style={{ borderColor: "rgb(var(--input))" }}
            >
              <option value="all">All Reasons</option>
              {REASONS.map((r) => (
                <option key={r.value} value={r.value}>
                  {r.label}
                </option>
              ))}
            </select>

            <select
              value={filterContentType}
              onChange={(e) => setFilterContentType(e.target.value)}
              className="px-4 py-2 text-sm rounded-lg bg-[rgb(var(--background))] text-[rgb(var(--foreground))] border"
              style={{ borderColor: "rgb(var(--input))" }}
            >
              <option value="all">All Types</option>
              {CONTENT_TYPES.map((ct) => (
                <option key={ct.value} value={ct.value}>
                  {ct.label}
                </option>
              ))}
            </select>
          </div>
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
            blacklisted content ({filteredBlacklisted.length})
          </h2>

          {loadingList ? (
            <div className="flex items-center justify-center py-12">
              <div className="loading-spinner" />
            </div>
          ) : filteredBlacklisted.length === 0 ? (
            <p className="text-center py-12 text-[rgb(var(--muted-foreground))]">
              {searchQuery || filterReason !== "all" || filterContentType !== "all"
                ? "no results found"
                : "no blacklisted content"}
            </p>
          ) : (
            <div className="space-y-3">
              {filteredBlacklisted.map((item) => (
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
                    <div className="flex items-center gap-2 mb-2">
                      <p className="text-xs font-mono text-[rgb(var(--foreground))] break-all flex-1">
                        {item.cid}
                      </p>
                      <button
                        onClick={() => copyCid(item.cid)}
                        className="p-1 rounded hover:bg-[rgba(var(--primary),0.1)] text-[rgb(var(--muted-foreground))] hover:text-[rgb(var(--primary))] transition-colors"
                        title="Copy CID"
                      >
                        <Copy size={14} />
                      </button>
                    </div>
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
