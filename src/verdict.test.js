import { describe, it, expect } from "vitest";
import { formatHeld, verdictText, holderDetail } from "./verdict.js";

const holder = (over = {}) => ({
  app: "Transmission",
  via: null,
  pid: 1,
  blocks: "sleep",
  label: "Transmission: Active Torrents",
  held: 2820,
  ...over,
});

describe("formatHeld", () => {
  it("does not put a number on something that just started", () => {
    expect(formatHeld(7)).toBe("just now");
  });

  it("counts in minutes, then in hours", () => {
    expect(formatHeld(2820)).toBe("47 min");
    expect(formatHeld(3900)).toBe("1 h 5 min");
    expect(formatHeld(7200)).toBe("2 h");
  });
});

describe("verdictText", () => {
  it("says how long the wait is when nothing is holding it", () => {
    const { title, sub } = verdictText({ mood: "calm", holders: [], sleep_after: 1 });
    expect(title).toBe("It will sleep");
    expect(sub).toBe("1 min after you stop touching it");
  });

  it("counts the holders instead of listing them", () => {
    expect(verdictText({ mood: "blocked", holders: [holder()], sleep_after: 1 }).sub).toBe(
      "One app is holding it awake",
    );
    expect(
      verdictText({ mood: "blocked", holders: [holder(), holder()], sleep_after: 1 }).sub,
    ).toBe("2 apps are holding it awake");
  });

  it("explains the charger rather than calling it a problem", () => {
    // Staying awake on power is deliberate — the popover must not read as an alarm.
    const { title, sub } = verdictText({ mood: "charging", holders: [holder()], sleep_after: 1 });
    expect(title).toBe("Not watching");
    expect(sub).toContain("on purpose");
  });
});

describe("holderDetail", () => {
  it("names the broker when there is one", () => {
    // The speakers are held by the audio service on the app's behalf; hiding
    // that would make the line look wrong to anyone who checks with pmset.
    expect(holderDetail(holder({ app: "Google Chrome", via: "coreaudiod" }))).toBe(
      "keeps it awake · via coreaudiod",
    );
  });

  it("separates screen holders from sleep holders", () => {
    expect(holderDetail(holder({ blocks: "display" }))).toBe("keeps the screen on");
  });
});
