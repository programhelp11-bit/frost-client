package dev.frost.forge;

import com.google.gson.JsonObject;
import com.mojang.authlib.GameProfile;
import dev.frost.core.Dispatcher;
import dev.frost.core.FrostState;
import dev.frost.core.Knockback;
import dev.frost.core.Models;
import net.minecraft.client.Minecraft;
import net.minecraft.client.entity.EntityPlayerSP;
import net.minecraft.client.multiplayer.WorldClient;
import net.minecraft.client.entity.EntityOtherPlayerMP;
import net.minecraft.init.Items;
import net.minecraft.item.Item;
import net.minecraft.item.ItemStack;
import net.minecraft.util.MovingObjectPosition;

import java.util.HashMap;
import java.util.Iterator;
import java.util.Map;
import java.util.UUID;

/**
 * Client-side PvP trainer bots (decision logic runs in the Rust launcher). Bots move, swing and
 * can be hit with vanilla 1.8 knockback maths, but never deal real damage and the server never
 * sees them.
 */
public final class Bots {
    private static final class Bot {
        EntityOtherPlayerMP entity;
        Models.BotConfig cfg;
        double vx, vy, vz;
    }

    private static final Map<Integer, Bot> BOTS = new HashMap<Integer, Bot>();
    private static WorldClient worldOfBots;
    private static boolean lastAttack;

    private Bots() {}

    public static void tick(Minecraft mc) {
        FrostState st = FrostState.INSTANCE;
        WorldClient world = mc.theWorld;
        EntityPlayerSP player = mc.thePlayer;

        if (world == null || player == null || world != worldOfBots) {
            BOTS.clear();
            worldOfBots = world;
            st.spawnQueue.clear();
            st.botCommands.clear();
            return;
        }

        if (st.clearBots.getAndSet(false)) {
            for (Bot b : BOTS.values()) world.removeEntityFromWorld(b.entity.getEntityId());
            BOTS.clear();
            st.botCommands.clear();
        }
        Models.BotConfig cfg;
        while ((cfg = st.spawnQueue.poll()) != null) {
            // Bots are a training feature: never spawn them on multiplayer servers.
            if (!mc.isSingleplayer()) {
                player.addChatMessage(new net.minecraft.util.ChatComponentText("\u00a7bFrost\u00a7r: bots only work in singleplayer"));
                continue;
            }
            Arena.setup(mc, cfg);
            spawn(world, player, cfg);
        }

        boolean attackNow = mc.gameSettings.keyBindAttack.isKeyDown();
        boolean attackPressed = attackNow && !lastAttack;
        lastAttack = attackNow;

        Iterator<Bot> it = BOTS.values().iterator();
        while (it.hasNext()) {
            Bot bot = it.next();
            if (bot.entity.isDead) {
                it.remove();
                continue;
            }
            if (attackPressed) playerHitsBot(mc, player, bot);
            stepBot(player, bot, st);
        }
    }

    private static void spawn(WorldClient world, EntityPlayerSP p, Models.BotConfig cfg) {
        GameProfile profile = new GameProfile(UUID.nameUUIDFromBytes(("frostbot" + cfg.id).getBytes()), cfg.name);
        EntityOtherPlayerMP e = new EntityOtherPlayerMP(world, profile);
        e.setEntityId(-1000 - cfg.id);
        double yaw = Math.toRadians(p.rotationYaw);
        double[] arenaSpawn = Arena.botSpawn;
        if (arenaSpawn != null) {
            e.setLocationAndAngles(arenaSpawn[0], arenaSpawn[1], arenaSpawn[2], 0f, 0f);
        } else {
            e.setLocationAndAngles(p.posX - Math.sin(yaw) * 4.0, p.posY, p.posZ + Math.cos(yaw) * 4.0, p.rotationYaw + 180f, 0f);
        }
        e.rotationYawHead = e.rotationYaw;
        e.setCustomNameTag(cfg.name);
        e.setAlwaysRenderNameTag(true);

        boolean bw = "bedwars_rush".equals(cfg.style);
        e.setCurrentItemOrArmor(4, new ItemStack(bw ? Items.leather_helmet : Items.diamond_helmet));
        e.setCurrentItemOrArmor(3, new ItemStack(bw ? Items.iron_chestplate : Items.diamond_chestplate));
        e.setCurrentItemOrArmor(2, new ItemStack(bw ? Items.iron_leggings : Items.diamond_leggings));
        e.setCurrentItemOrArmor(1, new ItemStack(bw ? Items.iron_boots : Items.diamond_boots));
        Item held = Items.diamond_sword;
        if ("axe_shield".equals(cfg.style)) held = Items.diamond_axe;
        else if (bw) held = Items.iron_sword;
        e.setCurrentItemOrArmor(0, new ItemStack(held));

        world.addEntityToWorld(e.getEntityId(), e);
        Bot bot = new Bot();
        bot.entity = e;
        bot.cfg = cfg;
        BOTS.put(cfg.id, bot);
    }

