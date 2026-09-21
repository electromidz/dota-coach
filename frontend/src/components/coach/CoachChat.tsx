"use client";

import { useEffect, useRef, useState } from "react";

import { Alert } from "@/components/ui/Alert";
import { Button } from "@/components/ui/Button";
import { Card } from "@/components/ui/Card";
import { ApiError, askCoach, getConversation } from "@/lib/api";
import type { ConversationMessage, ConversationResponse } from "@/lib/types";
import { cn } from "@/lib/utils";

/** Matches the backend's own limit, so the refusal happens before a round trip. */
const MAX_QUESTION = 1_000;

/**
 * Ask the coach about your own data.
 *
 * The transcript is conversation, not truth. Every figure in a reply was
 * checked against the player's evidence before the backend stored it — an
 * answer quoting a number their matches do not contain is discarded server
 * side and never arrives here. So this renders the text plainly and does not
 * try to verify anything itself; it could not, and pretending otherwise would
 * be the client asserting something it has no basis for.
 */
export function CoachChat({ roleLabel }: { roleLabel: string }) {
  const [data, setData] = useState<ConversationResponse | null>(null);
  const [question, setQuestion] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [paywalled, setPaywalled] = useState(false);
  const endRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    let cancelled = false;

    getConversation()
      .then((response) => {
        if (!cancelled) setData(response);
      })
      .catch((e: unknown) => {
        if (cancelled) return;
        if (e instanceof ApiError && (e.isUnauthenticated || e.status === 409)) return;
        setError(
          e instanceof ApiError ? e.message : "Could not load your conversation.",
        );
      });

    return () => {
      cancelled = true;
    };
  }, []);

  async function send() {
    const asked = question.trim();
    if (!asked || busy) return;

    setBusy(true);
    setError(null);

    // Shown immediately, and replaced by the stored turns on success. The
    // backend writes both turns in one transaction, so a failure leaves
    // nothing behind — and this optimistic turn is removed to match.
    const optimistic: ConversationMessage = {
      id: `pending-${Date.now()}`,
      speaker: "player",
      content: asked,
      evidence: [],
      model: null,
      created_at: new Date().toISOString(),
    };
    setData((current) =>
      current ? { ...current, messages: [...current.messages, optimistic] } : current,
    );
    setQuestion("");

    try {
      const response = await askCoach(asked);
      setData((current) =>
        current
          ? {
              ...current,
              messages: [
                ...current.messages.filter((m) => m.id !== optimistic.id),
                { ...optimistic, id: `${optimistic.id}-sent` },
                response.message,
              ],
            }
          : current,
      );
    } catch (e: unknown) {
      setData((current) =>
        current
          ? {
              ...current,
              messages: current.messages.filter((m) => m.id !== optimistic.id),
            }
          : current,
      );
      setQuestion(asked);

      if (e instanceof ApiError && e.isPaymentRequired) {
        setPaywalled(true);
      } else {
        setError(
          e instanceof ApiError ? e.message : "The coach could not answer just now.",
        );
      }
    } finally {
      setBusy(false);
    }
  }

  useEffect(() => {
    // Guarded: `scrollIntoView` is a convenience, and an environment without
    // it — jsdom, an older embedded webview — must not lose the conversation
    // to a TypeError in an effect.
    endRef.current?.scrollIntoView?.({ block: "nearest" });
  }, [data?.messages.length]);

  if (!data) {
    return <div className="h-48 animate-pulse rounded-card bg-surface-2" aria-busy="true" />;
  }

  return (
    <section className="flex flex-col gap-3">
      <h2 className="font-display text-sm uppercase tracking-[0.2em] text-ink-faint">
        Ask your coach
      </h2>

      {data.note ? <Alert tone="info">{data.note}</Alert> : null}
      {error ? <Alert>{error}</Alert> : null}
      {paywalled ? (
        <Alert tone="info" title="Your trial has ended">
          Asking the coach needs an active subscription.{" "}
          <a href="/billing" className="focus-neon rounded underline">
            See the plan
          </a>
          .
        </Alert>
      ) : null}

      <Card className="flex flex-col gap-4">
        {data.messages.length === 0 ? (
          <p className="text-sm leading-relaxed text-ink-muted">
            Ask about your {roleLabel} games — why something is happening, what
            to do about it, or whether it has got better. The coach answers from
            your measured data and says so when the data cannot tell.
          </p>
        ) : (
          <ol className="flex max-h-[28rem] flex-col gap-3 overflow-y-auto app-scroll">
            {data.messages.map((message) => (
              <Turn key={message.id} message={message} />
            ))}
            <div ref={endRef} />
          </ol>
        )}

        {data.llm_available ? (
          <form
            className="flex flex-col gap-2"
            onSubmit={(event) => {
              event.preventDefault();
              void send();
            }}
          >
            <label htmlFor="coach-question" className="sr-only">
              Ask your coach a question
            </label>
            <textarea
              id="coach-question"
              value={question}
              onChange={(event) => setQuestion(event.target.value.slice(0, MAX_QUESTION))}
              onKeyDown={(event) => {
                // Enter sends, shift+enter breaks the line — the convention
                // every chat surface uses.
                if (event.key === "Enter" && !event.shiftKey) {
                  event.preventDefault();
                  void send();
                }
              }}
              rows={2}
              disabled={busy}
              placeholder={`Why am I dying so much as ${roleLabel.toLowerCase()}?`}
              className="focus-neon min-h-[4.5rem] w-full resize-y rounded-lg border border-border bg-surface-2 p-3 text-sm text-ink placeholder:text-ink-faint disabled:opacity-60"
            />
            <div className="flex items-center justify-between gap-3">
              <span className="font-mono text-[0.6875rem] tabular-nums text-ink-faint">
                {question.length}/{MAX_QUESTION}
              </span>
              <Button type="submit" disabled={busy || !question.trim()}>
                {busy ? "Thinking…" : "Ask"}
              </Button>
            </div>
          </form>
        ) : null}
      </Card>
    </section>
  );
}

function Turn({ message }: { message: ConversationMessage }) {
  const fromPlayer = message.speaker === "player";

  return (
    <li
      className={cn(
        "flex flex-col gap-1",
        fromPlayer ? "items-end" : "items-start",
      )}
    >
      <span className="text-[0.625rem] uppercase tracking-wider text-ink-faint">
        {fromPlayer ? "You" : "Coach"}
      </span>
      <div
        className={cn(
          "max-w-[85%] whitespace-pre-wrap rounded-lg px-3 py-2 text-sm leading-relaxed",
          fromPlayer
            ? "bg-mark-track text-ink"
            : "border border-glass-edge bg-surface-2 text-ink",
        )}
      >
        {message.content}
      </div>

      {/* Provenance, the same way an insight carries it: these are the
          backend statements the reply's figures were checked against. */}
      {message.evidence.length > 0 ? (
        <span className="text-[0.625rem] text-ink-faint">
          Read from {message.evidence.length}{" "}
          {message.evidence.length === 1 ? "measurement" : "measurements"}
        </span>
      ) : null}
    </li>
  );
}
