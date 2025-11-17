import { createFileRoute, useNavigate } from "@tanstack/react-router";
import { useEffect, useState } from "react";
import { useQt } from "../../lib/qt-provider";

export const Route = createFileRoute("/oauth/callback")({
  component: OAuthCallback,
});

function OAuthCallback() {
  const navigate = useNavigate();
  const { finalizeAuth } = useQt();
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const handleCallback = async () => {
      try {
        // Give IndexedDB a moment to be ready
        await new Promise((resolve) => setTimeout(resolve, 100));

        // Extract OAuth parameters from URL hash
        const params = new URLSearchParams(location.hash.slice(1));

        console.log("callback params:", Object.fromEntries(params));

        // Finalize authorization through the provider
        await finalizeAuth(params);

        console.log("session created successfully");

        // Redirect to home
        navigate({ to: "/" });
      } catch (err) {
        console.error("oauth callback error:", err);
        setError(
          err instanceof Error ? err.message : "failed to complete login",
        );
      }
    };

    handleCallback();
  }, [navigate, finalizeAuth]);

  if (error) {
    return (
      <div className="min-h-screen flex items-center justify-center bg-[#282c34] text-white">
        <div className="text-center">
          <h1 className="text-2xl font-bold mb-4">Login Failed</h1>
          <p className="text-red-400 mb-4">{error}</p>
          <button
            onClick={() => navigate({ to: "/" })}
            className="px-4 py-2 bg-[#61dafb] text-[#282c34] rounded hover:bg-opacity-80"
          >
            Go Home
          </button>
        </div>
      </div>
    );
  }

  return (
    <div className="min-h-screen flex items-center justify-center bg-[#282c34] text-white">
      <div className="text-center">
        <div className="animate-spin rounded-full h-12 w-12 border-b-2 border-[#61dafb] mx-auto mb-4"></div>
        <p>Completing login...</p>
      </div>
    </div>
  );
}
