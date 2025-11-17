import { useState, useEffect } from "react";
import { ThemeToggle } from "../lib/theme";
import { Link } from "@tanstack/react-router";

interface HeaderProps {
  isScrolled: boolean;
}

export function Header({ isScrolled }: HeaderProps) {
  return (
    <header
      className={`backdrop-blur-xl sticky top-0 z-50 border-b flex items-center transition-all duration-300 ${
        isScrolled ? "shadow-lg h-14" : "h-16"
      }`}
      style={{
        borderColor: isScrolled ? "rgb(var(--card))" : "rgb(var(--background))",
        background: isScrolled ? "rgb(var(--card))" : "transparent",
        boxShadow: isScrolled
          ? "0 4px 6px -1px rgba(0, 0, 0, 0.1), 0 2px 4px -1px rgba(0, 0, 0, 0.06)"
          : "none",
      }}
    >
      <div className="max-w-4xl mx-auto px-6 py-2 relative flex-1">
        <div className="flex items-center justify-center">
          <Link to="/" className="text-base truncate hover:underline">
            <h1
              className={`font-cursive text-[rgb(var(--foreground))] transition-all duration-300 ${
                isScrolled ? "text-2xl" : "text-3xl"
              }`}
            >
              istat
            </h1>
          </Link>
        </div>
        <div className="absolute right-6 top-1/2 -translate-y-1/2">
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
