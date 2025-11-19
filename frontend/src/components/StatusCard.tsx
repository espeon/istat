import { useEffect, useState } from "react";
import { Link } from "@tanstack/react-router";

interface StatusCardProps {
  status: {
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
    emojiName?: string;
    emojiAlt?: string;
  };
  index: number;
}

function checkExpired(expires?: string): boolean {
  if (!expires) return false;
  const now = new Date();
  const expiryDate = new Date(expires);
  return now < expiryDate;
}

export function StatusCard({ status, index }: StatusCardProps) {
  const [imageLoaded, setImageLoaded] = useState(false);
  const [imageError, setImageError] = useState(false);

  const [isExpired, setIsExpired] = useState(checkExpired(status.expires));

  // check expires every ~second
  useEffect(() => {
    const interval = setInterval(() => {
      setIsExpired(checkExpired(status.expires));
    }, 1000);
    return () => clearInterval(interval);
  }, [status.expires]);

  if (isExpired) {
    return (
      <article
        key="status-card"
        className="status-card p-4 bg-[rgb(var(--card))] border"
        style={{
          animationDelay: `${index * 50}ms`,
          borderColor: "rgb(var(--border))",
        }}
      >
        <div className="flex gap-4">
          {/* emoji */}
          <div className="shrink-0">
            <div className="text-2xl text-[rgb(var(--muted))] w-20 h-20 flex justify-center items-center">
              <div>∅</div>
            </div>
          </div>
          <div className="flex-1 flex flex-col justify-center items-start min-w-0">
            <div className="italic font-serif text-[rgb(var(--muted-foreground))]">
              This status has expired
            </div>
          </div>
        </div>
      </article>
    );
  }

  return (
    <article
      key="status-card"
      className="status-card p-4 bg-[rgb(var(--card))] border"
      style={{
        animationDelay: `${index * 50}ms`,
        borderColor: "rgb(var(--border))",
      }}
    >
      <div className="flex gap-4">
        {/* emoji */}
        <div className="shrink-0 max-h-20">
          {!imageError ? (
            <img
              src={status.emojiUrl}
              alt={status.emojiAlt || status.emojiName || "status emoji"}
              className={`w-20 h-18 object-contain transition-opacity duration-300 ${
                imageLoaded ? "opacity-100" : "opacity-0"
              }`}
              onLoad={() => setImageLoaded(true)}
              onError={() => setImageError(true)}
            />
          ) : (
            <div className="text-2xl text-[rgb(var(--muted))] w-20 h-18 flex justify-center items-center">
              <div>∅</div>
            </div>
          )}
          {/* if there's a title we display it right under */}
          <div className="mt-1 text-xs max-w-20 line-clamp-2 text-center text-[rgb(var(--muted-foreground))] truncate">
            {status.emojiName}
          </div>
        </div>

        {/* content */}
        <div className="flex-1 min-w-0 flex flex-col justify-center gap-1">
          {/* user info */}
          <div className="flex items-center gap-2">
            {status.avatarUrl && (
              <img
                src={status.avatarUrl}
                alt=""
                className="w-8 h-8 rounded-full border"
                style={{ borderColor: "rgb(var(--border))" }}
              />
            )}
            <div className="flex-1 min-w-0">
              <div className="flex items-baseline gap-2">
                <Link
                  to="/$handle"
                  params={{ handle: status.handle }}
                  className="text-base truncate hover:underline"
                >
                  @{status.handle}
                </Link>
              </div>
            </div>
            <span className="text-base text-[rgb(var(--muted-foreground))]">
              {formatTimestamp(status.createdAt)}
            </span>
          </div>

          {/* status content */}
          <div>
            {status.title && (
              <h2
                className="text-lg mb-1 leading-snug text-[rgb(var(--card-foreground))]"
                style={{ fontFamily: "EB Garamond", fontWeight: 600 }}
              >
                {status.title}
              </h2>
            )}

            {status.description && (
              <p className="text-sm leading-relaxed text-[rgb(var(--card-foreground))]">
                {status.description}
              </p>
            )}

            {status.expires && (
              <div
                className="mt-2 inline-flex items-center gap-1.5 text-[0.7rem] text-[rgb(var(--accent))]"
                style={{ fontFamily: "EB Garamond", fontStyle: "italic" }}
              >
                <span>expires {formatTimestamp(status.expires)}</span>
              </div>
            )}
          </div>
        </div>
      </div>
    </article>
  );
}

function formatTimestamp(isoString: string): string {
  const date = new Date(isoString);
  const now = new Date();
  const diffMs = now.getTime() - date.getTime();
  const diffMins = Math.floor(diffMs / 60000);
  const diffHours = Math.floor(diffMs / 3600000);
  const diffDays = Math.floor(diffMs / 86400000);

  if (diffMins < 1) return "now";
  if (diffMins < 60) return `${diffMins}m ago`;
  if (diffHours < 24) return `${diffHours}h ago`;
  if (diffDays < 7) return `${diffDays}d ago`;

  return date.toLocaleDateString("en-US", {
    month: "short",
    day: "numeric",
  });
}
