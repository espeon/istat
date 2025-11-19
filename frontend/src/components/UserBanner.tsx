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
      className="border-b"
      style={{
        borderColor: "rgb(var(--border))",
        background: "rgb(var(--muted))",
      }}
    >
      <div className="max-w-4xl mx-auto px-6 py-4">
        {loading ? (
          <div className="flex items-center gap-3">
            <div className="w-8 h-8 rounded-full bg-[rgb(var(--border))] animate-pulse" />
            <div className="flex-1">
              <div className="h-4 w-32 bg-[rgb(var(--border))] rounded animate-pulse" />
            </div>
          </div>
        ) : profile ? (
          <div className="flex items-center justify-between gap-4">
            <div className="flex items-center gap-3 flex-1 min-w-0">
              {profile.avatar && (
                <img
                  src={profile.avatar}
                  alt=""
                  className="w-8 h-8 rounded-full border"
                  style={{ borderColor: "rgb(var(--border))" }}
                />
              )}
              <div className="flex-1 min-w-0">
                <p className="text-base text-[rgb(var(--muted-foreground))]">
                  hey{" "}
                  <span className="font-semibold font-serif text-[rgb(var(--foreground))]">
                    {profile.displayName || profile.handle}
                  </span>
                </p>
              </div>
            </div>
            <div className="flex items-center gap-2">
              <button
                onClick={onOpenStatusModal}
                className="hidden md:block px-3 py-1.5 text-xs transition-all duration-200 bg-[rgb(var(--primary))] text-[rgb(var(--primary-foreground))] hover:opacity-90"
              >
                post status
              </button>
              <button
                onClick={onLogout}
                className="px-3 py-1.5 text-xs transition-all duration-200 text-[rgb(var(--muted-foreground))] hover:text-[rgb(var(--foreground))]"
              >
                <LogOut size={16} />
              </button>
            </div>
          </div>
        ) : (
          <div className="flex items-center justify-between gap-4">
            <div className="flex items-center gap-3 flex-1 min-w-0">
              hi {did}
            </div>
            <div className="flex items-center gap-2">
              <button
                onClick={onOpenStatusModal}
                className="hidden md:block px-3 py-1.5 text-xs transition-all duration-200 bg-[rgb(var(--primary))] text-[rgb(var(--primary-foreground))] hover:opacity-90"
              >
                post status
              </button>
              <button
                onClick={onLogout}
                className="px-3 py-1.5 text-xs transition-all duration-200 text-[rgb(var(--muted-foreground))] hover:text-[rgb(var(--foreground))]"
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
