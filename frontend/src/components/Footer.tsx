import { Link } from "@tanstack/react-router";

export function Footer() {
  return (
    <footer className="border-t-4 mt-16 relative" style={{ borderColor: "rgb(var(--border))" }}>
      {/* Bold accent stripe */}
      <div
        className="absolute top-0 left-0 right-0 h-2"
        style={{
          background: `linear-gradient(90deg, rgb(var(--primary)) 0%, rgb(var(--accent)) 100%)`,
        }}
      />
      <div className="max-w-4xl mx-auto px-6 py-12 relative">
        <div className="flex items-start justify-between">
          <div className="flex flex-col gap-3">
            <span className="font-cursive text-4xl text-[rgb(var(--foreground))] tracking-tight" style={{ fontWeight: 600 }}>
              nyt
            </span>
            <span className="text-sm text-[rgb(var(--muted-foreground))] uppercase tracking-wider">
              Status Broadcasting
            </span>
          </div>
          <div className="flex flex-col gap-2 text-right text-sm text-[rgb(var(--muted-foreground))]">
            <div>
              by{" "}
              <Link
                to="/$handle"
                params={{ handle: "natalie.sh" }}
                className="text-[rgb(var(--foreground))] hover:text-[rgb(var(--primary))] transition-colors font-medium"
              >
                natalie.sh
              </Link>
            </div>
            <span className="text-xs uppercase tracking-wider">Built on ATProto</span>
          </div>
        </div>
      </div>
    </footer>
  );
}
