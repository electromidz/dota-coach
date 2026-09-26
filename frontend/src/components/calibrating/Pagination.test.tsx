import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { Pagination, pageWindow } from "./Pagination";

describe("pageWindow", () => {
  it("keeps its full width at both ends rather than shrinking", () => {
    // On page one a merely centred window would spend half its slots on pages
    // below one, and the strip would change size as the reader moves.
    expect(pageWindow(1, 20)).toEqual([1, 2, 3, 4, 5, 6]);
    expect(pageWindow(20, 20)).toEqual([15, 16, 17, 18, 19, 20]);
  });

  it("centres on the current page in the middle of a long history", () => {
    const window = pageWindow(10, 20);

    expect(window).toContain(10);
    expect(window.length).toBe(6);
    expect(window[0]).toBe(8);
  });

  it("never offers a page that does not exist", () => {
    expect(pageWindow(1, 3)).toEqual([1, 2, 3]);
    expect(pageWindow(2, 2)).toEqual([1, 2]);
    expect(pageWindow(1, 1)).toEqual([1]);
  });
});

describe("Pagination", () => {
  it("is absent when there is only one page to be on", () => {
    const { container } = render(
      <Pagination page={1} totalPages={1} onChange={vi.fn()} />,
    );

    expect(container.querySelector("nav")).toBeNull();
  });

  it("reports the page that was asked for", () => {
    const onChange = vi.fn();
    render(<Pagination page={3} totalPages={9} onChange={onChange} />);

    fireEvent.click(screen.getByLabelText("Page 5"));
    expect(onChange).toHaveBeenCalledWith(5);

    fireEvent.click(screen.getByText("Next"));
    expect(onChange).toHaveBeenCalledWith(4);

    fireEvent.click(screen.getByText("Prev"));
    expect(onChange).toHaveBeenCalledWith(2);
  });

  it("cannot step past either end", () => {
    const onChange = vi.fn();
    const { unmount } = render(
      <Pagination page={1} totalPages={4} onChange={onChange} />,
    );

    fireEvent.click(screen.getByText("Prev"));
    expect(onChange).not.toHaveBeenCalled();
    unmount();

    render(<Pagination page={4} totalPages={4} onChange={onChange} />);
    fireEvent.click(screen.getByText("Next"));
    expect(onChange).not.toHaveBeenCalled();
  });

  it("marks the current page for a screen reader, not only in colour", () => {
    render(<Pagination page={2} totalPages={4} onChange={vi.fn()} />);

    expect(screen.getByLabelText("Page 2").getAttribute("aria-current")).toBe(
      "page",
    );
    expect(
      screen.getByLabelText("Page 3").getAttribute("aria-current"),
    ).toBeNull();
    expect(screen.getByText("Page 2 of 4")).toBeTruthy();
  });

  it("takes no input while a page is loading", () => {
    const onChange = vi.fn();
    render(
      <Pagination page={2} totalPages={4} disabled onChange={onChange} />,
    );

    fireEvent.click(screen.getByLabelText("Page 4"));
    fireEvent.click(screen.getByText("Next"));
    expect(onChange).not.toHaveBeenCalled();
  });
});
