import React, { useState, useEffect, useContext } from "react";
import { Client, simpleFetchHandler } from "@atcute/client";
import {
  createAuthorizationUrl,
  getSession,
  deleteStoredSession,
  OAuthUserAgent,
  finalizeAuthorization,
} from "@atcute/oauth-browser-client";
import { initOAuth } from "./oauth";

interface QtContextType {
  client: Client;
  currentAgent: OAuthUserAgent | null;
  did: string | null;
  isLoggedIn: boolean;
  login: (handle: string) => Promise<void>;
  logout: () => Promise<void>;
  finalizeAuth: (params: URLSearchParams) => Promise<void>;
}

const QtContext = React.createContext<QtContextType | null>(null);

export function QtProvider({ children }: { children: React.ReactNode }) {
  const [currentAgent, setCurrentAgent] = useState<OAuthUserAgent | null>(null);
  const [did, setDid] = useState<string | null>(null);
  const [isLoggedIn, setIsLoggedIn] = useState(false);
  const [client, setClient] = useState<Client | null>(null);

  useEffect(() => {
    initOAuth();
    console.log("oauth initted");
    attemptResumeSession();
    console.log("session resumed?");
  }, []);

  // periodically refresh session to prevent token expiry
  useEffect(() => {
    console.log("setting up session refresh effect");
    if (!currentAgent) return;

    const interval = setInterval(
      async () => {
        try {
          console.log("attempting session refresh");
          await currentAgent.getSession();
        } catch (err) {
          console.error("session refresh failed, logging out:", err);
          await logout();
        }
      },
      60 * 60 * 1000, // every hour
    );

    return () => {
      console.log("cleaning up session refresher");
      clearInterval(interval);
    };
  }, [currentAgent]);

  const attemptResumeSession = async () => {
    const currentDid = localStorage.getItem("currentDid");
    if (currentDid) {
      try {
        const session = await getSession(currentDid as any, {
          allowStale: false,
        });
        if (session) {
          const agent = new OAuthUserAgent(session);
          const rpc = new Client({ handler: agent });

          // verify session is valid, refresh if needed
          await currentAgent?.getSession();

          setCurrentAgent(agent);
          setDid(currentDid);
          setIsLoggedIn(true);
          setClient(rpc);
        }
      } catch (err) {
        console.error("failed to resume session:", err);
        localStorage.removeItem("currentDid");
      }
    }
  };

  const login = async (handle: string) => {
    if (!handle.trim()) {
      throw new Error("handle is required");
    }

    const authUrl = await createAuthorizationUrl({
      target: { type: "account", identifier: handle.trim() as any },
      scope: "atproto transition:generic",
    });

    window.location.assign(authUrl);
  };

  const finalizeAuth = async (params: URLSearchParams) => {
    const session = await finalizeAuthorization(params);
    const userDid = session.session.info.sub;

    const agent = new OAuthUserAgent(session.session);
    const rpc = new Client({ handler: agent });

    localStorage.setItem("currentDid", userDid);

    setCurrentAgent(agent);
    setDid(userDid);
    setIsLoggedIn(true);
    setClient(rpc);
  };

  const logout = async () => {
    if (did) {
      deleteStoredSession(did as any);
      localStorage.removeItem("currentDid");
      setCurrentAgent(null);
      setDid(null);
      setIsLoggedIn(false);
      setClient(null);
    }
  };

  if (!client) {
    // get current URL and use that as service endpoint
    let origin = window.location.origin;
    // create a non-authenticated client for initial render
    const handler = simpleFetchHandler({
      service: origin,
    });
    const unauthClient = new Client({ handler });

    const value = {
      client: unauthClient,
      currentAgent: null,
      did: null,
      isLoggedIn: false,
      login,
      logout,
      finalizeAuth,
    };

    return <QtContext.Provider value={value}>{children}</QtContext.Provider>;
  }

  const value = {
    client,
    currentAgent,
    did,
    isLoggedIn,
    login,
    logout,
    finalizeAuth,
  };

  return <QtContext.Provider value={value}>{children}</QtContext.Provider>;
}

export function useQt() {
  const context = useContext(QtContext);
  if (!context) {
    throw new Error("useQt must be used within a QtProvider");
  }
  return context;
}

export function useXrpc(): Client {
  const { client } = useQt();
  return client;
}
