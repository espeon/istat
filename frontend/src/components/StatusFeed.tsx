import { StatusCard } from "./StatusCard";

interface StatusFeedProps {
  statuses: Array<{
    did: string;
    handle: string;
    displayName?: string;
    avatarUrl?: string;
    rkey: string;
    emojiUrl: string;
    title?: string;
    description?: string;
    expires?: string;
    createdAt: string;
  }>;
  loading: boolean;
  error: string | null;
  onRetry: () => void;
}

export function StatusFeed({ statuses, loading, error, onRetry }: StatusFeedProps) {
  if (loading) {
    return (
      <div className="flex justify-center py-20">
        <div className="loading-spinner" />
      </div>
    );
  }

  if (error) {
    return (
      <div className="text-center py-20">
        <h2 className="text-xl mb-3 text-[rgb(var(--foreground))]" style={{ fontFamily: "EB Garamond" }}>
          unable to load
        </h2>
        <p className="mb-4 text-sm text-[rgb(var(--muted-foreground))]">{error}</p>
        <button
          onClick={onRetry}
          className="px-6 py-2 text-sm transition-all duration-200 border-[rgb(var(--primary))] bg-transparent text-[rgb(var(--foreground))]"
          style={{ border: '1px solid rgb(var(--primary))' }}
        >
          try again
        </button>
      </div>
    );
  }

  if (statuses.length === 0) {
    return (
      <div className="text-center py-20">
        <p
          className="italic text-[rgb(var(--muted-foreground))]"
          style={{ fontFamily: "EB Garamond" }}
        >
          no statuses yet
        </p>
      </div>
    );
  }

  return (
    <div className="space-y-6">
      {statuses.map((status, index) => (
        <StatusCard
          key={`${status.did}-${status.rkey}`}
          status={status}
          index={index}
        />
      ))}
    </div>
  );
}
