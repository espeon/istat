import { createFileRoute } from "@tanstack/react-router";
import { useState, useEffect } from "react";
import { ok } from "@atcute/client";
import { SquarePen } from "lucide-react";
import { useQt } from "../lib/qt-provider";
import { Header, useScrollDetection } from "../components/Header";
import { UserBanner } from "../components/UserBanner";
import { SignInBanner } from "../components/SignInBanner";
import { StatusFeed } from "../components/StatusFeed";
import { SubmitModal } from "../components/SubmitModal";
import { Footer } from "../components/Footer";
import type { StatusView } from "../lexicons/types/vg/nat/istat/status/listStatuses";

export const Route = createFileRoute("/")({
  component: App,
});

interface ProfileData {
  displayName?: string;
  handle: string;
  avatar?: string;
}

function App() {
  const { isLoggedIn, did, login, logout, client, currentAgent } = useQt();
  const [handle, setHandle] = useState("");
  const [isLoggingIn, setIsLoggingIn] = useState(false);
  const [profile, setProfile] = useState<ProfileData | null>(null);
  const [loadingProfile, setLoadingProfile] = useState(false);
  const [statuses, setStatuses] = useState<StatusView[]>([]);
  const [loadingStatuses, setLoadingStatuses] = useState(true);
  const [statusesError, setStatusesError] = useState<string | null>(null);
  const [isModalOpen, setIsModalOpen] = useState(false);
  const isScrolled = useScrollDetection(20);

  useEffect(() => {
    fetchStatuses();
  }, []);

  useEffect(() => {
    if (isLoggedIn && did && currentAgent) {
      fetchProfile();
    }
  }, [isLoggedIn, did]);

  const fetchStatuses = async () => {
    setLoadingStatuses(true);
    setStatusesError(null);
    try {
      const data = await ok(
        client.get("vg.nat.istat.status.listStatuses", {
          params: { limit: 50 },
        }),
      );
      setStatuses(data.statuses);
    } catch (err) {
      console.error("failed to fetch statuses:", err);
      setStatusesError(err instanceof Error ? err.message : "unknown error");
    } finally {
      setLoadingStatuses(false);
    }
  };

  const fetchProfile = async () => {
    if (!did) return;

    setLoadingProfile(true);
    try {
      const data = await ok(
        client.get("vg.nat.istat.actor.getProfile", {
          params: { actor: did as any },
        }),
      );

      setProfile({
        displayName: data.displayName,
        handle: data.handle,
        avatar: data.avatar,
      });
    } catch (err) {
      console.error("failed to fetch profile:", err);
    } finally {
      setLoadingProfile(false);
    }
  };

  const handleLogin = async () => {
    if (!handle.trim()) {
      alert("please enter your bluesky handle");
      return;
    }

    setIsLoggingIn(true);
    try {
      await login(handle.trim());
    } catch (err) {
      console.error("login error:", err);
      alert(
        "failed to start login: " +
          (err instanceof Error ? err.message : "unknown error"),
      );
      setIsLoggingIn(false);
    }
  };

  const handleLogout = async () => {
    await logout();
    setProfile(null);
  };

  return (
    <div className="min-h-screen relative">
      <Header isScrolled={isScrolled} />

      {isLoggedIn ? (
        <UserBanner
          profile={profile}
          did={did}
          loading={loadingProfile}
          onLogout={handleLogout}
          onOpenStatusModal={() => setIsModalOpen(true)}
        />
      ) : (
        <SignInBanner
          handle={handle}
          onHandleChange={setHandle}
          onSignIn={handleLogin}
          isLoading={isLoggingIn}
        />
      )}

      <main className="max-w-4xl mx-auto px-6 py-12 relative z-10">
        <StatusFeed
          statuses={statuses}
          loading={loadingStatuses}
          error={statusesError}
          onRetry={fetchStatuses}
        />
      </main>

      {isLoggedIn && (
        <>
          <button
            onClick={() => setIsModalOpen(true)}
            className="fab-glow fixed bottom-8 right-8 w-16 h-16 bg-[rgb(var(--primary))] text-[rgb(var(--primary-foreground))] rounded-full flex items-center justify-center z-50"
            aria-label="new status"
          >
            <SquarePen className="w-7 h-7" />
          </button>
          <SubmitModal
            isOpen={isModalOpen}
            onClose={() => {
              setIsModalOpen(false);
              fetchStatuses(); // refresh feed after posting
            }}
          />
        </>
      )}

      <Footer />
    </div>
  );
}
