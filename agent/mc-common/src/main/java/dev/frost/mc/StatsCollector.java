package dev.frost.mc;

import dev.frost.core.Cps;
import dev.frost.core.Models;
import net.minecraft.client.Minecraft;
import net.minecraft.client.multiplayer.PlayerInfo;
import net.minecraft.client.player.LocalPlayer;
import net.minecraft.network.chat.Component;
import net.minecraft.world.effect.MobEffectInstance;
import net.minecraft.world.entity.EquipmentSlot;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.phys.EntityHitResult;
import net.minecraft.world.phys.HitResult;
import org.lwjgl.glfw.GLFW;

/** Collects everything the HUD and the launcher need to know about the running game. */
public final class StatsCollector {
    private static final Cps LEFT = new Cps();
    private static final Cps RIGHT = new Cps();
    private static boolean lastL;
    private static boolean lastR;
    private static int frames;
    private static long lastSecond = System.nanoTime();
    private static int fps;
    private static volatile Float lastReach;

    public static volatile Models.Stats latest = new Models.Stats();

    private StatsCollector() {}

    /** Called once per rendered frame (render thread): FPS + frame-accurate click detection. */
    public static void frame(Minecraft mc) {
        frames++;
        long now = System.nanoTime();
        if (now - lastSecond >= 1_000_000_000L) {
            fps = frames;
            frames = 0;
            lastSecond = now;
        }
        long window = GLFW.glfwGetCurrentContext();
        if (window == 0L || mc.screen != null || mc.player == null) {
            lastL = false;
            lastR = false;
            return;
        }
        boolean l = GLFW.glfwGetMouseButton(window, GLFW.GLFW_MOUSE_BUTTON_LEFT) == GLFW.GLFW_PRESS;
        boolean r = GLFW.glfwGetMouseButton(window, GLFW.GLFW_MOUSE_BUTTON_RIGHT) == GLFW.GLFW_PRESS;
        if (l && !lastL) {
            LEFT.click();
            recordReach(mc);
        }
        if (r && !lastR) {
            RIGHT.click();
        }
        lastL = l;
        lastR = r;
    }

    private static void recordReach(Minecraft mc) {
        HitResult hr = mc.hitResult;
        if (hr instanceof EntityHitResult ehr && mc.player != null) {
            lastReach = (float) mc.player.getEyePosition().distanceTo(ehr.getLocation());
        }
    }

    /** Called every client tick. */
    public static void tick(Minecraft mc) {
        LocalPlayer p = mc.player;
        if (p == null || mc.level == null) return;

        Models.Stats s = new Models.Stats();
        s.fps = fps;
        s.cps_left = LEFT.count();
        s.cps_right = RIGHT.count();
        s.yaw = p.getYRot();
        s.pos = new double[] {p.getX(), p.getY(), p.getZ()};
        s.last_reach = lastReach;

        s.keys.w = mc.options.keyUp.isDown();
        s.keys.a = mc.options.keyLeft.isDown();
        s.keys.s = mc.options.keyDown.isDown();
        s.keys.d = mc.options.keyRight.isDown();
        s.keys.space = mc.options.keyJump.isDown();
        s.keys.lmb = mc.options.keyAttack.isDown();
        s.keys.rmb = mc.options.keyUse.isDown();

        if (mc.getConnection() != null) {
            PlayerInfo info = mc.getConnection().getPlayerInfo(p.getUUID());
            if (info != null) s.ping_ms = info.getLatency();
        }

        EquipmentSlot[] slots = {EquipmentSlot.HEAD, EquipmentSlot.CHEST, EquipmentSlot.LEGS, EquipmentSlot.FEET};
        for (int i = 0; i < slots.length; i++) {
            ItemStack st = p.getItemBySlot(slots[i]);
            if (st.isEmpty()) continue;
            Models.Armor a = new Models.Armor();
            a.slot = i;
            a.name = st.getHoverName().getString();
            a.durability_pct = st.isDamageableItem()
                ? 1f - (float) st.getDamageValue() / Math.max(1, st.getMaxDamage())
                : 1f;
            s.armor.add(a);
        }

        for (MobEffectInstance e : p.getActiveEffects()) {
            Models.Potion po = new Models.Potion();
            po.name = Component.translatable(e.getDescriptionId()).getString();
            po.amplifier = e.getAmplifier();
            po.remaining_secs = Math.max(0, e.getDuration() / 20);
            s.potions.add(po);
        }

        s.biome = mc.level.getBiome(p.blockPosition()).getRegisteredName();
        latest = s;
    }
}
