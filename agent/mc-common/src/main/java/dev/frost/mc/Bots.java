package dev.frost.mc;

import com.google.gson.JsonObject;
import com.mojang.authlib.GameProfile;
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
    }

    private static final Map<Integer, Bot> BOTS = new HashMap<>();
    private static ClientLevel levelOfBots;
    private static boolean lastAttack;

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

        boolean attackNow = mc.options.keyAttack.isDown();
        boolean attackPressed = attackNow && !lastAttack;
        lastAttack = attackNow;

        Iterator<Bot> it = BOTS.values().iterator();
        while (it.hasNext()) {
            Bot bot = it.next();
            if (bot.entity.isRemoved()) {
                it.remove();
                continue;
            }
            if (attackPressed) playerHitsBot(mc, player, bot);
            stepBot(player, bot, st);
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
                e.setItemSlot(EquipmentSlot.OFFHAND, new ItemStack(Items.WHITE_WOOL, 32));
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

    private static void stepBot(LocalPlayer player, Bot bot, FrostState st) {
        RemotePlayer e = bot.entity;

        // 1) tell the Rust brain what the bot sees
        if (FrostMc.link() != null && FrostMc.link().connected()) {
            Models.Obs obs = new Models.Obs();
            obs.bot_pos = new Models.Vec3(e.getX(), e.getY(), e.getZ());
            obs.bot_yaw = e.getYRot();
            obs.bot_pitch = e.getXRot();
            obs.on_ground = e.onGround();
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
            if (c.attack) botAttacks(player, bot, c.sprint);
        }

        // 3) simple vanilla-like physics
        double vx = bot.velocity.x * 0.6 + ix * 0.4;
        double vz = bot.velocity.z * 0.6 + iz * 0.4;
        double vy = e.onGround() && bot.velocity.y <= 0 ? 0 : (bot.velocity.y - 0.08) * 0.98;
        bot.velocity = new Vec3(vx, vy, vz);
        e.move(MoverType.SELF, bot.velocity);
        if (e.onGround() && bot.velocity.y < 0) bot.velocity = new Vec3(bot.velocity.x, 0, bot.velocity.z);
    }

    private static void botAttacks(LocalPlayer player, Bot bot, boolean sprint) {
        RemotePlayer e = bot.entity;
        e.swing(InteractionHand.MAIN_HAND);
        double dx = player.getX() - e.getX();
        double dz = player.getZ() - e.getZ();
        double reach = Math.sqrt(dx * dx + dz * dz + Math.pow(player.getY() - e.getY(), 2));
        if (reach > 3.4) return;
        Vec3 pv = player.getDeltaMovement();
        double[] v = Knockback.apply(FrostState.INSTANCE.kbProfile, pv.x, pv.y, pv.z, player.onGround(),
            dx, dz, e.getYRot(), sprint, 0);
        player.setDeltaMovement(v[0], v[1], v[2]);
        player.animateHurt(e.getYRot());
        FrostMc.sendKnockbackEvent(false);
    }

    private static void playerHitsBot(Minecraft mc, LocalPlayer player, Bot bot) {
        HitResult hr = mc.hitResult;
        if (!(hr instanceof EntityHitResult ehr) || ehr.getEntity() != bot.entity) return;
        if (player.getEyePosition().distanceTo(ehr.getLocation()) > 3.1) return;
        double dx = bot.entity.getX() - player.getX();
        double dz = bot.entity.getZ() - player.getZ();
        double[] v = Knockback.apply(FrostState.INSTANCE.kbProfile, bot.velocity.x, bot.velocity.y, bot.velocity.z,
            bot.entity.onGround(), dx, dz, player.getYRot(), player.isSprinting(), 0);
        bot.velocity = new Vec3(v[0], v[1], v[2]);
        bot.entity.animateHurt(player.getYRot());
        FrostMc.sendKnockbackEvent(true);
    }
}
