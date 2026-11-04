"use client";

import Link from "next/link";
import { useCallback, useEffect, useState } from "react";

import { DetailSkeleton } from "@/components/admin/Skeletons";
import { Alert } from "@/components/ui/Alert";
import { Button } from "@/components/ui/Button";
import { Card } from "@/components/ui/Card";
import { ConfirmDialog } from "@/components/ui/ConfirmDialog";
import { CopyButton } from "@/components/ui/CopyButton";
import { Icon } from "@/components/ui/Icon";
import { RelativeTime } from "@/components/ui/RelativeTime";
import { Segmented } from "@/components/ui/Segmented";
import { StatusBadge } from "@/components/ui/StatusBadge";
import { ApiError } from "@/lib/api";
import {
  accountStatusLabel,
  activityDetail,
  activityFeed,
  activityIcon,
  activityLabel,
  activityTone,
  clockTime,
  displayName,
  durationLabel,
  exactTime,
  groupByDay,
  planName,
  shortDate,
  subscriptionLabel,
  subscriptionTone,
  type Tone,
} from "@/lib/admin/format";
import { disableUser, enableUser, extendAccess, getUser } from "@/lib/admin/queries";
import type { ActivityEvent, AdminUserDetail } from "@/lib/admin/types";
import { cn } from "@/lib/utils";

const EXTEND_PRESETS = [7, 14, 30, 90];

type Pending = "extend" | "disable" | "enable" | null;

const TONE_TEXT: Record<Tone, string> = {
  neutral: "text-ink-faint",
  info: "text-function",
  good: "text-string",
  warn: "text-warn",
  bad: "text-error",
  accent: "text-keyword",
};

