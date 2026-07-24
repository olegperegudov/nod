//! Кто мешает маку уснуть — разбор `pmset -g assertions`.
//!
//! Ядро Nod: всё остальное (иконка, попап) — показ того, что вернёт эта пара
//! функций. Отвечаем на один вопрос: уснёт ли мак, если хозяин встанет и уйдёт.
//!
//! Отсеиваем три вещи, и в них вся суть:
//!
//! * Самоистекающие ассерты (`Timeout will fire in N secs`) — фоновые агенты
//!   держат `caffeinate -i -t 300`, он отпустит сам.
//! * `powerd` / «Prevent sleep while display is on» — производная: powerd
//!   блокирует сон, ПОКА горит экран. Виновник — тот, кто держит экран.
//! * Ассерты, чей таймер выключен в профиле батареи (см. `Timers`).
//!
//! `coreaudiod` — не виновник, а посредник: он держит колонки за того, кто их
//! открыл (`Created for PID: N`). Резолвим до имени приложения, иначе список врёт.

use std::process::Command;

/// Что именно держатель не даёт сделать.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Blocks {
    Sleep,
    Display,
}

/// Один держатель — уже в том виде, в каком он ляжет в попап.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Holder {
    /// Приложение, которое реально держит (не посредник).
    pub app: String,
    /// Через кого держит, если через посредника: `coreaudiod` и т.п.
    pub via: Option<String>,
    /// pid, которому слать сигнал по крестику.
    pub pid: u32,
    pub blocks: Blocks,
    /// Как пишет о себе сам держатель: «Transmission: Active Torrents».
    pub label: String,
    /// Сколько уже держит, секунд.
    pub held: u64,
}

/// Настроенные таймеры сна для профиля питания, минуты. Ноль = сон выключен.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Timers {
    pub sleep: u32,
    pub display: u32,
}

fn run(cmd: &str, args: &[&str]) -> String {
    Command::new(cmd)
        .args(args)
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
        .unwrap_or_default()
}

/// Имя приложения по pid: helper'ы прячутся внутри .app, показываем сам .app.
fn app_name(pid: u32, fallback: &str) -> String {
    let path = run("ps", &["-p", &pid.to_string(), "-o", "comm="]);
    let path = path.trim();
    if path.is_empty() {
        return fallback.to_string();
    }
    if let Some(app) = path.split('/').find(|part| part.ends_with(".app")) {
        return app.trim_end_matches(".app").to_string();
    }
    path.rsplit('/').next().unwrap_or(fallback).to_string()
}

pub fn on_battery() -> bool {
    run("pmset", &["-g", "ps"]).contains("Battery Power")
}

/// Таймеры профиля батареи.
///
/// Судим всегда по батарее: на зарядке `sleep 0` — намеренная настройка (мак не
/// спит, чтобы длинные фоновые задачи доработали), и держатели там не новость.
/// А вопрос «уснёт ли, если я уйду» — ровно про батарею.
pub fn battery_timers() -> Timers {
    parse_timers(&run("pmset", &["-g", "custom"]))
}

/// Читаем НАСТРОЕННЫЕ таймеры, а не живые: `pmset -g` под блокировкой печатает
/// `sleep 0` и тем самым прячет реальную настройку.
fn parse_timers(custom: &str) -> Timers {
    let block = custom
        .split_once("Battery Power")
        .map(|(_, rest)| rest.split("Power:").next().unwrap_or(rest))
        .unwrap_or("");

    let value_of = |key: &str| -> u32 {
        block
            .lines()
            .find_map(|line| {
                let mut words = line.split_whitespace();
                match (words.next(), words.next()) {
                    (Some(name), Some(value)) if name == key => value.parse().ok(),
                    _ => None,
                }
            })
            .unwrap_or(0)
    };

    Timers {
        sleep: value_of("sleep"),
        display: value_of("displaysleep"),
    }
}

pub fn holders() -> Vec<Holder> {
    let assertions = run("pmset", &["-g", "assertions"]);
    parse_holders(&assertions, battery_timers(), &app_name)
}

