package dev.frost.mc;

import com.google.gson.JsonObject;
import com.mojang.authlib.GameProfile;
import dev.frost.core.Combat;
import dev.frost.core.Dispatcher;
import dev.frost.core.FrostState;
import dev.frost.core.Knockback;
import dev.frost.core.Models;
import net.minecraft.client.Minecraft;
import net.minecraft.client.multiplayer.ClientLevel;
import net.minecraft.client.player.LocalPlayer;
import net.minecraft.client.player.RemotePlayer;
import net.minecraft.network.chat.Component;
import net.minecraft.world.InteractionHand;
import net.minecraft.world.entity.EquipmentSlot;
import net.minecraft.world.entity.MoverType;
import net.minecraft.world.entity.ai.attributes.Attributes;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.Items;
import net.minecraft.world.phys.EntityHitResult;
import net.minecraft.world.phys.HitResult;
import net.minecraft.world.phys.Vec3;

import java.util.HashMap;
import java.util.Iterator;
import java.util.Map;
import java.util.UUID;

/**
 * Client-side PvP trainer bots. The decision logic runs in the Rust launcher (trainer.rs): every
 * tick we send an observation and apply the latest command we received.
 *
 * The bots are client-only entities: they move, swing, can be hit (knockback/hurt animation) and
 * push the player with vanilla knockback maths, but they do not deal real damage and the server
 * never sees them.
 */
public final class Bots {
    private static final class Bot {
        RemotePlayer entity;
        Models.BotConfig cfg;
        Vec3 velocity = Vec3.ZERO;
        float hp = 20f;
        int invul;      // ticks of damage immunity after being hit
        int respawnIn = -1;
    }

    private static final Map<Integer, Bot> BOTS = new HashMap<>();
    private static ClientLevel levelOfBots;
    private static boolean lastAttack;
    private static int kills;
    private static int deaths;
    private static float lastPlayerHp = 20f;

    private Bots() {}

    public static void tick(Minecraft mc) {
        FrostState st = FrostState.INSTANCE;
        ClientLevel level = mc.level;
        LocalPlayer player = mc.player;

        if (level == null || player == null || level != levelOfBots) {
            BOTS.clear();
            levelOfBots = level;
            st.spawnQueue.clear();
            st.botCommands.clear();
            return;
        }

        if (st.clearBots.getAndSet(false)) {
            for (Bot b : BOTS.values()) b.entity.discard();
            BOTS.clear();
            st.botCommands.clear();
        }
        Models.BotConfig cfg;
        while ((cfg = st.spawnQueue.poll()) != null) {
            // Bots are a training feature: never spawn them on multiplayer servers.
            if (mc.getSingleplayerServer() == null) {
                st.notify("Frost: bots only work in singleplayer");
                continue;
            }
            Arena.setup(mc, cfg);
            spawn(level, player, cfg);
        }

        // count the player's deaths for the score line
        float hpNow = player.getHealth();
        if (lastPlayerHp > 0f && hpNow <= 0f && !BOTS.isEmpty()) {
            deaths++;
            st.notify("Frost: you lost  |  score " + kills + " - " + deaths);
        }
        lastPlayerHp = hpNow;

        boolean attackNow = mc.options.keyAttack.isDown();
        boolean attackPressed = attackNow && !lastAttack;
        lastAttack = attackNow;

        Iterator<Bot> it = BOTS.values().iterator();
        while (it.hasNext()) {
            Bot bot = it.next();
            if (bot.respawnIn >= 0) {
                if (--bot.respawnIn < 0) respawn(level, player, bot);
                continue;
            }
            if (bot.entity.isRemoved()) {
                it.remove();
                continue;
            }
            if (attackPressed) playerHitsBot(mc, player, bot);
            stepBot(mc, player, bot, st);
        }
    }

