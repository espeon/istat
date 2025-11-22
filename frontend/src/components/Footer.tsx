import { Link } from "@tanstack/react-router";

export function Footer() {
  return (
    <footer className="border-t mt-16 relative overflow-hidden" style={{ borderColor: "rgba(var(--border), 0.5)" }}>
      {/* Subtle gradient overlay */}
      <div
        className="absolute inset-0 opacity-30 pointer-events-none"
        style={{
          background: "linear-gradient(to top, rgba(var(--accent), 0.05), transparent)",
        }}
      />
      <div className="max-w-4xl mx-auto px-6 py-8 relative">
        <div className="flex items-center justify-between text-sm text-[rgb(var(--muted-foreground))]">
          <div className="flex items-center gap-4">
            <span className="font-cursive text-lg text-[rgb(var(--accent))]">nyt.one</span>
            <span className="text-[rgb(var(--muted-foreground))]">•</span>
            <span className="text-xs">a new way to share status</span>
          </div>
          <div className="flex items-center gap-4">
            <div>
              by{" "}
              <Link
                to="/$handle"
                params={{ handle: "natalie.sh" }}
                className="hover:text-[rgb(var(--accent))] transition-colors"
              >
                natalie.sh
              </Link>
            </div>
            <span className="text-[rgb(var(--muted-foreground))]">•</span>
            <span>built on atproto</span>
          </div>
        </div>
      </div>
    </footer>
  );
}
