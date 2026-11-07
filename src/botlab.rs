//! Bot lab: a self-play trainer that keeps making the in-game PvP bots smarter.
//!
//! The bots fight with a parametric controller (a dozen tactical numbers: preferred distance,
//! strafe rhythm, counter-strafing, w-tap length, crit-jump range, when to retreat, ...). This
//! module simulates thousands of 1v1 duels with vanilla-like combat (attack cooldown, crits,
//! knockback, invulnerability frames) and improves those numbers with the cross-entropy method:
//! sample a population around the current mean, duel every candidate against a suite of scripted
//! styles plus the previous champion (self-play), keep the elite, repeat. The result is saved to
//! `botlab.json` and pushed to the game, where `BotBrain` (Java) runs the very same controller.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::config::data_dir;

pub const N: usize = 12;

pub struct Spec {
    pub name: &'static str,
    pub min: f32,
    pub max: f32,
    pub def: f32,
}

/// Order is part of the contract with `BotBrain.java` (`P_*` indices).
pub const SPECS: [Spec; N] = [
    Spec { name: "ideal_distance", min: 1.2, max: 3.6, def: 1.9 },
    Spec { name: "strafe_period", min: 0.15, max: 1.5, def: 0.3 },
    Spec { name: "strafe_flip", min: 0.05, max: 0.95, def: 0.55 },
    Spec { name: "counter_strafe", min: 0.0, max: 1.0, def: 1.0 },
    Spec { name: "aim_lead", min: 0.0, max: 0.4, def: 0.1 },
    Spec { name: "wtap_time", min: 0.0, max: 0.3, def: 0.12 },
    Spec { name: "retreat_hp", min: 2.0, max: 10.0, def: 6.0 },
    Spec { name: "retreat_prob", min: 0.0, max: 0.3, def: 0.06 },
    Spec { name: "crit_range", min: 0.2, max: 1.6, def: 0.9 },
    Spec { name: "attack_early", min: 0.0, max: 0.5, def: 0.25 },
    Spec { name: "kite_cooldown", min: 0.05, max: 0.6, def: 0.25 },
    Spec { name: "knock_jump", min: 0.0, max: 1.0, def: 1.0 },
];

pub fn defaults() -> [f32; N] {
    let mut p = [0.0; N];
    for (i, s) in SPECS.iter().enumerate() {
        p[i] = s.def;
    }
    p
}

fn clamp_params(p: &mut [f32; N]) {
    for (i, s) in SPECS.iter().enumerate() {
        p[i] = p[i].clamp(s.min, s.max);
    }
}

// ------------------------------------------------------------------------------------- rng

#[derive(Clone)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0xD1B5_4A32_D192_ED03)
    }
    fn next_u64(&mut self) -> u64 {
        // splitmix64
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    pub fn f(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32
    }
    fn noise(&mut self) -> f32 {
        self.f() + self.f() + self.f() - 1.5
    }
    fn gauss(&mut self) -> f32 {
        // Box-Muller
        let u = self.f().max(1e-7);
        let v = self.f();
        (-2.0 * u.ln()).sqrt() * (std::f32::consts::TAU * v).cos()
    }
}

// ------------------------------------------------------------------------------------- simulation

const DT: f32 = 0.05;
const REACH: f32 = 3.0;
const MAX_SECS: f32 = 45.0;

#[derive(Clone, Copy, Default)]
struct Keys {
    fwd: bool,
    back: bool,
    left: bool,
    right: bool,
    sprint: bool,
    jump: bool,
    attack: bool,
}

#[derive(Clone)]
struct Fighter {
    x: f32,
    z: f32,
    y: f32,
    vx: f32,
    vz: f32,
    vy: f32,
    yaw: f32,
    hp: f32,
    since_attack: f32,
    invul: f32,
    sprinting: bool,
}

impl Fighter {
    fn new(x: f32, z: f32, yaw: f32) -> Self {
        Fighter { x, z, y: 0.0, vx: 0.0, vz: 0.0, vy: 0.0, yaw, hp: 20.0, since_attack: 1.0, invul: 0.0, sprinting: false }
    }
    fn on_ground(&self) -> bool {
        self.y <= 0.001 && self.vy <= 0.0
    }
}