    private static void spawn(ClientLevel level, LocalPlayer p, Models.BotConfig cfg) {
        GameProfile profile = new GameProfile(
            UUID.nameUUIDFromBytes(("frostbot" + cfg.id).getBytes()), cfg.name);
        RemotePlayer e = new RemotePlayer(level, profile);
        e.setId(-1000 - cfg.id);
        double yaw = Math.toRadians(p.getYRot());
        double[] arenaSpawn = Arena.botSpawn;
        if (arenaSpawn != null) {
            e.setPos(arenaSpawn[0], arenaSpawn[1], arenaSpawn[2]);
            e.setYRot(0f);
        } else {
            e.setPos(p.getX() - Math.sin(yaw) * 4.0, p.getY(), p.getZ() + Math.cos(yaw) * 4.0);
            e.setYRot(p.getYRot() + 180f);
        }
        e.setCustomName(Component.literal(cfg.name));
        e.setCustomNameVisible(true);

        boolean bw = cfg.style.equals("bedwars_rush");
        e.setItemSlot(EquipmentSlot.HEAD, new ItemStack(bw ? Items.LEATHER_HELMET : Items.NETHERITE_HELMET));
        e.setItemSlot(EquipmentSlot.CHEST, new ItemStack(bw ? Items.IRON_CHESTPLATE : Items.NETHERITE_CHESTPLATE));
        e.setItemSlot(EquipmentSlot.LEGS, new ItemStack(bw ? Items.IRON_LEGGINGS : Items.NETHERITE_LEGGINGS));
        e.setItemSlot(EquipmentSlot.FEET, new ItemStack(bw ? Items.IRON_BOOTS : Items.NETHERITE_BOOTS));
        switch (cfg.style) {
            case "axe_shield" -> {
                e.setItemSlot(EquipmentSlot.MAINHAND, new ItemStack(Items.NETHERITE_AXE));
                e.setItemSlot(EquipmentSlot.OFFHAND, new ItemStack(Items.SHIELD));
            }
            case "mace" -> e.setItemSlot(EquipmentSlot.MAINHAND, new ItemStack(Items.MACE));
            case "bedwars_rush" -> {
                e.setItemSlot(EquipmentSlot.MAINHAND, new ItemStack(Items.IRON_SWORD));
                e.setItemSlot(EquipmentSlot.OFFHAND, new ItemStack(Items.COBBLESTONE, 32));
            }
            case "crystal" -> e.setItemSlot(EquipmentSlot.MAINHAND, new ItemStack(Items.END_CRYSTAL));
            default -> e.setItemSlot(EquipmentSlot.MAINHAND, new ItemStack(Items.NETHERITE_SWORD));
        }

        level.addEntity(e);
        Bot bot = new Bot();
        bot.entity = e;
        bot.cfg = cfg;
        BOTS.put(cfg.id, bot);
    }

    private static void respawn(ClientLevel level, LocalPlayer p, Bot bot) {
        double[] a = Arena.botSpawn;
        RemotePlayer e = bot.entity;
        if (e.isRemoved()) {
            spawn(level, p, bot.cfg);
            return;
        }
        if (a != null) e.setPos(a[0], a[1], a[2]);
        else e.setPos(p.getX() + 4, p.getY(), p.getZ() + 4);
        bot.hp = 20f;
        bot.invul = 20;
        bot.velocity = Vec3.ZERO;
        e.setInvisible(false);
        nameTag(bot);
    }

    private static void nameTag(Bot bot) {
        bot.entity.setCustomName(Component.literal(bot.cfg.name + " \u00a7c" + (int) Math.ceil(bot.hp) + "\u2764"));
    }

    private static void stepBot(Minecraft mc, LocalPlayer player, Bot bot, FrostState st) {
        RemotePlayer e = bot.entity;
        if (bot.invul > 0) bot.invul--;
        if (bot.hp < 20f) {
            bot.hp = Math.min(20f, bot.hp + Combat.regenPerSecond() / 20f);
            if (bot.entity.tickCount % 20 == 0) nameTag(bot);
        }

        // 1) tell the Rust brain what the bot sees
        if (FrostMc.link() != null && FrostMc.link().connected()) {
            Models.Obs obs = new Models.Obs();
            obs.bot_pos = new Models.Vec3(e.getX(), e.getY(), e.getZ());
            obs.bot_yaw = e.getYRot();
            obs.bot_pitch = e.getXRot();
            obs.on_ground = e.onGround();
            obs.bot_health = bot.hp;
            Vec3 pm = player.getDeltaMovement();
            obs.target_pos = new Models.Vec3(player.getX(), player.getY(), player.getZ());
            obs.target_vel = new Models.Vec3(pm.x, pm.y, pm.z);
            obs.target_blocking = player.isBlocking();
            obs.target_health = player.getHealth();
            JsonObject msg = new JsonObject();
            msg.addProperty("type", "bot_obs");
            msg.addProperty("id", bot.cfg.id);
            msg.add("obs", Dispatcher.gson().toJsonTree(obs));
            FrostMc.link().send(msg);
        }

        // 2) apply the newest command
        Models.BotCommand c = st.botCommands.get(bot.cfg.id);
        double ix = 0;
        double iz = 0;
        if (c != null) {
            float yaw = e.getYRot() + c.d_yaw;
            float pitch = Math.max(-90f, Math.min(90f, e.getXRot() + c.d_pitch));
            e.setYRot(yaw);
            e.setXRot(pitch);
            e.setYHeadRot(yaw);
            e.setYBodyRot(yaw);

            float fwd = (c.forward ? 1f : 0f) - (c.backward ? 1f : 0f);
            float str = (c.left ? 1f : 0f) - (c.right ? 1f : 0f);
            double rad = Math.toRadians(yaw);
            double dx = -Math.sin(rad) * fwd + Math.cos(rad) * str;
            double dz = Math.cos(rad) * fwd + Math.sin(rad) * str;
            double len = Math.sqrt(dx * dx + dz * dz);
            if (len > 1e-6) {
                double speed = c.sprint ? 0.28 : 0.216;
                ix = dx / len * speed;
                iz = dz / len * speed;
            }
            if (c.jump && e.onGround()) {
                bot.velocity = new Vec3(bot.velocity.x, 0.42, bot.velocity.z);
            }
            if (c.attack) botAttacks(mc, player, bot, c.sprint);
        }

        // 3) simple vanilla-like physics
        double vx = bot.velocity.x * 0.6 + ix * 0.4;
        double vz = bot.velocity.z * 0.6 + iz * 0.4;
        double vy = e.onGround() && bot.velocity.y <= 0 ? 0 : (bot.velocity.y - 0.08) * 0.98;
        bot.velocity = new Vec3(vx, vy, vz);
        e.move(MoverType.SELF, bot.velocity);
        if (e.onGround() && bot.velocity.y < 0) bot.velocity = new Vec3(bot.velocity.x, 0, bot.velocity.z);
    }

