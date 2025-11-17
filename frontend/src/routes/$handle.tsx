import { createFileRoute } from "@tanstack/react-router";
import { useState, useEffect } from "react";
import { ok } from "@atcute/client";
import { useQt } from "../lib/qt-provider";
import { Header, useScrollDetection } from "../components/Header";
import type { UserStatusView } from "../lexicons/types/vg/nat/istat/status/listUserStatuses";

export const Route = createFileRoute("/$handle")({
  component: UserProfile,
});

interface ProfileData {
  displayName?: string;
  handle: string;
  avatar?: string;
  description?: string;
}

function UserProfile() {
  const { handle } = Route.useParams();
  const { client } = useQt();
  const [profile, setProfile] = useState<ProfileData | null>(null);
  const [statuses, setStatuses] = useState<UserStatusView[]>([]);
  const [loadingProfile, setLoadingProfile] = useState(true);
  const [loadingStatuses, setLoadingStatuses] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const isScrolled = useScrollDetection(20);

  useEffect(() => {
    fetchProfile();
    fetchStatuses();
  }, [handle]);

  const fetchProfile = async () => {
    setLoadingProfile(true);
    setError(null);
    try {
      console.log("getting actor", handle);
      const data = await ok(
        client.get("vg.nat.istat.actor.getProfile", {
          params: { actor: handle as any },
        }),
      );

      setProfile({
        displayName: data.displayName,
        handle: data.handle,
        avatar: data.avatar,
        description: data.description,
      });
    } catch (err) {
      console.error("failed to fetch profile:", err);
      setError(err instanceof Error ? err.message : "failed to load profile");
    } finally {
      setLoadingProfile(false);
    }
  };

  const fetchStatuses = async () => {
    setLoadingStatuses(true);
    try {
      const data = await ok(
        client.get("vg.nat.istat.status.listUserStatuses", {
          params: { handle: handle as any, limit: 50 },
        }),
      );
      setStatuses(data.statuses);
    } catch (err) {
      console.error("failed to fetch statuses:", err);
    } finally {
      setLoadingStatuses(false);
    }
  };

  if (error) {
    return (
      <div className="min-h-screen bg-[rgb(var(--background))]">
        <Header isScrolled={isScrolled} />
        <main className="max-w-4xl mx-auto px-6 py-16">
          <div className="text-center">
            <h1 className="text-2xl font-bold text-[rgb(var(--foreground))] mb-4">
              profile not found
            </h1>
            <p className="text-[rgb(var(--muted-foreground))]">{error}</p>
          </div>
        </main>
      </div>
    );
  }

  return (
    <div className="min-h-screen bg-[rgb(var(--background))]">
      <Header isScrolled={isScrolled} />

      {loadingProfile ? (
        <div className="max-w-4xl mx-auto px-6 py-8">
          <div className="animate-pulse">
            <div className="h-24 w-24 bg-[rgb(var(--muted))] rounded-full mb-4" />
            <div className="h-8 w-48 bg-[rgb(var(--muted))] rounded mb-2" />
            <div className="h-4 w-32 bg-[rgb(var(--muted))] rounded" />
          </div>
        </div>
      ) : profile ? (
        <div className="max-w-4xl mx-auto px-6 py-8">
          <div className="flex items-start gap-6 mb-8">
            {profile.avatar && (
              <img
                src={profile.avatar}
                alt={profile.displayName || profile.handle}
                className="w-24 h-24 rounded-full object-cover"
              />
            )}
            <div className="flex-1">
              <h1 className="text-3xl font-bold text-[rgb(var(--foreground))] mb-1">
                {profile.displayName || profile.handle}
              </h1>
              <p className="text-[rgb(var(--muted-foreground))] mb-3">
                @{profile.handle}
              </p>
              {profile.description && (
                <p className="text-[rgb(var(--foreground))] whitespace-pre-wrap">
                  {profile.description}
                </p>
              )}
            </div>
          </div>

          <div className="border-t border-[rgb(var(--border))] pt-8">
            <h2 className="text-xl font-semibold text-[rgb(var(--foreground))] mb-6">
              status history
            </h2>

            {loadingStatuses ? (
              <div className="space-y-4">
                {[1, 2, 3].map((i) => (
                  <div
                    key={i}
                    className="animate-pulse bg-[rgb(var(--card))] rounded-lg p-6"
                  >
                    <div className="h-16 w-16 bg-[rgb(var(--muted))] rounded mb-3" />
                    <div className="h-4 w-3/4 bg-[rgb(var(--muted))] rounded mb-2" />
                    <div className="h-3 w-1/2 bg-[rgb(var(--muted))] rounded" />
                  </div>
                ))}
              </div>
            ) : statuses.length === 0 ? (
              <p className="text-[rgb(var(--muted-foreground))] text-center py-8">
                no statuses yet
              </p>
            ) : (
              <div className="space-y-4">
                {statuses.map((status) => (
                  <div
                    key={status.rkey}
                    className="bg-[rgb(var(--card))] rounded-lg p-6 border border-[rgb(var(--border))]"
                  >
                    <div className="flex items-start gap-4">
                      <img
                        src={status.emojiUrl}
                        alt="status emoji"
                        className="w-16 h-16 object-contain"
                      />
                      <div className="flex-1">
                        {status.title && (
                          <h3 className="font-semibold text-[rgb(var(--foreground))] mb-1">
                            {status.title}
                          </h3>
                        )}
                        {status.description && (
                          <p className="text-[rgb(var(--muted-foreground))] mb-2">
                            {status.description}
                          </p>
                        )}
                        <p className="text-sm text-[rgb(var(--muted-foreground))]">
                          {new Date(status.createdAt).toLocaleString()}
                        </p>
                      </div>
                    </div>
                  </div>
                ))}
              </div>
            )}
          </div>
        </div>
      ) : null}
    </div>
  );
}
