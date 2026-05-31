import type { Metadata } from "next";
import changelog from "../../data/changelog.json";
import {
  CATEGORIES,
  getEntryCategory,
  isMinorEntry,
  type CategoryStyle,
  type ChangelogEntry,
} from "../../lib/changelog-categories";

export const metadata: Metadata = {
  title: "Changelog",
  description: "What's new in Vibn — features, improvements, and the road to launch.",
};

type ChangelogDay = {
  date: string;
  entries: ChangelogEntry[];
};

function formatDate(iso: string): string {
  return new Date(`${iso}T00:00:00Z`).toLocaleDateString("en-US", {
    year: "numeric",
    month: "long",
    day: "numeric",
    timeZone: "UTC",
  });
}

function CategoryIcon({ category }: { category: CategoryStyle }) {
  const Icon = category.icon;
  return (
    <div
      className={
        "relative mt-0.5 grid h-10 w-10 shrink-0 place-items-center overflow-hidden rounded-xl border shadow-[inset_0_1px_0_rgba(255,255,255,0.04),0_10px_24px_rgba(2,6,23,0.18)] " +
        category.iconTileClass
      }
    >
      <Icon aria-hidden strokeWidth={1.85} className={`h-[1.05rem] w-[1.05rem] ${category.iconClass}`} />
    </div>
  );
}

function MajorEntry({ entry, isFirst }: { entry: ChangelogEntry; isFirst: boolean }) {
  const category = getEntryCategory(entry);
  return (
    <div
      className={
        "relative overflow-hidden px-6 py-5 " +
        (isFirst ? "" : "border-t border-[color:var(--color-border)]/55")
      }
    >
      <div
        aria-hidden
        className="pointer-events-none absolute left-0 top-0 h-32 w-56"
        style={{ background: `radial-gradient(ellipse at top left, ${category.glow} 0%, transparent 78%)` }}
      />
      <div className="relative flex items-start gap-4">
        <CategoryIcon category={category} />
        <div className="min-w-0 flex-1">
          <h3 className="text-[15px] font-semibold tracking-tight text-white">{entry.title}</h3>
          <p className="mt-1 text-[13.5px] leading-relaxed text-[color:var(--color-text-dim)]">
            {entry.description}
          </p>
          <div className="mt-2.5">
            <span
              className={
                "inline-flex rounded-full border px-2 py-[0.22rem] text-[9.5px] font-semibold uppercase tracking-[0.16em] backdrop-blur-sm " +
                category.badgeClass
              }
            >
              {category.label}
            </span>
          </div>
        </div>
      </div>
    </div>
  );
}

function MinorRow({
  entries,
  hasContentAbove,
}: {
  entries: ChangelogEntry[];
  hasContentAbove: boolean;
}) {
  if (entries.length === 0) return null;
  return (
    <div
      className={
        "bg-white/[0.012] px-6 py-3 " +
        (hasContentAbove ? "border-t border-[color:var(--color-border)]/55" : "")
      }
    >
      <div className="mb-2 text-[10px] font-semibold uppercase tracking-[0.18em] text-[color:var(--color-text-dim)]">
        Also
      </div>
      <ul className="space-y-1.5 pl-0">
        {entries.map((entry, i) => {
          const cat = getEntryCategory(entry);
          return (
            <li key={i} className="flex items-baseline gap-2 text-[12.5px] leading-snug">
              <span
                aria-hidden
                className="mt-[5px] h-1.5 w-1.5 shrink-0 rounded-full"
                style={{ background: cat.glow.replace("0.12", "0.6").replace("0.13", "0.6").replace("0.11", "0.6").replace("0.10", "0.6").replace("0.09", "0.6") }}
              />
              <div>
                <span className="font-medium text-white/85">{entry.title}</span>
                <span className="text-[color:var(--color-text-dim)]"> — {entry.description}</span>
              </div>
            </li>
          );
        })}
      </ul>
    </div>
  );
}

export default function ChangelogPage() {
  const days = changelog as ChangelogDay[];
  const categoriesInUse = Array.from(
    new Set(days.flatMap((d) => d.entries.map((e) => getEntryCategory(e).label))),
  )
    .map((label) => CATEGORIES[label])
    .filter(Boolean);

  return (
    <section className="relative isolate overflow-hidden pt-28 pb-24 sm:pt-32">
      <div className="pointer-events-none absolute inset-0 -z-10">
        <div
          className="absolute left-1/2 top-0 h-[520px] w-[520px] -translate-x-1/2 rounded-full blur-[120px] anim-drift-1"
          style={{ background: "radial-gradient(circle, rgba(124,60,255,0.30), transparent 65%)" }}
        />
        <div className="absolute inset-0 dot-grid opacity-50" />
      </div>

      <div className="mx-auto max-w-3xl px-6">
        <div className="text-center">
          <div className="inline-flex rounded-full border border-[color:var(--color-border)] bg-[color:var(--color-surface)]/50 px-3 py-1 text-xs text-[color:var(--color-text-dim)] backdrop-blur">
            Changelog
          </div>
          <h1 className="mt-5 text-balance text-4xl font-semibold tracking-tight sm:text-5xl">
            <span className="gradient-text">What&rsquo;s new</span>{" "}
            <span className="text-white/85">in Vibn.</span>
          </h1>
          <p className="mx-auto mt-4 max-w-xl text-pretty text-[15px] text-[color:var(--color-text-dim)]">
            Notable features and improvements, in reverse chronological order. We ship in
            the open — follow along here.
          </p>
        </div>

        {categoriesInUse.length > 0 && (
          <div className="mt-8 flex flex-wrap justify-center gap-1.5">
            {categoriesInUse.map((c) => (
              <span
                key={c.label}
                className={
                  "inline-flex items-center gap-1.5 rounded-full border px-2.5 py-1 text-[10.5px] font-semibold uppercase tracking-[0.14em] " +
                  c.badgeClass
                }
              >
                <c.icon className={`h-3 w-3 ${c.iconClass}`} aria-hidden />
                {c.label}
              </span>
            ))}
          </div>
        )}

        <div className="mt-14 space-y-12">
          {days.map((day) => {
            const major = day.entries.filter((e) => !isMinorEntry(e));
            const minor = day.entries.filter(isMinorEntry);
            return (
              <article key={day.date}>
                <time
                  dateTime={day.date}
                  className="mb-4 block text-[13px] font-semibold tracking-[0.02em] text-[color:var(--color-text-dim)]"
                >
                  {formatDate(day.date)}
                </time>
                <div className="rounded-2xl bg-[linear-gradient(135deg,rgba(124,60,255,0.45)_0%,rgba(242,65,183,0.4)_50%,rgba(255,138,61,0.45)_100%)] p-px">
                  <div className="overflow-hidden rounded-[calc(1rem-1px)] bg-[color:var(--color-bg-2)]/92 backdrop-blur-sm">
                    {major.map((entry, i) => (
                      <MajorEntry key={`major-${i}`} entry={entry} isFirst={i === 0} />
                    ))}
                    <MinorRow entries={minor} hasContentAbove={major.length > 0} />
                  </div>
                </div>
              </article>
            );
          })}
        </div>

        <p className="mt-16 text-center text-[12px] text-[color:var(--color-text-dim)]">
          Want to be first in line? <span className="text-white/80">Star us on launch day.</span>
        </p>
      </div>
    </section>
  );
}
