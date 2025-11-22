import { useState, useEffect } from "react";
import { ok } from "@atcute/client";
import { useQt } from "./qt-provider";

/**
 * Hook to check if the current user is an admin
 */
export function useIsAdmin() {
  const { isLoggedIn, client } = useQt();
  const [isAdmin, setIsAdmin] = useState(false);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    const checkAdmin = async () => {
      if (!isLoggedIn) {
        setIsAdmin(false);
        setLoading(false);
        return;
      }

      setLoading(true);
      try {
        const data = await ok(
          client.get("vg.nat.istat.moderation.isAdmin", {}),
        );
        setIsAdmin(data.isAdmin);
      } catch (err) {
        console.error("Failed to check admin status:", err);
        setIsAdmin(false);
      } finally {
        setLoading(false);
      }
    };

    checkAdmin();
  }, [isLoggedIn, client]);

  return { isAdmin, loading };
}