    private static void stepBot(EntityPlayerSP player, Bot bot, FrostState st) {
        EntityOtherPlayerMP e = bot.entity;

        if (FrostForge.link() != null && FrostForge.link().connected()) {
            Models.Obs obs = new Models.Obs();
            obs.bot_pos = new Models.Vec3(e.posX, e.posY, e.posZ);
            obs.bot_yaw = e.rotationYaw;
            obs.bot_pitch = e.rotationPitch;
            obs.on_ground = e.onGround;
            obs.target_pos = new Models.Vec3(player.posX, player.posY, player.posZ);
            obs.target_vel = new Models.Vec3(player.motionX, player.motionY, player.motionZ);
            obs.target_blocking = player.isBlocking();
            obs.target_health = player.getHealth();
            JsonObject msg = new JsonObject();
            msg.addProperty("type", "bot_obs");
            msg.addProperty("id", bot.cfg.id);
            msg.add("obs", Dispatcher.gson().toJsonTree(obs));
            FrostForge.link().send(msg);
        }

        Models.BotCommand c = st.botCommands.get(bot.cfg.id);
        double ix = 0;
        double iz = 0;
        if (c != null) {
            float yaw = e.rotationYaw + c.d_yaw;
            float pitch = Math.max(-90f, Math.min(90f, e.rotationPitch + c.d_pitch));
            e.rotationYaw = yaw;
            e.rotationPitch = pitch;
            e.rotationYawHead = yaw;
            e.renderYawOffset = yaw;

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
            if (c.jump && e.onGround) bot.vy = 0.42;
            if (c.attack) botAttacks(player, bot, c.sprint);
        }

        bot.vx = bot.vx * 0.6 + ix * 0.4;
        bot.vz = bot.vz * 0.6 + iz * 0.4;
        bot.vy = e.onGround && bot.vy <= 0 ? 0 : (bot.vy - 0.08) * 0.98;
        e.moveEntity(bot.vx, bot.vy, bot.vz);
        if (e.onGround && bot.vy < 0) bot.vy = 0;
    }

    private static void botAttacks(EntityPlayerSP player, Bot bot, boolean sprint) {
        EntityOtherPlayerMP e = bot.entity;
        e.swingItem();
        double dx = player.posX - e.posX;
        double dz = player.posZ - e.posZ;
        double reach = Math.sqrt(dx * dx + dz * dz + Math.pow(player.posY - e.posY, 2));
        if (reach > 3.4) return;
        double[] v = Knockback.apply(FrostState.INSTANCE.kbProfile, player.motionX, player.motionY, player.motionZ,
            player.onGround, dx, dz, e.rotationYaw, sprint, 0);
        player.motionX = v[0];
        player.motionY = v[1];
        player.motionZ = v[2];
        player.performHurtAnimation();
        FrostForge.sendKnockbackEvent(false);
    }

    private static void playerHitsBot(Minecraft mc, EntityPlayerSP player, Bot bot) {
        MovingObjectPosition mop = mc.objectMouseOver;
        if (mop == null || mop.typeOfHit != MovingObjectPosition.MovingObjectType.ENTITY || mop.entityHit != bot.entity) return;
        if (player.getPositionEyes(1.0F).distanceTo(mop.hitVec) > 3.1) return;
        double dx = bot.entity.posX - player.posX;
        double dz = bot.entity.posZ - player.posZ;
        double[] v = Knockback.apply(FrostState.INSTANCE.kbProfile, bot.vx, bot.vy, bot.vz,
            bot.entity.onGround, dx, dz, player.rotationYaw, player.isSprinting(), 0);
        bot.vx = v[0];
        bot.vy = v[1];
        bot.vz = v[2];
        bot.entity.performHurtAnimation();
        FrostForge.sendKnockbackEvent(true);
    }
}
