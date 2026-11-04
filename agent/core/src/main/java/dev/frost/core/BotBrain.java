package dev.frost.core;

import java.util.Random;

/**
 * PvP brain of a trainer bot, running inside the game (no launcher round trip needed).
 * Port of trainer.rs BotBrain plus an online-learning layer: the bot tries different fighting
 * "plans" (pressure / spacing / circling), measures damage dealt minus damage taken for each, and
 * gradually prefers what works against the player it is fighting.
 */
public final class BotBrain {
    /** Fighting plans the learner chooses between. */
    static final int PRESSURE = 0, SPACING = 1, CIRCLE = 2;

    private final Models.BotConfig cfg;
    private final Random rng;
    private float reactionLeft, attackTimer, strafeTimer, wtapTimer, crystalTimer, shieldTimer;
    private float airTime, jumpCooldown, retreatLeft, lastDist, targetLateral;
    private float strafeDir = 1f;
    private boolean hadTargetInRange;

    // --- learning layer ---
    private final float[] armValue = new float[3];
    private final int[] armCount = new int[3];
    private int arm = PRESSURE;
    private float episodeLeft;
    private float epTargetHp = -1f, epBotHp = -1f;
    private float prevTargetHp = -1f, prevBotHp = -1f;
    /** Rough count of how often the player blocks/strafes, used for flavour in debug output. */
    public float lastReward;

    public BotBrain(Models.BotConfig cfg, long seed) {
        this.cfg = cfg;
        this.rng = new Random(seed);
        // optimistic start so every plan is tried
        for (int i = 0; i < 3; i++) armValue[i] = 0.5f;
    }

    public int currentArm() {
        return arm;
    }

    private float noise() {
        return rng.nextFloat() + rng.nextFloat() + rng.nextFloat() - 1.5f;
    }

    public double reach() {
        return 2.6 + 0.4 * clamp01(cfg.difficulty);
    }

    private static float clamp01(float v) {
        return Math.max(0f, Math.min(1f, v));
    }

    private static float wrap(float a) {
        a %= 360f;
        if (a > 180f) a -= 360f;
        if (a <= -180f) a += 360f;
        return a;
    }

    private void learn(Models.Obs obs, float dt) {
        if (prevTargetHp < 0f) {
            prevTargetHp = obs.target_health;
            prevBotHp = obs.bot_health;
        }
        episodeLeft -= dt;
        if (episodeLeft > 0f) return;
        if (epTargetHp >= 0f) {
            float dealt = Math.max(0f, epTargetHp - obs.target_health);
            float taken = Math.max(0f, epBotHp - obs.bot_health);
            float reward = dealt - taken;
            lastReward = reward;
            armCount[arm]++;
            armValue[arm] += (reward - armValue[arm]) / Math.min(armCount[arm] + 1, 8);
        }
        // epsilon-greedy: smarter bots exploit more
        float eps = 0.35f - 0.2f * clamp01(cfg.difficulty);
        if (rng.nextFloat() < eps) {
            arm = rng.nextInt(3);
        } else {
            int best = 0;
            for (int i = 1; i < 3; i++) if (armValue[i] > armValue[best]) best = i;
            arm = best;
        }
        epTargetHp = obs.target_health;
        epBotHp = obs.bot_health;
        episodeLeft = 3f + rng.nextFloat() * 2f;
    }

