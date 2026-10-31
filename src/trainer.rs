//! Singleplayer PvP trainer: bot brains and the knockback trainer.
//!
//! The in-game agent (Java side) sends one [`Obs`] per bot per game tick over the IPC bridge;
//! [`TrainerManager::tick`] answers with a [`Command`] (movement keys, aim delta, attack, ...).
//! All decision logic lives here in Rust so it is testable and engine-version independent.

use std::collections::HashMap;
use std::ops::{Add, Mul, Sub};

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use serde::{Deserialize, Serialize};

// ------------------------------------------------------------------------------------------------
// Math
// ------------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct Vec3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Vec3 {
    pub const ZERO: Vec3 = Vec3 { x: 0.0, y: 0.0, z: 0.0 };
    pub fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }
    pub fn horizontal_len(self) -> f64 {
        (self.x * self.x + self.z * self.z).sqrt()
    }
    pub fn len(self) -> f64 {
        (self.x * self.x + self.y * self.y + self.z * self.z).sqrt()
    }
}

impl Add for Vec3 {
    type Output = Vec3;
    fn add(self, o: Vec3) -> Vec3 {
        Vec3::new(self.x + o.x, self.y + o.y, self.z + o.z)
    }
}
impl Sub for Vec3 {
    type Output = Vec3;
    fn sub(self, o: Vec3) -> Vec3 {
        Vec3::new(self.x - o.x, self.y - o.y, self.z - o.z)
    }
}
impl Mul<f64> for Vec3 {
    type Output = Vec3;
    fn mul(self, k: f64) -> Vec3 {
        Vec3::new(self.x * k, self.y * k, self.z * k)
    }
}

/// Wrap an angle difference into (-180, 180].
pub fn wrap_degrees(mut a: f32) -> f32 {
    a %= 360.0;
    if a > 180.0 {
        a -= 360.0;
    }
    if a <= -180.0 {
        a += 360.0;
    }
    a
}

// ------------------------------------------------------------------------------------------------
// Bot configuration and I/O
// ------------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CombatStyle {
    Sword,
    Crystal,
    AxeShield,
    Mace,
    BedwarsRush,
}

impl CombatStyle {
    pub const ALL: [CombatStyle; 5] = [
        CombatStyle::Sword,
        CombatStyle::Crystal,
        CombatStyle::AxeShield,
        CombatStyle::Mace,
        CombatStyle::BedwarsRush,
    ];

