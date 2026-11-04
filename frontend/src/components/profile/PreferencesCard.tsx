"use client";

import { useId, useState, useSyncExternalStore } from "react";

import { Card } from "@/components/ui/Card";
import { ApiError, selectCoachingRole } from "@/lib/api";
import {
  COACH_LANGUAGES,
  getDevicePreferencesSnapshot,
  subscribeDevicePreferences,
  timezones,
  writeDevicePreferences,
  type DevicePreferences,
} from "@/lib/profile";
import type { CoachableRole, RoleSelectionResponse } from "@/lib/types";

const SELECT =
  "min-h-11 w-full rounded-xl border border-glass-edge bg-surface-2/60 px-3 text-sm text-ink outline-none focus-neon disabled:cursor-not-allowed disabled:opacity-60";

type Status = { tone: "ok" | "error"; text: string } | null;

/**
 * Three settings, two kinds of storage.
 *
 * The default role is the coaching role, so it saves to the backend — the same
 * call the Coach page makes — and is applied optimistically: the select moves
 * at once and steps back if the server refuses. Language and timezone have no
 * account field yet, so they stay in this browser and the card says so.
 */
export function PreferencesCard({
  roles,
  onRolesChange,
}: {
  roles: RoleSelectionResponse | null;
  onRolesChange: (next: RoleSelectionResponse) => void;
}) {
  const id = useId();
  /** The optimistic choice while it is being saved; the server's otherwise. */
  const [pending, setPending] = useState<CoachableRole | null>(null);
  const role = pending ?? roles?.profile?.selected_role ?? "";
  const saving = pending !== null;
  // Storage exists only in the browser: `null` on the server and first paint.
  const device = useSyncExternalStore(
    subscribeDevicePreferences,
    getDevicePreferencesSnapshot,
    () => null,
  );
  const [status, setStatus] = useState<Status>(null);

  async function changeRole(next: CoachableRole) {
    setPending(next);
    setStatus(null);

    try {
      const saved = await selectCoachingRole(next);
      onRolesChange(saved);
      setStatus({ tone: "ok", text: `Coaching role set to ${saved.profile?.selected_role_label ?? next}.` });
    } catch (error) {
      // Dropping the pending value is the revert: the select falls back to
      // what the server still holds.
      setStatus({
        tone: "error",
        text: error instanceof ApiError ? error.message : "Could not save your role. Try again.",
      });
    } finally {
      setPending(null);
    }
  }

  function changeDevice(patch: Partial<DevicePreferences>) {
    if (!device) return;
    const next = { ...device, ...patch };
    setStatus(
      writeDevicePreferences(next)
        ? { tone: "ok", text: "Saved on this device." }
        : { tone: "error", text: "This browser is blocking storage, so this will not be remembered." },
    );
  }

  return (
    <Card className="flex flex-col gap-4">
      <div>
        <h2 className="text-sm text-ink">Preferences</h2>
        <p className="text-xs text-ink-faint">Changes save as you make them.</p>
      </div>

      <div className="flex flex-col gap-1.5">
        <label htmlFor={`${id}-role`} className="text-xs text-ink-muted">
          Default role
        </label>
        <select
          id={`${id}-role`}
          className={SELECT}
          value={role}
          disabled={!roles || saving}
          aria-describedby={`${id}-role-hint`}
          onChange={(e) => void changeRole(e.target.value as CoachableRole)}
        >
          {role === "" ? <option value="">Choose a role</option> : null}
          {(roles?.selectable_roles ?? []).map((option) => (
            <option key={option.role} value={option.role}>
              {option.position}. {option.label}
            </option>
          ))}
        </select>
        <p id={`${id}-role-hint`} className="text-[0.6875rem] text-ink-faint">
          Coaching, benchmarks and training focus are scoped to this role.
        </p>
      </div>

      <div className="flex flex-col gap-1.5">
        <label htmlFor={`${id}-lang`} className="text-xs text-ink-muted">
          Coach language
        </label>
        <select
          id={`${id}-lang`}
          className={SELECT}
          value={device?.language ?? "en"}
          disabled={!device}
          aria-describedby={`${id}-device-hint`}
          onChange={(e) => changeDevice({ language: e.target.value })}
        >
          {COACH_LANGUAGES.map((lang) => (
            <option key={lang.value} value={lang.value}>
              {lang.label}
            </option>
          ))}
        </select>
      </div>

      <div className="flex flex-col gap-1.5">
        <label htmlFor={`${id}-tz`} className="text-xs text-ink-muted">
          Timezone
        </label>
        <select
          id={`${id}-tz`}
          className={SELECT}
          value={device?.timezone ?? ""}
          disabled={!device}
          aria-describedby={`${id}-device-hint`}
          onChange={(e) => changeDevice({ timezone: e.target.value })}
        >
          {device
            ? timezones(device.timezone).map((zone) => (
                <option key={zone} value={zone}>
                  {zone.replaceAll("_", " ")}
                </option>
              ))
            : null}
        </select>
        <p id={`${id}-device-hint`} className="text-[0.6875rem] text-ink-faint">
          Language and timezone are kept in this browser for now; the coach
          still answers in English.
        </p>
      </div>

      <p
        role="status"
        aria-live="polite"
        className={status?.tone === "error" ? "text-xs text-error" : "text-xs text-string"}
      >
        {status?.text ?? ""}
      </p>
    </Card>
  );
}