export function UserDetail({ id }: { id: string }) {
  const [data, setData] = useState<AdminUserDetail | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [notFound, setNotFound] = useState(false);
  const [pending, setPending] = useState<Pending>(null);
  const [days, setDays] = useState(7);
  const [notice, setNotice] = useState<string | null>(null);

  const load = useCallback(async () => {
    try {
      setData(await getUser(id));
      setError(null);
    } catch (e) {
      if (e instanceof ApiError && e.status === 404) {
        setNotFound(true);
        return;
      }
      setError(e instanceof ApiError ? e.message : "Could not load this account.");
    }
  }, [id]);

  useEffect(() => {
    let cancelled = false;
    getUser(id)
      .then((detail) => !cancelled && setData(detail))
      .catch((e) => {
        if (cancelled) return;
        if (e instanceof ApiError && e.status === 404) setNotFound(true);
        else setError(e instanceof ApiError ? e.message : "Could not load this account.");
      });
    return () => {
      cancelled = true;
    };
  }, [id]);

  /** Runs inside the dialog: a throw keeps it open with the message. */
  async function act(run: () => Promise<unknown>, done: string) {
    await run();
    await load();
    setNotice(done);
  }

  if (notFound) {
    return (
      <div className="flex flex-col gap-4">
        <BackLink />
        <Alert title="Not found">No such account.</Alert>
      </div>
    );
  }
  if (!data) return error ? <Alert>{error}</Alert> : <DetailSkeleton />;

  const name = displayName(data);
  const disabled = data.status === "disabled";

  return (
    <div className="flex flex-col gap-6">
      <BackLink />
      {error ? <Alert>{error}</Alert> : null}
      <div aria-live="polite" className="empty:hidden">
        {notice ? (
          <Alert tone="success" title="Done">
            {notice}
          </Alert>
        ) : null}
      </div>

      <Card className="flex flex-wrap items-start justify-between gap-4">
        <div className="flex min-w-0 items-center gap-4">
          {data.avatar_url ? (
            // eslint-disable-next-line @next/next/no-img-element
            <img src={data.avatar_url} alt="" className="size-14 shrink-0 rounded-full border border-glass-edge" />
          ) : (
            <span className="flex size-14 shrink-0 items-center justify-center rounded-full border border-glass-edge bg-surface-2 font-display text-lg text-ink-faint">
              {name.slice(0, 1).toUpperCase()}
            </span>
          )}
          <div className="min-w-0">
            <h2 className="truncate font-display text-lg tracking-wide text-ink">{name}</h2>
            <p className="flex items-center gap-1 font-mono text-xs tabular-nums text-ink-faint">
              {data.steam_id}
              <CopyButton value={data.steam_id} label="SteamID64" />
            </p>
            <p className="text-xs text-ink-faint">
              Joined <span title={exactTime(data.created_at)}>{shortDate(data.created_at)}</span> ·{" "}
              {data.last_login_at ? (
                <>
                  last seen <RelativeTime iso={data.last_login_at} />
                </>
              ) : (
                "never signed in"
              )}
            </p>
          </div>
        </div>

        <div className="flex flex-wrap items-center gap-2">
          {data.is_admin ? <StatusBadge tone="accent">Admin</StatusBadge> : null}
          <StatusBadge label="Account status" tone={disabled ? "bad" : "neutral"}>
            Account: {accountStatusLabel(data.status)}
          </StatusBadge>
          <StatusBadge label="Subscription status" tone={subscriptionTone(data.subscription_status)}>
            Subscription: {subscriptionLabel(data.subscription_status)}
          </StatusBadge>
        </div>
      </Card>

      <div className="grid gap-4 lg:grid-cols-2">
        <Card className="flex flex-col gap-4">
          <h2 className="text-xs uppercase tracking-wider text-ink-faint">Subscription</h2>
          <dl className="grid grid-cols-2 gap-3 text-sm">
            <Fact label="Status">{subscriptionLabel(data.subscription_status)}</Fact>
            <Fact label="Plan">{planName(data.subscription_plan)}</Fact>
            <Fact label="Trial ends">
              {data.trial_ends_at ? <DateWithTitle iso={data.trial_ends_at} /> : "—"}
            </Fact>
            <Fact label="Paid through">
              {data.current_period_end ? <DateWithTitle iso={data.current_period_end} /> : "—"}
            </Fact>
          </dl>
          <Button
            variant="ghost"
            onClick={() => setPending("extend")}
            disabled={data.subscription_status === null}
            className="self-start px-5 text-sm"
          >
            <Icon name="clock" className="size-4" />
            Extend access…
          </Button>
          {data.subscription_status === null ? (
            <p className="text-xs text-ink-faint">
              This account has never signed in, so there is no access to extend yet.
            </p>
          ) : null}
        </Card>

        <Card className="flex flex-col gap-4">
          <h2 className="text-xs uppercase tracking-wider text-ink-faint">Account</h2>
          <p className="text-sm text-ink-muted">
            {disabled
              ? "This account is disabled and refused on every request. Re-enabling takes effect on its next request."
              : "Disabling refuses every future request from this account, checked fresh each time."}
          </p>
          {disabled ? (
            <Button
              variant="ghost"
              onClick={() => setPending("enable")}
              className="self-start border-string/40 px-5 text-sm text-string hover:border-string hover:text-string"
            >
              <Icon name="check" className="size-4" />
              Re-enable account…
            </Button>
          ) : (
            <Button
              variant="ghost"
              onClick={() => setPending("disable")}
              className="self-start border-error/40 px-5 text-sm text-error hover:border-error hover:text-error"
            >
              <Icon name="alert" className="size-4" />
              Disable account…
            </Button>
          )}
        </Card>
      </div>

      <ActivityFeed events={activityFeed(data)} />

      <ConfirmDialog
        open={pending === "extend"}
        title={`Extend access for ${name}`}
        description="Adds time to whichever window governs their access now. A lapsed account is let back in."
        confirmLabel={`Extend by ${durationLabel(days)}`}
        withReason
        onClose={() => setPending(null)}
        onConfirm={(reason) =>
          act(() => extendAccess(id, days, reason), `Access extended by ${durationLabel(days)}.`)
        }
      >
        <Segmented<string>
          label="How long"
          value={String(days)}
          options={EXTEND_PRESETS.map((d) => ({ value: String(d), label: durationLabel(d) }))}
          onChange={(v) => setDays(Number(v))}
        />
      </ConfirmDialog>

      <ConfirmDialog
        open={pending === "disable"}
        title={`Disable ${name}?`}
        description="They will be refused on their very next request. You can re-enable the account later."
        confirmLabel="Disable account"
        tone="danger"
        withReason
        onClose={() => setPending(null)}
        onConfirm={(reason) => act(() => disableUser(id, reason), "Account disabled.")}
      />

      <ConfirmDialog
        open={pending === "enable"}
        title={`Re-enable ${name}?`}
        description="They can sign in and use the app again on their next request."
        confirmLabel="Re-enable account"
        withReason
        onClose={() => setPending(null)}
        onConfirm={(reason) => act(() => enableUser(id, reason), "Account re-enabled.")}
      />
    </div>
  );
}