fn wrap(mut a: f32) -> f32 {
    a %= 360.0;
    if a > 180.0 {
        a -= 360.0;
    }
    if a <= -180.0 {
        a += 360.0;
    }
    a
}

/// Movement: keys are relative to the yaw (Minecraft convention, yaw 0 faces +z).
fn step_motion(f: &mut Fighter, k: Keys) {
    let (sin, cos) = f.yaw.to_radians().sin_cos();
    let fwd = (k.fwd as i32 - k.back as i32) as f32;
    let side = (k.right as i32 - k.left as i32) as f32;
    let (mut dx, mut dz) = (-sin * fwd + -cos * side, cos * fwd + -sin * side);
    let len = (dx * dx + dz * dz).sqrt();
    if len > 0.0 {
        dx /= len;
        dz /= len;
    }
    f.sprinting = k.sprint && k.fwd && !k.back;
    let speed = if f.sprinting { 5.6 } else { 4.3 };
    let grip = if f.on_ground() { 0.42 } else { 0.08 };
    f.vx += (dx * speed - f.vx) * grip;
    f.vz += (dz * speed - f.vz) * grip;
    if k.jump && f.on_ground() {
        f.vy = 8.4;
    }
    f.vy -= 31.0 * DT;
    f.y += f.vy * DT;
    if f.y <= 0.0 {
        f.y = 0.0;
        if f.vy < 0.0 {
            f.vy = 0.0;
        }
    }
    f.x += f.vx * DT;
    f.z += f.vz * DT;
    f.since_attack += DT;
    f.invul = (f.invul - DT).max(0.0);
}

/// Resolve `a` hitting `b`. Returns damage dealt.
fn strike(a: &mut Fighter, b: &mut Fighter) -> f32 {
    let dx = b.x - a.x;
    let dz = b.z - a.z;
    let dist = (dx * dx + dz * dz + (b.y - a.y).powi(2)).sqrt();
    let want = wrap((-dx).atan2(dz).to_degrees() - a.yaw).abs();
    let strength = (a.since_attack / 0.625).clamp(0.0, 1.0);
    if dist > REACH || want > 22.0 {
        // swung at nothing: cooldown still resets
        a.since_attack = 0.0;
        return 0.0;
    }
    let mut dmg = 6.0 * (0.2 + strength * strength * 0.8);
    if strength > 0.9 && !a.on_ground() && a.vy < 0.0 && !a.sprinting {
        dmg *= 1.5; // crit
    }
    let sprint_hit = a.sprinting && strength > 0.9;
    a.since_attack = 0.0;
    if b.invul > 0.0 && dmg <= 6.0 {
        return 0.0;
    }
    b.hp -= dmg;
    b.invul = 0.5;
    let flat = (dx * dx + dz * dz).sqrt().max(0.001);
    let kb = if sprint_hit { 9.5 } else { 6.5 };
    b.vx += dx / flat * kb;
    b.vz += dz / flat * kb;
    if b.on_ground() {
        b.vy = 4.5;
    }
    if sprint_hit {
        a.vx *= 0.6;
        a.vz *= 0.6;
    }
    dmg
}

// ------------------------------------------------------------------------------------- brains

/// Tactical controller; the Java `BotBrain` implements the same decisions with the same numbers.
struct Brain {
    p: [f32; N],
    d: f32,
    attack_timer: f32,
    strafe_timer: f32,
    strafe_dir: f32,
    wtap: f32,
    jump_cd: f32,
    retreat: f32,
    last_dist: f32,
    lateral: f32,
    reaction: f32,
    in_range_before: bool,
    arm_left: f32,
    arm: usize,
    t: f32,
}

const ARM_MUL: [f32; 3] = [1.0, 1.58, 1.32];

impl Brain {
    fn new(p: [f32; N], d: f32) -> Self {
        Brain {
            p,
            d,
            attack_timer: 0.0,
            strafe_timer: 0.0,
            strafe_dir: 1.0,
            wtap: 0.0,
            jump_cd: 0.0,
            retreat: 0.0,
            last_dist: 0.0,
            lateral: 0.0,
            reaction: 0.0,
            in_range_before: false,
            arm_left: 0.0,
            arm: 0,
            t: 0.0,
        }
    }

