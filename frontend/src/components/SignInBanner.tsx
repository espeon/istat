interface SignInBannerProps {
  handle: string;
  onHandleChange: (handle: string) => void;
  onSignIn: () => void;
  isLoading: boolean;
}

export function SignInBanner({ handle, onHandleChange, onSignIn, isLoading }: SignInBannerProps) {
  return (
    <div
      className="border-b"
      style={{
        borderColor: 'rgb(var(--border))',
        background: 'rgb(var(--card))'
      }}
    >
      <div className="max-w-4xl mx-auto px-6 py-4">
        <div className="flex items-center justify-between gap-4">
          <p className="text-sm text-[rgb(var(--muted-foreground))]">
            sign in to post your own status
          </p>
          <div className="flex items-center gap-2">
            <input
              type="text"
              value={handle}
              onChange={(e) => onHandleChange(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && onSignIn()}
              placeholder="handle.bsky.social"
              className="px-3 py-1.5 text-xs w-48 bg-[rgb(var(--background))] text-[rgb(var(--foreground))] border"
              style={{ borderColor: 'rgb(var(--input))' }}
              disabled={isLoading}
            />
            <button
              onClick={onSignIn}
              disabled={isLoading}
              className="px-3 py-1.5 text-xs transition-all duration-200 disabled:opacity-50 bg-[rgb(var(--primary))] text-[rgb(var(--primary-foreground))] hover:opacity-90"
            >
              {isLoading ? "..." : "sign in"}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
