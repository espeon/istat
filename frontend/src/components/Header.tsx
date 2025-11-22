import { useState, useEffect } from "react";
import { ThemeToggle } from "../lib/theme";
import { Link } from "@tanstack/react-router";

interface HeaderProps {
  isScrolled: boolean;
}

export function Header({ isScrolled }: HeaderProps) {
  return (
    <header
      className={`sticky top-0 z-50 border-b-4 flex items-center transition-all duration-300 ${
        isScrolled ? "h-16" : "h-20"
      }`}
      style={{
        borderColor: "rgb(var(--border))",
        background: "rgb(var(--background))",
        boxShadow: isScrolled
          ? "0 4px 0 rgb(var(--primary))"
          : "none",
      }}
    >
      <div className="max-w-4xl mx-auto px-6 py-2 relative flex-1">
        {/* Bold accent block */}
        <div
          className="absolute left-0 top-0 bottom-0 w-2 transition-all duration-300"
          style={{
            background: `linear-gradient(180deg, rgb(var(--primary)) 0%, rgb(var(--accent)) 100%)`,
            width: isScrolled ? "4px" : "2px",
          }}
        />

        <div className="flex items-center justify-between">
          <Link to="/" className="group flex items-center gap-3">
            <h1
              className={`font-cursive text-[rgb(var(--foreground))] transition-all duration-300 tracking-tight ${
                isScrolled ? "text-3xl" : "text-5xl"
              }`}
              style={{
                fontWeight: 600,
                letterSpacing: "-0.02em",
              }}
            >
              <span className="relative inline-block">
                nyt
                {/* Underline accent */}
                <span
                  className="absolute bottom-0 left-0 h-1 bg-[rgb(var(--primary))] transition-all duration-300 group-hover:h-2"
                  style={{
                    width: "100%",
                  }}
                />
              </span>
            </h1>
            <span className="text-xs uppercase tracking-wider text-[rgb(var(--muted-foreground))] font-sans mt-2">
              Status
            </span>
          </Link>

          <div className="flex items-center gap-4">
            <ThemeToggle />
          </div>
        </div>
      </div>
    </header>
  );
}

export function useScrollDetection(threshold = 20) {
  const [isScrolled, setIsScrolled] = useState(() => {
    // Avoid accessing `window` during SSR; initialize from current scroll position on the client.
    if (typeof window === "undefined") return false;
    return window.scrollY > threshold;
  });

  useEffect(() => {
    if (typeof window === "undefined") return;

    const handleScroll = () => {
      setIsScrolled(window.scrollY > threshold);
    };

    // Set initial state based on the current scroll position when the hook mounts
    handleScroll();

    window.addEventListener("scroll", handleScroll, { passive: true });
    return () => window.removeEventListener("scroll", handleScroll);
  }, [threshold]);

  return isScrolled;
}
