import { Link } from "@tanstack/react-router";

export function Footer() {
  return (
    <footer className="mt-24 relative">
      <div className="max-w-4xl mx-auto px-8 py-16 relative">
        <div className="flex flex-col items-center gap-6 text-center">
          <div
            className="font-cursive text-3xl px-1 py-2"
            style={{
              background: `linear-gradient(135deg, rgb(var(--primary)) 0%, rgb(var(--accent)) 100%)`,
              WebkitBackgroundClip: "text",
              WebkitTextFillColor: "transparent",
              backgroundClip: "text",
              fontWeight: 500,
              filter: "drop-shadow(0 2px 8px rgba(var(--primary), 0.2))",
            }}
          >
            nyt
          </div>

          <div className="flex items-center gap-6 text-sm text-[rgb(var(--muted-foreground))]">
            <Link
              to="/$handle"
              params={{ handle: "natalie.sh" }}
              className="hover:text-[rgb(var(--primary))] transition-colors"
            >
              by natalie.sh
            </Link>
            <span className="opacity-40">•</span>
            <span className="opacity-70">built on atproto</span>
          </div>

          <div
            className="h-px w-32 mt-4"
            style={{
              background: `linear-gradient(90deg, transparent 0%, rgba(var(--primary), 0.3) 50%, transparent 100%)`,
            }}
          />
        </div>
      </div>
    </footer>
  );
}