    pub fn label(self) -> &'static str {
        match self {
            CombatStyle::Sword => "Sword PvP",
            CombatStyle::Crystal => "Crystal PvP",
            CombatStyle::AxeShield => "Axe & Shield",
            CombatStyle::Mace => "Mace",
            CombatStyle::BedwarsRush => "Bedwars Rush",
        }
    }

    /// 1.9+ attack interval in seconds for a fully charged hit.
    fn cooldown_secs(self) -> f32 {
        match self {
            CombatStyle::Sword | CombatStyle::BedwarsRush => 0.625,
            CombatStyle::Crystal => 0.625,
            CombatStyle::AxeShield => 1.0,
            CombatStyle::Mace => 1.7,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BotConfig {
    pub id: u32,
    pub name: String,
    pub style: CombatStyle,
    /// 0.0 (beginner) ..= 1.0 (very sharp)
    pub difficulty: f32,
    /// 1.8.9 rules: no attack cooldown, CPS based hitting.
    pub legacy_combat: bool,
}

/// What the bot "sees" this tick.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Obs {
    pub bot_pos: Vec3,
    pub bot_yaw: f32,
    pub bot_pitch: f32,
    pub bot_health: f32,
    pub on_ground: bool,
    pub target_pos: Vec3,
    pub target_vel: Vec3,
    pub target_blocking: bool,
    pub target_health: f32,
    /// Distance from an existing end crystal to the target, if one is placed.
    pub crystal_to_target: Option<f32>,
    /// Seconds since the previous observation.
    pub dt: f32,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Command {
    pub forward: bool,
    pub backward: bool,
    pub left: bool,
    pub right: bool,
    pub jump: bool,
    pub sprint: bool,
    pub d_yaw: f32,
    pub d_pitch: f32,
    pub attack: bool,
    /// Right click: raise shield / block.
    pub use_item: bool,
    pub place_crystal: bool,
    pub hit_crystal: bool,
}

// ------------------------------------------------------------------------------------------------
// Bot brain
// ------------------------------------------------------------------------------------------------

pub struct BotBrain {
    cfg: BotConfig,
    rng: StdRng,
    reaction_left: f32,
    had_target_in_range: bool,
    attack_timer: f32,
    strafe_timer: f32,
    strafe_dir: f32,
    wtap_timer: f32,
    crystal_timer: f32,
    shield_timer: f32,
}

impl BotBrain {
    pub fn new(cfg: BotConfig) -> Self {
        let seed = 0x9E37_79B9_7F4A_7C15u64 ^ u64::from(cfg.id);
        Self::with_seed(cfg, seed)
    }

    pub fn with_seed(cfg: BotConfig, seed: u64) -> Self {
        Self {
            rng: StdRng::seed_from_u64(seed),
            cfg,
            reaction_left: 0.0,
            had_target_in_range: false,
            attack_timer: 0.0,
            strafe_timer: 0.0,
            strafe_dir: 1.0,
            wtap_timer: 0.0,
            crystal_timer: 0.0,
            shield_timer: 0.0,
        }
    }

    pub fn config(&self) -> &BotConfig {
        &self.cfg
    }

    fn noise(&mut self) -> f32 {
        // cheap bell-ish distribution in [-1.5, 1.5]
        self.rng.gen::<f32>() + self.rng.gen::<f32>() + self.rng.gen::<f32>() - 1.5
    }

    /// Reach in blocks; sharper bots hit from slightly farther but never beyond vanilla 3.0.
    pub fn reach(&self) -> f64 {
        2.6 + 0.4 * f64::from(self.cfg.difficulty.clamp(0.0, 1.0))
    }

    pub fn tick(&mut self, obs: &Obs) -> Command {
        let d = self.cfg.difficulty.clamp(0.0, 1.0);
        let dt = obs.dt.clamp(0.0, 0.25);
        let mut cmd = Command::default();

        // --- aim ---------------------------------------------------------------------------
        // Lead the target a little, more for better bots.
        let lead = obs.target_vel * (0.05 + 0.1 * f64::from(d));
        let to = (obs.target_pos + lead) - obs.bot_pos;
        let horiz = to.horizontal_len();
        let desired_yaw = (-to.x).atan2(to.z).to_degrees() as f32;
        let desired_pitch = (-(to.y + 0.9)).atan2(horiz.max(0.001)).to_degrees() as f32;

        let yaw_err = wrap_degrees(desired_yaw - obs.bot_yaw);
        let pitch_err = desired_pitch - obs.bot_pitch;
        let max_turn = (160.0 + 700.0 * d) * dt; // deg per tick
        let wobble = (1.0 - d) * 2.5;
        let smooth = 0.25 + 0.5 * d;
        cmd.d_yaw = (yaw_err * smooth + self.noise() * wobble).clamp(-max_turn, max_turn);
        cmd.d_pitch = (pitch_err * smooth + self.noise() * wobble * 0.5).clamp(-max_turn, max_turn);

        let dist = to.len();
        let reach = self.reach();
        let aimed = yaw_err.abs() < 8.0 + 25.0 * (1.0 - d);

        // --- reaction time: first contact is delayed ------------------------------------------------
        let in_range = dist <= reach + 0.2;
        if in_range && !self.had_target_in_range {
            self.reaction_left = 0.35 - 0.28 * d + self.rng.gen::<f32>() * 0.12;
        }
        self.had_target_in_range = in_range;
        self.reaction_left = (self.reaction_left - dt).max(0.0);

        // --- movement: approach, keep spacing, strafe ---------------------------------------------
        self.strafe_timer -= dt;
        if self.strafe_timer <= 0.0 {
            self.strafe_timer = 0.35 + self.rng.gen::<f32>() * 0.9;
            if self.rng.gen::<f32>() < 0.55 {
                self.strafe_dir = -self.strafe_dir;
            }
        }
        let ideal = match self.cfg.style {
            CombatStyle::Crystal => 4.0,
            CombatStyle::Mace => 2.2,
            _ => 2.4,
        };
        if dist > ideal + 0.3 {
            cmd.forward = true;
            cmd.sprint = true;
        } else if dist < ideal - 0.6 {
            cmd.backward = true;
        }
        if dist < 6.0 {
            if self.strafe_dir > 0.0 {
                cmd.right = true;
            } else {
                cmd.left = true;
            }
        }
        // W-tap: briefly drop sprint after a hit to reset it (extra knockback on the target).
        self.wtap_timer = (self.wtap_timer - dt).max(0.0);
        if self.wtap_timer > 0.0 {
            cmd.sprint = false;
            cmd.forward = false;
        }
        // Occasional jumps in close range (crits for sharper bots).
        if obs.on_ground && dist < 4.0 && self.rng.gen::<f32>() < 0.02 + 0.05 * d {
            cmd.jump = true;
        }

        // --- combat -----------------------------------------------------------------------------
        self.attack_timer -= dt;
        match self.cfg.style {
            CombatStyle::Crystal => self.crystal_logic(obs, d, dt, &mut cmd),
            _ => {
                let ready = self.attack_timer <= 0.0 && self.reaction_left <= 0.0;
                let can_hit = in_range && dist <= reach && aimed;
                if self.cfg.style == CombatStyle::AxeShield {
                    self.shield_logic(obs, d, dt, can_hit && ready, &mut cmd);
                }
                if ready && can_hit && !cmd.use_item {
                    cmd.attack = true;
                    self.attack_timer = self.next_attack_interval(d);
                    self.wtap_timer = if d > 0.4 { 0.12 } else { 0.0 };
                }
            }
        }
        cmd
    }

    fn next_attack_interval(&mut self, d: f32) -> f32 {
        if self.cfg.legacy_combat {
            // 1.8: click-speed based, 5..13 CPS depending on difficulty with jitter.
            let cps = 5.0 + 8.0 * d + self.noise() * 1.5;
            1.0 / cps.clamp(3.0, 14.0)
        } else {
            // 1.9+: wait for the cooldown; weaker bots swing a bit early (weaker hits).
            let early = 1.0 - 0.25 * (1.0 - d) * self.rng.gen::<f32>();
            self.cfg.style.cooldown_secs() * early + 0.02
        }
    }

    fn shield_logic(&mut self, obs: &Obs, d: f32, dt: f32, wants_attack: bool, cmd: &mut Command) {
        self.shield_timer = (self.shield_timer - dt).max(0.0);
        let dist = (obs.target_pos - obs.bot_pos).len();
        // Shield up while waiting for the axe cooldown at close range, drop it to swing.
        if self.shield_timer > 0.0 {
            cmd.use_item = true;
            return;
        }
        if !wants_attack && dist < 3.5 && self.rng.gen::<f32>() < 0.04 + 0.08 * d {
            self.shield_timer = 0.4 + self.rng.gen::<f32>() * 0.5;
            cmd.use_item = true;
        }
        // The player holds a shield: axes disable it, so prioritise swinging.
        if obs.target_blocking && wants_attack {
            self.shield_timer = 0.0;
            cmd.use_item = false;
        }
    }

    fn crystal_logic(&mut self, obs: &Obs, d: f32, dt: f32, cmd: &mut Command) {
        self.crystal_timer -= dt;
        let delay = 0.45 - 0.3 * d;
        if self.crystal_timer > 0.0 {
            return;
        }
        match obs.crystal_to_target {
            Some(c) if c < 2.5 && self.reaction_left <= 0.0 => {
                cmd.hit_crystal = true;
                self.crystal_timer = delay;
            }
            None if (obs.target_pos - obs.bot_pos).len() < 7.0 => {
                cmd.place_crystal = true;
                self.crystal_timer = delay;
            }
            _ => {}
        }
    }
}

// ------------------------------------------------------------------------------------------------
// Knockback trainer
// ------------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct KbProfile {
    pub strength: f64,
    pub friction: f64,
    pub vertical_cap: f64,
    /// 1.8 applies vertical knockback even in mid-air; 1.9+ only when grounded.
    pub legacy: bool,
}

impl KbProfile {
    pub fn legacy_1_8() -> Self {
        Self { strength: 0.4, friction: 2.0, vertical_cap: 0.4, legacy: true }
    }
    pub fn modern() -> Self {
        Self { strength: 0.4, friction: 2.0, vertical_cap: 0.4, legacy: false }
    }
}

/// Velocity of the victim after a hit, following vanilla's `knockBack` + sprint/enchant bonus.
///
/// `attacker_yaw` is the attacker's yaw in degrees (Minecraft convention).
pub fn apply_knockback(
    profile: &KbProfile,
    victim_vel: Vec3,
    victim_on_ground: bool,
    attacker_to_victim: Vec3,
    attacker_yaw: f32,
    attacker_sprinting: bool,
    kb_enchant_level: u32,
) -> Vec3 {
    let mut v = victim_vel;
    let (dx, dz) = (attacker_to_victim.x, attacker_to_victim.z);
    let dist = (dx * dx + dz * dz).sqrt().max(1e-4);

    // base knockback: damp old motion, push away from the attacker
    // (`attacker_to_victim` points from attacker to victim, so adding it pushes away).
    v.x = victim_vel.x / profile.friction + dx / dist * profile.strength;
    v.z = victim_vel.z / profile.friction + dz / dist * profile.strength;

    if profile.legacy || victim_on_ground {
        v.y = victim_vel.y / profile.friction + profile.strength;
        if v.y > profile.vertical_cap {
            v.y = profile.vertical_cap;
        }
    }

    // extra knockback: sprint-hit counts as +1 level, each enchant level +1
    let extra = kb_enchant_level + u32::from(attacker_sprinting);
    if extra > 0 {
        let yaw = f64::from(attacker_yaw).to_radians();
        v.x += -yaw.sin() * f64::from(extra) * 0.5;
        v.y += 0.1;
        v.z += yaw.cos() * f64::from(extra) * 0.5;
    }
    v
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct KbSession {
    pub combo: u32,
    pub best_combo: u32,
    pub hits_landed: u32,
    pub hits_taken: u32,
    pub last_hit_secs_ago: f32,
}

impl KbSession {
    /// Advance time; combos expire after 2 s without a hit.
    pub fn tick(&mut self, dt: f32) {
        self.last_hit_secs_ago += dt;
        if self.last_hit_secs_ago > 2.0 {
            self.combo = 0;
        }
    }

    pub fn on_hit_landed(&mut self) {
        self.combo += 1;
        self.best_combo = self.best_combo.max(self.combo);
        self.hits_landed += 1;
        self.last_hit_secs_ago = 0.0;
    }

    pub fn on_hit_taken(&mut self) {
        self.combo = 0;
        self.hits_taken += 1;
    }
}

// ------------------------------------------------------------------------------------------------
// Manager
// ------------------------------------------------------------------------------------------------

#[derive(Default)]
pub struct TrainerManager {
    bots: HashMap<u32, BotBrain>,
    next_id: u32,
    pub kb: KbSession,
}

impl TrainerManager {
    /// Register a bot and return the config to send to the game (which spawns the entity).
    pub fn spawn(&mut self, name: &str, style: CombatStyle, difficulty: f32, legacy: bool) -> BotConfig {
        self.next_id += 1;
        let cfg = BotConfig {
            id: self.next_id,
            name: name.to_owned(),
            style,
            difficulty,
            legacy_combat: legacy,
        };
        self.bots.insert(cfg.id, BotBrain::new(cfg.clone()));
        cfg
    }

    pub fn remove(&mut self, id: u32) {
        self.bots.remove(&id);
    }

    pub fn clear(&mut self) {
        self.bots.clear();
    }

    pub fn len(&self) -> usize {
        self.bots.len()
    }

    pub fn is_empty(&self) -> bool {
        self.bots.is_empty()
    }

    pub fn tick(&mut self, id: u32, obs: &Obs) -> Option<Command> {
        self.bots.get_mut(&id).map(|b| b.tick(obs))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(style: CombatStyle, d: f32, legacy: bool) -> BotConfig {
        BotConfig { id: 1, name: "Bot".into(), style, difficulty: d, legacy_combat: legacy }
    }

    fn obs(dist: f64) -> Obs {
        Obs {
            bot_pos: Vec3::ZERO,
            bot_yaw: 0.0, // facing +z (south)
            bot_pitch: 0.0,
            bot_health: 20.0,
            on_ground: true,
            target_pos: Vec3::new(0.0, 0.0, dist),
            target_vel: Vec3::ZERO,
            target_blocking: false,
            target_health: 20.0,
            crystal_to_target: None,
            dt: 0.05,
        }
    }

    #[test]
    fn bot_approaches_distant_target() {
        let mut b = BotBrain::with_seed(cfg(CombatStyle::Sword, 0.8, false), 1);
        let c = b.tick(&obs(8.0));
        assert!(c.forward && c.sprint && !c.attack);
    }

    #[test]
    fn bot_attacks_in_range_after_reaction_time() {
        let mut b = BotBrain::with_seed(cfg(CombatStyle::Sword, 1.0, true), 2);
        let mut attacked = false;
        for _ in 0..40 {
            if b.tick(&obs(2.2)).attack {
                attacked = true;
                break;
            }
        }
        assert!(attacked, "bot never attacked at close range");
    }

    #[test]
    fn bot_never_hits_beyond_vanilla_reach() {
        let mut b = BotBrain::with_seed(cfg(CombatStyle::Sword, 1.0, true), 3);
        for _ in 0..200 {
            assert!(!b.tick(&obs(3.6)).attack);
        }
    }

    #[test]
    fn legacy_bot_swings_faster_than_axe_bot() {
        let count = |style, legacy| {
            let mut b = BotBrain::with_seed(cfg(style, 0.9, legacy), 4);
            (0..400).filter(|_| b.tick(&obs(2.2)).attack).count()
        };
        assert!(count(CombatStyle::Sword, true) > count(CombatStyle::AxeShield, false));
    }

    #[test]
    fn aim_turns_toward_target() {
        let mut b = BotBrain::with_seed(cfg(CombatStyle::Sword, 1.0, false), 5);
        let mut o = obs(4.0);
        o.target_pos = Vec3::new(4.0, 0.0, 0.0); // +x is yaw -90 in Minecraft
        let c = b.tick(&o);
        assert!(c.d_yaw < 0.0, "expected negative yaw delta, got {}", c.d_yaw);
    }

    #[test]
    fn crystal_bot_places_then_detonates() {
        let mut b = BotBrain::with_seed(cfg(CombatStyle::Crystal, 1.0, false), 6);
        let mut o = obs(5.0);
        let mut placed = false;
        for _ in 0..20 {
            placed |= b.tick(&o).place_crystal;
        }
        assert!(placed);
        o.crystal_to_target = Some(1.0);
        let mut hit = false;
        for _ in 0..40 {
            hit |= b.tick(&o).hit_crystal;
        }
        assert!(hit);
    }

    #[test]
    fn sprint_hit_knocks_back_further() {
        let p = KbProfile::modern();
        let away = Vec3::new(0.0, 0.0, 1.0);
        let walk = apply_knockback(&p, Vec3::ZERO, true, away, 0.0, false, 0);
        let sprint = apply_knockback(&p, Vec3::ZERO, true, away, 0.0, true, 0);
        assert!(sprint.horizontal_len() > walk.horizontal_len());
        assert!(walk.z > 0.0, "victim must be pushed away from the attacker");
    }

    #[test]
    fn legacy_applies_vertical_knockback_in_air_modern_does_not() {
        let away = Vec3::new(0.0, 0.0, 1.0);
        let legacy = apply_knockback(&KbProfile::legacy_1_8(), Vec3::ZERO, false, away, 0.0, false, 0);
        let modern = apply_knockback(&KbProfile::modern(), Vec3::ZERO, false, away, 0.0, false, 0);
        assert!(legacy.y > 0.0);
        assert_eq!(modern.y, 0.0);
    }

    #[test]
    fn combo_tracking() {
        let mut s = KbSession::default();
        s.on_hit_landed();
        s.on_hit_landed();
        assert_eq!(s.combo, 2);
        s.on_hit_taken();
        assert_eq!(s.combo, 0);
        assert_eq!(s.best_combo, 2);
        s.on_hit_landed();
        s.tick(2.5);
        assert_eq!(s.combo, 0);
    }

    #[test]
    fn manager_spawns_unique_ids() {
        let mut m = TrainerManager::default();
        let a = m.spawn("A", CombatStyle::Sword, 0.5, true);
        let b = m.spawn("B", CombatStyle::Mace, 0.5, false);
        assert_ne!(a.id, b.id);
        assert_eq!(m.len(), 2);
        assert!(m.tick(a.id, &obs(3.0)).is_some());
        m.clear();
        assert!(m.is_empty());
    }

    #[test]
    fn wrap_works() {
        assert_eq!(wrap_degrees(190.0), -170.0);
        assert_eq!(wrap_degrees(-190.0), 170.0);
    }
}