function ActivityFeed({ events }: { events: ActivityEvent[] }) {
  const [filter, setFilter] = useState<"all" | "admin" | "account">("all");
  const shown =
    filter === "all" ? events : events.filter((e) => (filter === "admin" ? e.kind === "admin" : e.kind === "event"));
  const groups = groupByDay(shown);

  return (
    <section aria-labelledby="activity-heading" className="flex flex-col gap-3">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <h2 id="activity-heading" className="text-xs uppercase tracking-wider text-ink-faint">
          Activity
        </h2>
        <Segmented
          label="Show"
          value={filter}
          options={[
            { value: "all", label: "All" },
            { value: "account", label: "Account" },
            { value: "admin", label: "Admin actions" },
          ]}
          onChange={setFilter}
        />
      </div>

      {groups.length === 0 ? (
        <Card>
          <p className="text-sm text-ink-muted">Nothing recorded yet.</p>
        </Card>
      ) : (
        <Card className="flex flex-col gap-5">
          {groups.map((group) => (
            <div key={group.day} className="flex flex-col gap-2">
              <h3 className="text-xs font-semibold text-ink-muted">{group.day}</h3>
              <ol className="flex flex-col">
                {group.rows.map((event) => {
                  const detail = activityDetail(event);
                  return (
                    <li
                      key={event.id}
                      className="flex items-start gap-3 border-l border-glass-edge py-2 pl-4"
                    >
                      <span
                        className={cn(
                          "-ml-[1.6rem] flex size-6 shrink-0 items-center justify-center rounded-full border border-glass-edge bg-surface",
                          TONE_TEXT[activityTone(event.type)],
                        )}
                      >
                        <Icon name={activityIcon(event.type)} className="size-3.5" />
                      </span>
                      <div className="min-w-0 flex-1">
                        <p className="text-sm text-ink">
                          {activityLabel(event.type)}
                          {detail ? <span className="text-ink-faint"> · {detail}</span> : null}
                        </p>
                        {event.kind === "admin" ? (
                          <p className="text-xs text-ink-faint">
                            By {event.actor}
                            {event.reason ? (
                              <>
                                {" "}— <q className="text-ink-muted">{event.reason}</q>
                              </>
                            ) : null}
                          </p>
                        ) : null}
                      </div>
                      <time
                        dateTime={event.at}
                        title={exactTime(event.at)}
                        className="shrink-0 font-mono text-xs tabular-nums text-ink-faint"
                      >
                        {clockTime(event.at)}
                      </time>
                    </li>
                  );
                })}
              </ol>
            </div>
          ))}
        </Card>
      )}
    </section>
  );
}

function Fact({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="flex flex-col gap-0.5">
      <dt className="text-xs text-ink-faint">{label}</dt>
      <dd className="text-ink">{children}</dd>
    </div>
  );
}

function DateWithTitle({ iso }: { iso: string }) {
  return (
    <time dateTime={iso} title={exactTime(iso)}>
      {shortDate(iso)}
    </time>
  );
}

function BackLink() {
  return (
    <Link
      href="/admin/users"
      className="focus-neon inline-flex min-h-11 w-fit cursor-pointer items-center gap-1.5 rounded text-sm text-ink-muted transition-colors duration-200 ease-out hover:text-function"
    >
      <Icon name="chevron-left" className="size-5" />
      All users
    </Link>
  );
}
