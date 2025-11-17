import { Link } from "@tanstack/react-router";

export function Footer() {
  return (
    <footer className="border-t border-[rgb(var(--border))] mt-16">
      <div className="max-w-4xl mx-auto px-6 py-8">
        <div className="flex items-center justify-between text-sm text-[rgb(var(--muted-foreground))]">
          <div>
            <a
              href="https://github.com/yourusername/istat"
              target="_blank"
              rel="noopener noreferrer"
              className="hover:text-[rgb(var(--foreground))] transition-colors"
            >
              source code
            </a>
          </div>
          <div className="flex items-center gap-4">
            <div>
              by{" "}
              <Link
                to="/$handle"
                params={{ handle: "natalie.sh" }}
                className="hover:text-[rgb(var(--foreground))] transition-colors"
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