    public Models.BotCommand tick(Models.Obs obs) {
        float d = clamp01(cfg.difficulty);
        float dt = Math.max(0f, Math.min(0.25f, obs.dt));
        Models.BotCommand cmd = new Models.BotCommand();
        learn(obs, dt);

        // --- aim: lead the target, turn speed and wobble depend on difficulty
        double lx = obs.target_vel.x * (0.05 + 0.1 * d);
        double ly = obs.target_vel.y * (0.05 + 0.1 * d);
        double lz = obs.target_vel.z * (0.05 + 0.1 * d);
        double tx = obs.target_pos.x + lx - obs.bot_pos.x;
        double ty = obs.target_pos.y + ly - obs.bot_pos.y;
        double tz = obs.target_pos.z + lz - obs.bot_pos.z;
        double horiz = Math.sqrt(tx * tx + tz * tz);
        float desiredYaw = (float) Math.toDegrees(Math.atan2(-tx, tz));
        float desiredPitch = (float) Math.toDegrees(Math.atan2(-(ty + 0.9), Math.max(horiz, 0.001)));
        float yawErr = wrap(desiredYaw - obs.bot_yaw);
        float pitchErr = desiredPitch - obs.bot_pitch;
        float maxTurn = (160f + 700f * d) * Math.max(dt, 0.05f);
        float wobble = (1f - d) * 2.5f;
        float smooth = 0.35f + 0.5f * d;
        cmd.d_yaw = Math.max(-maxTurn, Math.min(maxTurn, yawErr * smooth + noise() * wobble));
        cmd.d_pitch = Math.max(-maxTurn, Math.min(maxTurn, pitchErr * smooth + noise() * wobble * 0.5f));

        double dist = Math.sqrt(tx * tx + ty * ty + tz * tz);
        double reach = reach();
        boolean aimed = Math.abs(yawErr) < 8f + 25f * (1f - d);

        // --- bookkeeping
        if (obs.on_ground) airTime = 0f; else airTime += dt;
        jumpCooldown = Math.max(0f, jumpCooldown - dt);
        retreatLeft = Math.max(0f, retreatLeft - dt);
        double flat = Math.max(0.001, Math.sqrt(Math.pow(obs.target_pos.x - obs.bot_pos.x, 2) + Math.pow(obs.target_pos.z - obs.bot_pos.z, 2)));
        double nx = (obs.target_pos.x - obs.bot_pos.x) / flat, nz = (obs.target_pos.z - obs.bot_pos.z) / flat;
        float lateral = (float) (obs.target_vel.x * nz - obs.target_vel.z * nx);
        targetLateral += (lateral - targetLateral) * 0.25f;
        boolean knocked = lastDist > 0f && lastDist < 3.2f && (dist - lastDist) > 1.4;
        lastDist = (float) dist;

        // --- reaction time at first contact
        boolean inRange = dist <= reach + 0.2;
        if (inRange && !hadTargetInRange) reactionLeft = 0.25f - 0.2f * d + rng.nextFloat() * 0.1f;
        hadTargetInRange = inRange;
        reactionLeft = Math.max(0f, reactionLeft - dt);

        // --- tactics: back off when hurt, finish when the player is low
        boolean finish = obs.target_health > 0f && obs.target_health <= 6f;
        if (obs.bot_health > 0f && obs.bot_health <= 6f && d > 0.4f && !finish && retreatLeft <= 0f && rng.nextFloat() < 0.06f) {
            retreatLeft = 1.0f + rng.nextFloat();
        }
        boolean retreating = retreatLeft > 0f;
        if (knocked && obs.on_ground && d > 0.2f) {
            cmd.jump = true;
            jumpCooldown = 0.6f;
        }

        // --- movement depends on the learned plan
        strafeTimer -= dt;
        if (strafeTimer <= 0f) {
            float base = arm == CIRCLE ? 0.9f : 0.3f;
            strafeTimer = base + rng.nextFloat() * (1f - 0.4f * d);
            if (rng.nextFloat() < (arm == CIRCLE ? 0.25f : 0.55f)) strafeDir = -strafeDir;
            if (d > 0.45f && Math.abs(targetLateral) > 0.08f && rng.nextFloat() < d) {
                strafeDir = targetLateral > 0f ? -1f : 1f;
            }
        }
        double ideal;
        if ("crystal".equals(cfg.style)) ideal = 4.0;
        else if ("mace".equals(cfg.style)) ideal = 2.2;
        else if (arm == PRESSURE) ideal = 1.9;
        else if (arm == SPACING) ideal = 3.0;
        else ideal = 2.5;
        boolean kiting = d > 0.4f && !cfg.legacy_combat && attackTimer > 0.25f && dist < reach + 0.5
            && !"crystal".equals(cfg.style) && arm != PRESSURE;
        if (retreating || kiting) {
            cmd.backward = true;
        } else if (dist > ideal + 0.3) {
            cmd.forward = true;
            cmd.sprint = true;
        } else if (dist < ideal - 0.6) {
            cmd.backward = true;
        } else if (finish) {
            cmd.forward = true;
            cmd.sprint = true;
        }
        if (dist < 6.0) {
            if (strafeDir > 0f) cmd.right = true; else cmd.left = true;
        }
        wtapTimer = Math.max(0f, wtapTimer - dt);
        if (wtapTimer > 0f) {
            cmd.sprint = false;
            cmd.forward = false;
        }

        // --- jumping: crits for sharp bots
        boolean critStyle = !cfg.legacy_combat && d > 0.4f
            && ("sword".equals(cfg.style) || "axe_shield".equals(cfg.style) || "bedwars_rush".equals(cfg.style));
        boolean wantsCritJump = critStyle && obs.on_ground && jumpCooldown <= 0f && !retreating
            && attackTimer < 0.3f && dist < reach + 0.9;
        if (wantsCritJump) {
            cmd.jump = true;
            jumpCooldown = 0.75f;
        } else if (obs.on_ground && dist < 4.0 && !critStyle && rng.nextFloat() < 0.02f + 0.05f * d) {
            cmd.jump = true;
        }

        // --- combat
        attackTimer -= dt;
        if ("crystal".equals(cfg.style)) {
            // crystals are not simulated client-side: fight with the sword like the other styles
        }
        boolean ready = attackTimer <= 0f && reactionLeft <= 0f;
        if (critStyle && !obs.on_ground && airTime < 0.3f) ready = false;
        boolean canHit = inRange && dist <= reach && aimed;
        if ("axe_shield".equals(cfg.style)) shield(obs, d, dt, canHit && ready, cmd);
        if (ready && canHit && !cmd.use_item) {
            cmd.attack = true;
            attackTimer = nextAttackInterval(d);
            wtapTimer = d > 0.4f ? 0.12f : 0f;
        }
        return cmd;
    }

    private float cooldownSecs() {
        if ("axe_shield".equals(cfg.style)) return 1.0f;
        if ("mace".equals(cfg.style)) return 1.7f;
        return 0.625f;
    }

    private float nextAttackInterval(float d) {
        if (cfg.legacy_combat) {
            float cps = 5f + 8f * d + noise() * 1.5f;
            return 1f / Math.max(3f, Math.min(14f, cps));
        }
        float early = 1f - 0.25f * (1f - d) * rng.nextFloat();
        return cooldownSecs() * early + 0.02f;
    }

    private void shield(Models.Obs obs, float d, float dt, boolean wantsAttack, Models.BotCommand cmd) {
        shieldTimer = Math.max(0f, shieldTimer - dt);
        double dx = obs.target_pos.x - obs.bot_pos.x, dz = obs.target_pos.z - obs.bot_pos.z;
        double dist = Math.sqrt(dx * dx + dz * dz);
        if (shieldTimer > 0f) {
            cmd.use_item = true;
            return;
        }
        if (!wantsAttack && dist < 3.5 && rng.nextFloat() < 0.04f + 0.08f * d) {
            shieldTimer = 0.4f + rng.nextFloat() * 0.5f;
            cmd.use_item = true;
        }
        if (obs.target_blocking && wantsAttack) {
            shieldTimer = 0f;
            cmd.use_item = false;
        }
    }
}
