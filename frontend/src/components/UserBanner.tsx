import { LogOut } from "lucide-react";

interface UserBannerProps {
  profile: {
    displayName?: string;
    handle: string;
    avatar?: string;
  } | null;
  did: string | null;
  loading: boolean;
  onLogout: () => void;
  onOpenStatusModal?: () => void;
}

export function UserBanner({
  profile,
  did,
  loading,
  onLogout,
  onOpenStatusModal,
}: UserBannerProps) {
  console.log(profile);
  return (
    <div
      className="border-b relative"
      style={{
        borderColor: "rgba(var(--border), 0.2)",
        background: "rgba(var(--card), 0.4)",
        backdropFilter: "blur(20px)",
      }}
    >
      <div className="max-w-4xl mx-auto px-8 py-4">
        {loading ? (
          <div className="flex items-center gap-4">
            <div className="w-10 h-10 rounded-full bg-[rgba(var(--primary),0.1)] animate-pulse" />
            <div className="flex-1">
              <div className="h-4 w-32 bg-[rgba(var(--primary),0.1)] rounded-full animate-pulse" />
            </div>
          </div>
        ) : profile ? (
          <div className="flex items-center justify-between gap-4">
            <div className="flex items-center gap-4 flex-1 min-w-0">
              {profile.avatar && (
                <div className="relative">
                  <img
                    src={profile.avatar}
                    alt=""
                    className="w-10 h-10 rounded-full"
                    style={{
                      border: "2px solid rgba(var(--primary), 0.3)",
                      boxShadow: "0 4px 12px rgba(var(--primary), 0.2)",
                    }}
                  />
                </div>
              )}
              <div className="flex-1 min-w-0">
                <p className="text-sm text-[rgb(var(--muted-foreground))]">
                  welcome back,{" "}
                  <span
                    className="font-serif text-[rgb(var(--foreground))]"
                    style={{ fontWeight: 500 }}
                  >
                    {profile.displayName || profile.handle}
                  </span>
                </p>
              </div>
            </div>
            <div className="flex items-center gap-2">
              <button
                onClick={onOpenStatusModal}
                className="hidden md:block px-4 py-2 text-xs transition-all duration-300 bg-[rgb(var(--primary))] text-[rgb(var(--primary-foreground))] rounded-full hover:shadow-lg hover:-translate-y-0.5"
                style={{
                  boxShadow: "0 4px 12px rgba(var(--primary), 0.3)",
                }}
              >
                post status
              </button>
              <button
                onClick={onLogout}
                className="p-2 text-xs transition-all duration-300 text-[rgb(var(--muted-foreground))] hover:text-[rgb(var(--foreground))] rounded-full hover:bg-[rgba(var(--muted),0.5)]"
              >
                <LogOut size={16} />
              </button>
            </div>
          </div>
        ) : (
          <div className="flex items-center justify-between gap-4">
            <div className="flex items-center gap-3 flex-1 min-w-0">
              <p className="text-sm text-[rgb(var(--muted-foreground))]">
                hi {did}
              </p>
            </div>
            <div className="flex items-center gap-2">
              <button
                onClick={onOpenStatusModal}
                className="hidden md:block px-4 py-2 text-xs transition-all duration-300 bg-[rgb(var(--primary))] text-[rgb(var(--primary-foreground))] rounded-full hover:shadow-lg hover:-translate-y-0.5"
                style={{
                  boxShadow: "0 4px 12px rgba(var(--primary), 0.3)",
                }}
              >
                post status
              </button>
              <button
                onClick={onLogout}
                className="p-2 text-xs transition-all duration-300 text-[rgb(var(--muted-foreground))] hover:text-[rgb(var(--foreground))] rounded-full hover:bg-[rgba(var(--muted),0.5)]"
              >
                <LogOut size={16} />
              </button>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