/// Разбор вывода `pmset -g assertions`.
///
/// `resolve` отделён ради тестов: живой `ps` в них не позовёшь, а имена
/// приложений — половина смысла этой функции.
fn parse_holders(
    assertions: &str,
    timers: Timers,
    resolve: &dyn Fn(u32, &str) -> String,
) -> Vec<Holder> {
    let mut found = Vec::new();

    for line in assertions.lines() {
        if let Some(entry) = parse_header(line) {
            found.push(entry);
            continue;
        }
        // Продолжения записи идут с отступом табом и уточняют последнюю.
        if let Some(last) = found.last_mut() {
            if line.starts_with('\t') {
                if line.contains("Timeout will fire in") {
                    last.self_expiring = true;
                }
                if let Some(pid) = line.split("Created for PID:").nth(1) {
                    last.for_pid = pid.trim().trim_end_matches('.').parse().ok();
                }
            }
        }
    }

    found
        .into_iter()
        .filter(|raw| !raw.self_expiring)
        // powerd держит систему, пока горит экран — следствие, а не причина.
        .filter(|raw| raw.proc != "powerd")
        .filter_map(|raw| {
            let blocks = match raw.kind.as_str() {
                "PreventUserIdleDisplaySleep" | "InternalPreventDisplaySleep" => Blocks::Display,
                "PreventUserIdleSystemSleep" | "PreventSystemSleep" => Blocks::Sleep,
                _ => return None,
            };
            let limit = match blocks {
                Blocks::Display => timers.display,
                Blocks::Sleep => timers.sleep,
            };
            if limit == 0 {
                return None;
            }

            // Посредник — только тот, кто держит ЗА другой процесс. «Steam
            // Helper», разрешённый в «Steam», — не посредник, а тот же самый
            // Steam, и приписка «через Steam Helper» была бы шумом.
            let via = raw.for_pid.map(|_| raw.proc.clone());
            let pid = raw.for_pid.unwrap_or(raw.pid);
            let app = resolve(pid, &raw.proc);

            Some(Holder { app, via, pid, blocks, label: raw.label, held: raw.held })
        })
        .collect()
}

struct RawAssertion {
    pid: u32,
    proc: String,
    kind: String,
    label: String,
    held: u64,
    self_expiring: bool,
    for_pid: Option<u32>,
}

