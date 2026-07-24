// The words the popover puts on a verdict. Kept apart from the DOM so the
// phrasing — the part a person actually reads — can be tested.

/** "47 min", "2 h 5 min". Anything under a minute is not worth a number. */
export function formatHeld(seconds) {
  if (seconds < 60) return "just now";
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes} min`;
  const hours = Math.floor(minutes / 60);
  const rest = minutes % 60;
  return rest ? `${hours} h ${rest} min` : `${hours} h`;
}

/** Headline and one line under it. */
export function verdictText(verdict) {
  if (verdict.mood === "charging") {
    return {
      title: "Not watching",
      sub: "Plugged in — it stays awake on purpose",
    };
  }
  const count = verdict.holders.length;
  if (count === 0) {
    return {
      title: "It will sleep",
      sub: verdict.sleep_after
        ? `${verdict.sleep_after} min after you stop touching it`
        : "Nothing is holding it",
    };
  }
  return {
    title: "It won't sleep",
    sub: count === 1 ? "One app is holding it awake" : `${count} apps are holding it awake`,
  };
}

/** The small print under an app's name: what it says it is doing. */
export function holderDetail(holder) {
  const what = holder.blocks === "display" ? "keeps the screen on" : "keeps it awake";
  return holder.via ? `${what} · via ${holder.via}` : what;
}
