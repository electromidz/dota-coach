import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { ApiError } from "@/lib/api";
import type { ConversationResponse } from "@/lib/types";

import { CoachChat } from "./CoachChat";

const getConversation = vi.hoisted(() => vi.fn());
const askCoach = vi.hoisted(() => vi.fn());

vi.mock("@/lib/api", async () => {
  const actual = await vi.importActual<typeof import("@/lib/api")>("@/lib/api");
  return { ...actual, getConversation, askCoach };
});

function conversation(
  overrides: Partial<ConversationResponse> = {},
): ConversationResponse {
  return {
    role: "carry",
    role_label: "Carry",
    messages: [],
    llm_available: true,
    note: null,
    ...overrides,
  };
}

afterEach(() => {
  getConversation.mockReset();
  askCoach.mockReset();
});

describe("CoachChat", () => {
  it("sends a question and shows the reply", async () => {
    getConversation.mockResolvedValue(conversation());
    askCoach.mockResolvedValue({
      role: "carry",
      role_label: "Carry",
      message: {
        id: "reply-1",
        speaker: "coach",
        content: "Your deaths are what is holding you back.",
        evidence: ["overall.deaths"],
        model: "test-model",
        created_at: new Date().toISOString(),
      },
    });

    render(<CoachChat roleLabel="Carry" />);

    const box = await screen.findByLabelText("Ask your coach a question");
    fireEvent.change(box, { target: { value: "Why am I dying so much?" } });
    fireEvent.click(screen.getByRole("button", { name: "Ask" }));

    expect(
      await screen.findByText("Your deaths are what is holding you back."),
    ).toBeDefined();
    // Both turns are on screen, attributed.
    expect(screen.getByText("Why am I dying so much?")).toBeDefined();

    // Provenance, the same way an insight carries it.
    expect(screen.getByText("Read from 1 measurement")).toBeDefined();
  });

  it("puts the question back when the coach cannot answer", async () => {
    getConversation.mockResolvedValue(conversation());
    askCoach.mockRejectedValue(
      new ApiError("UPSTREAM", "The coach's answer quoted a figure that is not in your data.", 502),
    );

    render(<CoachChat roleLabel="Carry" />);

    const box = await screen.findByLabelText("Ask your coach a question");
    fireEvent.change(box, { target: { value: "How are my deaths?" } });
    fireEvent.click(screen.getByRole("button", { name: "Ask" }));

    expect(await screen.findByText(/quoted a figure/)).toBeDefined();

    // The backend stored neither turn, so neither is left in the transcript.
    // Scoped to list items: the question is also back in the textarea, which
    // a bare text query would match.
    await waitFor(() => {
      expect(screen.queryAllByRole("listitem")).toHaveLength(0);
    });
    // And it is restored rather than lost, so the player can retry.
    expect((box as HTMLTextAreaElement).value).toBe("How are my deaths?");
  });

  it("offers the plan rather than an error when the trial has ended", async () => {
    getConversation.mockResolvedValue(conversation());
    askCoach.mockRejectedValue(
      new ApiError("PAYMENT_REQUIRED", "Your trial has ended.", 402),
    );

    render(<CoachChat roleLabel="Carry" />);

    const box = await screen.findByLabelText("Ask your coach a question");
    fireEvent.change(box, { target: { value: "What should I work on?" } });
    fireEvent.click(screen.getByRole("button", { name: "Ask" }));

    expect(await screen.findByText("Your trial has ended")).toBeDefined();
    expect(screen.getByText("See the plan")).toBeDefined();
  });

  it("hides the composer when no model is configured but still reads", async () => {
    getConversation.mockResolvedValue(
      conversation({
        llm_available: false,
        note: "No coaching model is configured.",
        messages: [
          {
            id: "old",
            speaker: "coach",
            content: "Earlier advice.",
            evidence: [],
            model: "test-model",
            created_at: new Date().toISOString(),
          },
        ],
      }),
    );

    render(<CoachChat roleLabel="Carry" />);

    // The transcript survives a deployment with no model; only asking stops.
    expect(await screen.findByText("Earlier advice.")).toBeDefined();
    expect(screen.getByText("No coaching model is configured.")).toBeDefined();
    expect(screen.queryByRole("button", { name: "Ask" })).toBeNull();
  });

  it("will not send an empty question", async () => {
    getConversation.mockResolvedValue(conversation());

    render(<CoachChat roleLabel="Carry" />);

    const button = await screen.findByRole("button", { name: "Ask" });
    expect((button as HTMLButtonElement).disabled).toBe(true);
    expect(askCoach).not.toHaveBeenCalled();
  });
});