/// `   pid 29940(Transmission): [0x000a…] 00:34:00 PreventUserIdleSystemSleep named: "…"`
fn parse_header(line: &str) -> Option<RawAssertion> {
    let rest = line.trim_start().strip_prefix("pid ")?;
    let (pid, rest) = rest.split_once('(')?;
    let (proc, rest) = rest.split_once("): ")?;
    let rest = rest.trim_start();
    // Идентификатор ассерта в квадратных скобках нам не нужен ни для чего.
    let rest = rest.strip_prefix('[')?.split_once(']')?.1.trim_start();
    let (clock, rest) = rest.split_once(' ')?;
    let (kind, rest) = rest.split_once(" named: ")?;
    let label = rest.trim().trim_matches('"').to_string();

    let mut parts = clock.split(':');
    let hours: u64 = parts.next()?.parse().ok()?;
    let minutes: u64 = parts.next()?.parse().ok()?;
    let seconds: u64 = parts.next()?.parse().ok()?;

    Some(RawAssertion {
        pid: pid.trim().parse().ok()?,
        proc: proc.to_string(),
        kind: kind.to_string(),
        label,
        held: hours * 3600 + minutes * 60 + seconds,
        self_expiring: false,
        for_pid: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Дословный `pmset -g assertions` с мака 2026-07-24 — дня, когда он перестал
    /// засыпать: Steam держал экран, Ribbit через coreaudiod держал систему.
    /// Нужный набор держателей на живой машине руками не собрать.
    const ASSERTIONS: &str = concat!(
        "Assertion status system-wide:\n",
        "   PreventUserIdleSystemSleep     1\n",
        "Listed by owning process:\n",
        "   pid 594(WindowServer): [0x000a42de00099d74] 00:00:00 UserIsActive named: \"tickle\"\n",
        "\tTimeout will fire in 600 secs Action=TimeoutActionRelease\n",
        "   pid 29940(Transmission): [0x000a427500019cd4] 00:34:00 PreventUserIdleSystemSleep named: \"Transmission: Active Torrents\"\n",
        "   pid 535(powerd): [0x000a42df00019d75] 00:32:14 PreventUserIdleSystemSleep named: \"Powerd - Prevent sleep while display is on\"\n",
        "   pid 46131(coreaudiod): [0x000a4a6600018bf1] 00:00:07 PreventUserIdleSystemSleep named: \"com.apple.audio.BuiltInSpeakerDevice.context.preventuseridlesleep\"\n",
        "\tCreated for PID: 46609.\n",
        "\tResources: audio-out BuiltInSpeakerDevice\n",
        "   pid 28131(caffeinate): [0x000a49ed00019df3] 00:02:07 PreventUserIdleSystemSleep named: \"caffeinate command-line tool\"\n",
        "\tDetails: caffeinate asserting for 300 secs\n",
        "\tTimeout will fire in 172 secs Action=TimeoutActionRelease\n",
        "   pid 771(Steam Helper): [0x000a42df00019e01] 01:05:00 PreventUserIdleDisplaySleep named: \"Steam\"\n",
        "Kernel Assertions: 0x104=USB,MAGICWAKE\n",
    );

    const BATTERY: Timers = Timers { sleep: 1, display: 2 };

    fn fake_resolve(pid: u32, fallback: &str) -> String {
        match pid {
            46609 => "Google Chrome".to_string(),
            771 => "Steam".to_string(),
            _ => fallback.to_string(),
        }
    }

    fn parse(timers: Timers) -> Vec<Holder> {
        parse_holders(ASSERTIONS, timers, &fake_resolve)
    }

    #[test]
    fn reports_every_real_holder() {
        let mut names: Vec<_> = parse(BATTERY).into_iter().map(|h| h.app).collect();
        names.sort();
        assert_eq!(names, ["Google Chrome", "Steam", "Transmission"]);
    }

    #[test]
    fn self_expiring_and_derived_are_dropped() {
        let names: Vec<_> = parse(BATTERY).into_iter().map(|h| h.app).collect();
        assert!(!names.contains(&"caffeinate".to_string()), "сам отпустит через 172 сек");
        assert!(!names.contains(&"powerd".to_string()), "следствие горящего экрана");
    }

    #[test]
    fn user_is_active_is_not_a_holder() {
        let names: Vec<_> = parse(BATTERY).into_iter().map(|h| h.app).collect();
        assert!(!names.contains(&"WindowServer".to_string()));
    }

    #[test]
    fn broker_is_resolved_to_the_app_that_took_the_speakers() {
        let chrome = parse(BATTERY).into_iter().find(|h| h.app == "Google Chrome").unwrap();
        assert_eq!(chrome.via.as_deref(), Some("coreaudiod"));
        // Крестик должен убить Chrome, а не системную службу звука.
        assert_eq!(chrome.pid, 46609);
    }

    #[test]
    fn holder_carries_its_age_and_what_it_blocks() {
        let steam = parse(BATTERY).into_iter().find(|h| h.app == "Steam").unwrap();
        assert_eq!(steam.blocks, Blocks::Display);
        assert_eq!(steam.held, 3900);
        assert_eq!(steam.via, None, "Steam Helper живёт внутри Steam.app, посредника нет");
    }

    #[test]
    fn disabled_timer_hides_its_holders() {
        // Сон выключен, экран гасится: остаются только держатели экрана.
        let names: Vec<_> = parse(Timers { sleep: 0, display: 10 })
            .into_iter()
            .map(|h| h.app)
            .collect();
        assert_eq!(names, ["Steam"]);
    }

    #[test]
    fn battery_profile_is_read_not_the_live_one() {
        // Живой `pmset -g` под блокировкой печатает sleep 0 — берём настройку.
        let custom = "Battery Power:\n displaysleep 2\n sleep 1\nAC Power:\n displaysleep 10\n sleep 0\n";
        assert_eq!(parse_timers(custom), BATTERY);
    }
}