    fn act(&mut self, me: &mut Fighter, foe: &Fighter, rng: &mut Rng) -> Keys {
        let p = self.p;
        let d = self.d;
        let mut k = Keys::default();
        self.t += DT;
        // the in-game bandit rotates between three plans; train across all of them
        self.arm_left -= DT;
        if self.arm_left <= 0.0 {
            self.arm = (self.arm + 1) % 3;
            self.arm_left = 3.0 + rng.f() * 2.0;
        }

        // aim with lead and a human-ish turn rate
        let lead = p[4] * (0.5 + d);
        let tx = foe.x + foe.vx * lead - me.x;
        let tz = foe.z + foe.vz * lead - me.z;
        let desired = (-tx).atan2(tz).to_degrees();
        let err = wrap(desired - me.yaw);
        let max_turn = (160.0 + 700.0 * d) * DT;
        let smooth = 0.35 + 0.5 * d;
        let wobble = (1.0 - d) * 2.5;
        me.yaw += (err * smooth + rng.noise() * wobble).clamp(-max_turn, max_turn);

        let dist = ((foe.x - me.x).powi(2) + (foe.z - me.z).powi(2)).sqrt().max(0.001);
        let nx = (foe.x - me.x) / dist;
        let nz = (foe.z - me.z) / dist;
        let lateral = foe.vx * nz - foe.vz * nx;
        self.lateral += (lateral - self.lateral) * 0.25;
        let knocked = self.last_dist > 0.0 && self.last_dist < 3.2 && dist - self.last_dist > 0.45;
        self.last_dist = dist;
        let aimed = wrap(desired - me.yaw).abs() < 8.0 + 25.0 * (1.0 - d);
        let in_range = dist <= REACH + 0.2;
        if in_range && !self.in_range_before {
            self.reaction = 0.25 - 0.2 * d + rng.f() * 0.1;
        }
        self.in_range_before = in_range;
        self.reaction = (self.reaction - DT).max(0.0);
        self.jump_cd = (self.jump_cd - DT).max(0.0);
        self.retreat = (self.retreat - DT).max(0.0);
        self.wtap = (self.wtap - DT).max(0.0);
        self.attack_timer -= DT;

        let finish = foe.hp > 0.0 && foe.hp <= 6.0;
        if me.hp > 0.0 && me.hp <= p[6] && d > 0.4 && !finish && self.retreat <= 0.0 && rng.f() < p[7] {
            self.retreat = 1.0 + rng.f();
        }
        let retreating = self.retreat > 0.0;
        if knocked && me.on_ground() && d > 0.2 && rng.f() < p[11] {
            k.jump = true;
            self.jump_cd = 0.6;
        }

        self.strafe_timer -= DT;
        if self.strafe_timer <= 0.0 {
            let base = if self.arm == 2 { p[1] * 3.0 } else { p[1] };
            self.strafe_timer = base + rng.f() * (1.0 - 0.4 * d);
            let flip = if self.arm == 2 { p[2] * 0.45 } else { p[2] };
            if rng.f() < flip {
                self.strafe_dir = -self.strafe_dir;
            }
            if d > 0.45 && self.lateral.abs() > 0.08 && rng.f() < d * p[3] {
                self.strafe_dir = if self.lateral > 0.0 { -1.0 } else { 1.0 };
            }
        }
        let ideal = p[0] * ARM_MUL[self.arm];
        let kiting = d > 0.4 && self.attack_timer > p[10] && dist < REACH + 0.5 && self.arm != 0;
        if retreating || kiting {
            k.back = true;
        } else if dist > ideal + 0.3 || finish {
            k.fwd = true;
            k.sprint = true;
        } else if dist < ideal - 0.6 {
            k.back = true;
        }
        if dist < 6.0 {
            if self.strafe_dir > 0.0 {
                k.right = true;
            } else {
                k.left = true;
            }
        }
        if self.wtap > 0.0 {
            k.sprint = false;
            k.fwd = false;
        }

        let crit_style = d > 0.4;
        if crit_style && me.on_ground() && self.jump_cd <= 0.0 && !retreating && self.attack_timer < 0.3 && dist < REACH + p[8] {
            k.jump = true;
            self.jump_cd = 0.75;
        }

        let mut ready = self.attack_timer <= 0.0 && self.reaction <= 0.0;
        if crit_style && !me.on_ground() && me.vy > 0.0 {
            ready = false; // wait for the fall so the hit crits
        }
        if ready && dist <= REACH && aimed {
            k.attack = true;
            self.attack_timer = 0.625 * (1.0 - p[9] * (1.0 - d) * rng.f()) + 0.02;
            self.wtap = if d > 0.4 { p[5] } else { 0.0 };
        }
        k
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Style {
    Rusher,
    Strafer,
    CritJumper,
    Spammer,
}

/// Scripted opponents that stand in for different kinds of players.
struct Scripted {
    style: Style,
    flip_timer: f32,
    dir: f32,
    cps_timer: f32,
}

impl Scripted {
    fn new(style: Style) -> Self {
        Scripted { style, flip_timer: 0.0, dir: 1.0, cps_timer: 0.0 }
    }

    fn act(&mut self, me: &mut Fighter, foe: &Fighter, rng: &mut Rng) -> Keys {
        let tx = foe.x - me.x;
        let tz = foe.z - me.z;
        let dist = (tx * tx + tz * tz).sqrt().max(0.001);
        let desired = (-tx).atan2(tz).to_degrees();
        me.yaw += (wrap(desired - me.yaw) * 0.6 + rng.noise() * 1.0).clamp(-30.0, 30.0);
        let mut k = Keys::default();
        self.flip_timer -= DT;
        self.cps_timer -= DT;
        match self.style {
            Style::Rusher => {
                k.fwd = dist > 1.6;
                k.sprint = true;
                if dist <= REACH && self.cps_timer <= 0.0 {
                    k.attack = true;
                    self.cps_timer = 0.55 + rng.f() * 0.1;
                }
            }
            Style::Strafer => {
                if self.flip_timer <= 0.0 {
                    self.dir = -self.dir;
                    self.flip_timer = 0.3 + rng.f() * 0.5;
                }
                if dist > 2.9 {
                    k.fwd = true;
                    k.sprint = true;
                } else if dist < 2.2 {
                    k.back = true;
                }
                if self.dir > 0.0 {
                    k.right = true
                } else {
                    k.left = true
                }
                if dist <= REACH && me.since_attack >= 0.62 {
                    k.attack = true;
                }
            }
            Style::CritJumper => {
                k.fwd = dist > 2.0;
                k.sprint = dist > 3.0;
                if dist < 3.6 && me.on_ground() {
                    k.jump = true;
                }
                if dist <= REACH && !me.on_ground() && me.vy < 0.0 && me.since_attack >= 0.62 {
                    k.attack = true;
                }
            }
            Style::Spammer => {
                k.fwd = dist > 2.2;
                if self.flip_timer <= 0.0 {
                    self.dir = -self.dir;
                    self.flip_timer = 0.8;
                }
                if self.dir > 0.0 {
                    k.right = true
                } else {
                    k.left = true
                }
                if dist <= REACH && self.cps_timer <= 0.0 {
                    k.attack = true;
                    self.cps_timer = 0.11 + rng.f() * 0.08;
                }
            }
        }
        k
    }
}

enum Opponent {
    Script(Scripted),
    Brain(Brain),
}

impl Opponent {
    fn act(&mut self, me: &mut Fighter, foe: &Fighter, rng: &mut Rng) -> Keys {
        match self {
            Opponent::Script(s) => s.act(me, foe, rng),
            Opponent::Brain(b) => b.act(me, foe, rng),
        }
    }
}

/// Opponent kinds used for training and evaluation.
#[derive(Clone, Copy)]
enum Kind {
    Script(Style),
    Params([f32; N]),
}

/// Result of one duel from the candidate's point of view, in -1.5 ..= 1.5, plus whether it won.
fn duel(cand: &[f32; N], kind: Kind, seed: u64) -> (f32, bool) {
    let mut rng = Rng::new(seed);
    let side = if rng.f() < 0.5 { 1.0 } else { -1.0 };
    let mut a = Fighter::new(0.0, -3.5 * side, if side > 0.0 { 0.0 } else { 180.0 });
    let mut b = Fighter::new(0.0, 3.5 * side, if side > 0.0 { 180.0 } else { 0.0 });
    let mut brain = Brain::new(*cand, 0.85);
    let mut foe = match kind {
        Kind::Script(s) => Opponent::Script(Scripted::new(s)),
        Kind::Params(p) => Opponent::Brain(Brain::new(p, 0.85)),
    };
    let steps = (MAX_SECS / DT) as usize;
    for _ in 0..steps {
        let ka = brain.act(&mut a, &b, &mut rng);
        let kb = foe.act(&mut b, &a, &mut rng);
        step_motion(&mut a, ka);
        step_motion(&mut b, kb);
        // simultaneous swings: the one whose cooldown is fuller resolves first
        let (fa, fb) = (a.since_attack, b.since_attack);
        if ka.attack && kb.attack && fb > fa {
            strike(&mut b, &mut a);
            strike(&mut a, &mut b);
        } else {
            if ka.attack {
                strike(&mut a, &mut b);
            }
            if kb.attack {
                strike(&mut b, &mut a);
            }
        }
        // keep the fighters from standing inside each other
        let dx = b.x - a.x;
        let dz = b.z - a.z;
        let flat = (dx * dx + dz * dz).sqrt();
        if flat < 0.6 && flat > 0.0001 {
            let push = (0.6 - flat) / 2.0;
            a.x -= dx / flat * push;
            a.z -= dz / flat * push;
            b.x += dx / flat * push;
            b.z += dz / flat * push;
        }
        if a.hp <= 0.0 || b.hp <= 0.0 {
            break;
        }
    }
    let diff = ((a.hp.max(0.0) - b.hp.max(0.0)) / 20.0).clamp(-1.0, 1.0);
    if b.hp <= 0.0 && a.hp > 0.0 {
        (1.0 + 0.5 * a.hp / 20.0, true)
    } else if a.hp <= 0.0 && b.hp > 0.0 {
        (-1.0 + 0.5 * (20.0 - b.hp) / 20.0, false)
    } else {
        (diff, diff > 0.0)
    }
}

const STYLES: [Style; 4] = [Style::Rusher, Style::Strafer, Style::CritJumper, Style::Spammer];

/// Average score and win rate of `p` against the four scripted styles (`rounds` duels each).
pub fn evaluate(p: &[f32; N], rounds: usize, seed: u64) -> (f32, f32) {
    let mut score = 0.0;
    let mut wins = 0;
    let mut total = 0;
    for (si, s) in STYLES.iter().enumerate() {
        for r in 0..rounds {
            let (sc, w) = duel(p, Kind::Script(*s), seed ^ ((si as u64) << 20) ^ (r as u64 * 7919));
            score += sc;
            wins += w as usize;
            total += 1;
        }
    }
    (score / total as f32, wins as f32 / total as f32)
}

// ------------------------------------------------------------------------------------- the lab

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Lab {
    pub generation: u32,
    /// Current best parameter set (real units, see [`SPECS`]).
    pub mean: Vec<f32>,
    /// Per-parameter search width, as a fraction of the parameter range.
    pub sigma: Vec<f32>,
    /// Win rate of `mean` against the scripted styles (held-out seeds).
    pub win_rate: f32,
    /// Same for the untrained default parameters.
    pub baseline_rate: f32,
    /// Score history (one point per evaluation), newest last.
    pub history: Vec<f32>,
    /// Hall of fame: older champions the next generations must beat.
    pub champions: Vec<Vec<f32>>,
}

impl Default for Lab {
    fn default() -> Self {
        Lab {
            generation: 0,
            mean: defaults().to_vec(),
            sigma: vec![0.22; N],
            win_rate: 0.0,
            baseline_rate: 0.0,
            history: Vec::new(),
            champions: Vec::new(),
        }
    }
}

fn arr(v: &[f32]) -> [f32; N] {
    let mut a = defaults();
    for (i, x) in v.iter().take(N).enumerate() {
        a[i] = *x;
    }
    clamp_params(&mut a);
    a
}

fn path() -> PathBuf {
    data_dir().join("botlab.json")
}

impl Lab {
    pub fn load() -> Lab {
        let mut lab: Lab = std::fs::read(path()).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
        lab.mean = arr(&lab.mean).to_vec();
        lab.sigma.resize(N, 0.1);
        lab
    }

    pub fn save(&self) {
        let _ = std::fs::create_dir_all(data_dir());
        if let Ok(b) = serde_json::to_vec_pretty(self) {
            let _ = std::fs::write(path(), b);
        }
    }

    pub fn params(&self) -> Vec<f32> {
        arr(&self.mean).to_vec()
    }

    pub fn reset(&mut self) {
        *self = Lab::default();
    }

    /// One generation of the cross-entropy method.
    pub fn generation_step(&mut self) {
        const POP: usize = 28;
        const ELITE: usize = 7;
        let g = self.generation as u64;
        let mean = arr(&self.mean);
        let mut rng = Rng::new(0xF057 ^ g.wrapping_mul(0x51ED));
        // opponents: the four scripted styles, the untrained defaults and the last champions
        let mut foes: Vec<Kind> = STYLES.iter().map(|s| Kind::Script(*s)).collect();
        foes.push(Kind::Params(defaults()));
        for c in self.champions.iter().rev().take(2) {
            foes.push(Kind::Params(arr(c)));
        }
        let mut cands: Vec<([f32; N], f32)> = Vec::with_capacity(POP + 1);
        let mut samples: Vec<[f32; N]> = vec![mean];
        for _ in 0..POP {
            let mut c = mean;
            for i in 0..N {
                let range = SPECS[i].max - SPECS[i].min;
                c[i] += rng.gauss() * self.sigma[i] * range;
            }
            clamp_params(&mut c);
            samples.push(c);
        }
        for c in samples {
            // every candidate meets every opponent on the same seeds (common random numbers)
            let mut total = 0.0;
            for (fi, foe) in foes.iter().enumerate() {
                for r in 0..2u64 {
                    total += duel(&c, *foe, g * 1_000_003 + fi as u64 * 131 + r * 17).0;
                }
            }
            cands.push((c, total / (foes.len() * 2) as f32));
        }
        cands.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        let elite = &cands[..ELITE];
        let mut new_mean = [0.0f32; N];
        for i in 0..N {
            let m: f32 = elite.iter().map(|e| e.0[i]).sum::<f32>() / ELITE as f32;
            let range = SPECS[i].max - SPECS[i].min;
            let var: f32 = elite.iter().map(|e| (e.0[i] - m).powi(2)).sum::<f32>() / ELITE as f32;
            new_mean[i] = 0.6 * m + 0.4 * mean[i];
            // keep exploring: never collapse below 3 % of the range
            self.sigma[i] = (0.7 * self.sigma[i] + 0.3 * (var.sqrt() / range) * 1.4).clamp(0.03, 0.3);
        }
        clamp_params(&mut new_mean);
        self.mean = new_mean.to_vec();
        self.generation += 1;

        if self.generation % 10 == 0 {
            // the champion joins the hall of fame, later generations have to beat it
            self.champions.push(self.mean.clone());
            if self.champions.len() > 6 {
                self.champions.remove(0);
            }
            self.evaluate_now();
        }
    }

    pub fn evaluate_now(&mut self) {
        let (_, win) = evaluate(&arr(&self.mean), 8, 0xE7A1);
        self.win_rate = win;
        self.baseline_rate = evaluate(&defaults(), 8, 0xE7A1).1;
        self.history.push(win);
        if self.history.len() > 240 {
            self.history.remove(0);
        }
    }

    pub fn run(&mut self, generations: u32) {
        for _ in 0..generations {
            self.generation_step();
        }
        if self.history.is_empty() {
            self.evaluate_now();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duels_are_deterministic_and_bounded() {
        let p = defaults();
        let a = duel(&p, Kind::Script(Style::Strafer), 5);
        let b = duel(&p, Kind::Script(Style::Strafer), 5);
        assert_eq!(a.0, b.0);
        assert!(a.0.abs() <= 1.5);
    }

    #[test]
    fn default_bot_fights_back() {
        let (score, win) = evaluate(&defaults(), 4, 99);
        assert!(win > 0.15, "default bot should win some duels, got {win} (score {score})");
    }

    #[test]
    fn training_does_not_get_worse() {
        let mut lab = Lab::default();
        let before = evaluate(&arr(&lab.mean), 6, 0xAB).0;
        lab.run(12);
        let after = evaluate(&arr(&lab.mean), 6, 0xAB).0;
        assert!(after >= before - 0.05, "before {before} after {after}");
        assert!(lab.generation == 12);
        for (i, s) in SPECS.iter().enumerate() {
            assert!(lab.mean[i] >= s.min && lab.mean[i] <= s.max);
        }
    }
}
