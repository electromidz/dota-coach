"use client";

import { createContext, useContext, useEffect, useState } from "react";

import { ApiError, getMe } from "./api";
import type { MeResponse } from "./types";

export type Session =
  | { kind: "loading" }
  | { kind: "anonymous" }
  | { kind: "signed-in"; me: MeResponse }
  | { kind: "error"; message: string };

interface SessionValue {
  session: Session;
  setSession: React.Dispatch<React.SetStateAction<Session>>;
}

const SessionContext = createContext<SessionValue | null>(null);

/**
 * Resolves the session once for the whole screen.
 *
 * Context rather than a hook per component: the page body and the tab bar both
 * need to know, and they must agree — two independent fetches could disagree
 * mid-flight and show a signed-in chrome around a signed-out body.
 *
 * `401` is a state, not a failure: it means "show the signed-out view", and is
 * kept distinct from a transport or server error so a logged-out visitor is
 * never told that something broke.
 */
export function SessionProvider({ children }: { children: React.ReactNode }) {
  const [session, setSession] = useState<Session>({ kind: "loading" });

  useEffect(() => {
    let cancelled = false;

    getMe()
      .then((me) => {
        if (!cancelled) setSession({ kind: "signed-in", me });
      })
      .catch((error: unknown) => {
        if (cancelled) return;

        if (error instanceof ApiError && error.isUnauthenticated) {
          setSession({ kind: "anonymous" });
          return;
        }
        setSession({
          kind: "error",
          message:
            error instanceof ApiError
              ? error.message
              : "Could not reach the coaching service.",
        });
      });

    return () => {
      cancelled = true;
    };
  }, []);

  return (
    <SessionContext.Provider value={{ session, setSession }}>
      {children}
    </SessionContext.Provider>
  );
}

export function useSession(): SessionValue {
  const value = useContext(SessionContext);
  if (!value) {
    throw new Error("useSession must be used inside a SessionProvider");
  }
  return value;
}
