import { useState, useEffect } from "react";
import { ThemeToggle } from "../lib/theme";
import { Link } from "@tanstack/react-router";

interface HeaderProps {
  isScrolled: boolean;
}

export function Header({ isScrolled }: HeaderProps) {
  return (
    <header
      className={`sticky top-0 z-50 flex items-center transition-all duration-500 ${
        isScrolled ? "h-14" : "h-20"
      }`}
      style={{
        background: isScrolled
          ? "rgba(var(--background), 0.8)"
          : "transparent",
        backdropFilter: isScrolled ? "blur(20px)" : "none",
        borderBottom: isScrolled
          ? "1px solid rgba(var(--border), 0.2)"
          : "none",
      }}
    >
      <div className="max-w-4xl mx-auto px-8 py-4 relative flex-1">
        <div className="flex items-center justify-between">
          <Link to="/" className="group">
            <h1
              className={`font-cursive transition-all duration-500 ${
                isScrolled ? "text-2xl" : "text-4xl"
              }`}
              style={{
                background: `linear-gradient(135deg, rgb(var(--primary)) 0%, rgb(var(--accent)) 100%)`,
                WebkitBackgroundClip: "text",
                WebkitTextFillColor: "transparent",
                backgroundClip: "text",
                fontWeight: 500,
                filter: "drop-shadow(0 2px 8px rgba(var(--primary), 0.3))",
              }}
            >
              nyt
            </h1>
          </Link>

          <ThemeToggle />
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