    private static void botAttacks(Minecraft mc, LocalPlayer player, Bot bot, boolean sprint) {
        RemotePlayer e = bot.entity;
        e.swing(InteractionHand.MAIN_HAND);
        double dx = player.getX() - e.getX();
        double dz = player.getZ() - e.getZ();
        double reach = Math.sqrt(dx * dx + dz * dz + Math.pow(player.getY() - e.getY(), 2));
        if (reach > 3.4 || player.hurtTime > 0 || player.getHealth() <= 0f || player.isCreative()) return;
        boolean legacy = bot.cfg.legacy_combat;
        float dmg = Combat.weaponDamage(bot.cfg.style, legacy);
        if (Combat.isCrit(e.onGround(), bot.velocity.y, sprint, legacy)) dmg *= 1.5f;
        boolean blocked = player.isBlocking();
        if (!blocked) {
            damagePlayer(mc, player, dmg);
            Vec3 pv = player.getDeltaMovement();
            double[] v = Knockback.apply(FrostState.INSTANCE.kbProfile, pv.x, pv.y, pv.z, player.onGround(),
                dx, dz, e.getYRot(), sprint, 0);
            player.setDeltaMovement(v[0], v[1], v[2]);
            player.animateHurt(e.getYRot());
            FrostMc.sendKnockbackEvent(false);
        }
    }

    /** Real damage through the integrated server (armor, totems and death work like vanilla). */
    private static void damagePlayer(Minecraft mc, LocalPlayer player, float amount) {
        net.minecraft.server.MinecraftServer server = mc.getSingleplayerServer();
        if (server == null) return;
        final String cmd = String.format(java.util.Locale.ROOT, "damage %s %.2f minecraft:mob_attack", player.getUUID(), amount);
        server.execute(() -> {
            try {
                server.getCommands().performPrefixedCommand(server.createCommandSourceStack().withSuppressedOutput(), cmd);
            } catch (Throwable ignored) {
            }
        });
    }

    private static void playerHitsBot(Minecraft mc, LocalPlayer player, Bot bot) {
        HitResult hr = mc.hitResult;
        if (!(hr instanceof EntityHitResult ehr) || ehr.getEntity() != bot.entity) return;
        if (player.getEyePosition().distanceTo(ehr.getLocation()) > 3.1) return;
        if (bot.invul > 0) return;
        boolean legacy = bot.cfg.legacy_combat;
        float scale = legacy ? 1f : player.getAttackStrengthScale(0.5f);
        float dmg = (float) player.getAttributeValue(Attributes.ATTACK_DAMAGE) * (0.2f + scale * scale * 0.8f);
        if (scale > 0.9f && Combat.isCrit(player.onGround(), player.getDeltaMovement().y, player.isSprinting(), legacy)) dmg *= 1.5f;
        dmg = Combat.afterArmor(dmg, Combat.armorPoints(bot.cfg.style), Combat.armorToughness(bot.cfg.style, legacy), legacy);
        bot.hp = Combat.hurt(bot.hp, dmg);
        bot.invul = 10;
        double dx = bot.entity.getX() - player.getX();
        double dz = bot.entity.getZ() - player.getZ();
        double[] v = Knockback.apply(FrostState.INSTANCE.kbProfile, bot.velocity.x, bot.velocity.y, bot.velocity.z,
            bot.entity.onGround(), dx, dz, player.getYRot(), player.isSprinting(), 0);
        bot.velocity = new Vec3(v[0], v[1], v[2]);
        bot.entity.animateHurt(player.getYRot());
        FrostMc.sendKnockbackEvent(true);
        if (bot.hp <= 0f) {
            kills++;
            FrostState.INSTANCE.notify("Frost: " + bot.cfg.name + " defeated  |  score " + kills + " - " + deaths);
            bot.entity.setPos(bot.entity.getX(), -200, bot.entity.getZ());
            bot.respawnIn = 40;
        } else {
            nameTag(bot);
        }
    }
}
