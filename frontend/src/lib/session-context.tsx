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
 * A cookie on *this* origin that says only "this browser was signed in
 * recently". The real session cookie is `HttpOnly` and scoped to the backend
 * host, so the Next server cannot see it and has no way to know, while
 * rendering, whether a visitor is a player or a crawler.
 *
 * That guess has to be made somewhere: `/` must serve the full marketing page
 * to anyone unauthenticated — otherwise the pitch is invisible to search —
 * while not flashing that page at a signed-in player on every navigation.
 * This hint is what resolves it.
 *
 * It is a rendering hint and nothing else. It carries no identity, grants no
 * access, and every route behind it still resolves the real session with
 * `/api/auth/me`; a forged or stale hint costs a visitor one wrong skeleton
 * and nothing more. Deliberately not `HttpOnly`: the client is what sets and
 * clears it.
 */
const HINT_COOKIE = "dc_signed_in";
const HINT_MAX_AGE_DAYS = 30;

function writeHint(signedIn: boolean) {
  if (typeof document === "undefined") return;

  const secure = window.location.protocol === "https:" ? "; Secure" : "";
  const maxAge = signedIn ? HINT_MAX_AGE_DAYS * 24 * 60 * 60 : 0;

  document.cookie = `${HINT_COOKIE}=${signedIn ? "1" : ""}; Path=/; Max-Age=${maxAge}; SameSite=Lax${secure}`;
}

/**
 * Drops the rendering hint on sign-out, so the redirect to `/` lands on the
 * marketing page rather than an app skeleton that immediately 401s.
 */
export function clearSignedInHint() {
  writeHint(false);
}

/**
 * Records that this browser has a session.
 *
 * Needed outside `SessionProvider` because of an ordering problem the provider
 * cannot solve on its own: `/` only mounts the provider once the hint already
 * exists, and the hint only gets written by the provider. Something has to
 * write the first one — see `SessionHandoff`.
 */
export function markSignedInHint() {
  writeHint(true);
}

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
        if (cancelled) return;
        writeHint(true);
        setSession({ kind: "signed-in", me });
      })
      .catch((error: unknown) => {
        if (cancelled) return;

        if (error instanceof ApiError && error.isUnauthenticated) {
          // A stale hint would keep serving the app skeleton to someone whose
          // session has expired, so a confirmed 401 clears it.
          writeHint(false);
          setSession({ kind: "anonymous" });
          return;
        }
        // A transport or 5xx failure says nothing about the session; leaving
        // the hint alone means an outage cannot log the visitor's next page
        // load out of its own layout.
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
