import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { ConfirmDialog } from "./ConfirmDialog";
import { DataTable, type Column } from "./DataTable";

afterEach(cleanup);

describe("ConfirmDialog", () => {
  it("passes the trimmed reason to onConfirm and closes on success", async () => {
    const onConfirm = vi.fn().mockResolvedValue(undefined);
    const onClose = vi.fn();
    render(
      <ConfirmDialog open title="Disable Midz?" confirmLabel="Disable" withReason onConfirm={onConfirm} onClose={onClose} />,
    );

    fireEvent.change(screen.getByLabelText(/Reason/), { target: { value: "  chargeback  " } });
    fireEvent.click(screen.getByRole("button", { name: "Disable" }));

    await waitFor(() => expect(onConfirm).toHaveBeenCalledWith("chargeback"));
    await waitFor(() => expect(onClose).toHaveBeenCalled());
  });

  it("stays open and shows the error when the action fails", async () => {
    const onClose = vi.fn();
    render(
      <ConfirmDialog
        open
        title="Disable?"
        confirmLabel="Disable"
        onConfirm={() => Promise.reject(new Error("Server said no."))}
        onClose={onClose}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Disable" }));

    expect(await screen.findByText("Server said no.")).toBeDefined();
    expect(onClose).not.toHaveBeenCalled();
  });
});

type Row = { id: string; name: string };
const columns: Column<Row, "name">[] = [
  { id: "name", header: "Name", sortKey: "name", rowHeader: true, cell: (r) => r.name },
];

describe("DataTable", () => {
  it("announces the sort and asks for the opposite direction on click", () => {
    const onSort = vi.fn();
    render(
      <DataTable
        caption="People"
        columns={columns}
        rows={[{ id: "1", name: "Amy" }]}
        rowKey={(r) => r.id}
        sort={{ key: "name", dir: "desc" }}
        onSort={onSort}
        empty="Nobody."
      />,
    );

    expect(screen.getByRole("columnheader").getAttribute("aria-sort")).toBe("descending");
    fireEvent.click(screen.getByRole("button", { name: /Name/ }));
    expect(onSort).toHaveBeenCalledWith({ key: "name", dir: "asc" });
  });

  it("shows the empty state inside the table", () => {
    render(<DataTable caption="People" columns={columns} rows={[]} rowKey={(r) => r.id} empty="Nobody matches." />);
    expect(screen.getByText("Nobody matches.")).toBeDefined();
  });
});
