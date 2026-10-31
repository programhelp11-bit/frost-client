package dev.frost.forge;

import dev.frost.core.FrostState;
import dev.frost.core.Models;
import net.minecraft.client.Minecraft;
import net.minecraftforge.client.event.FOVUpdateEvent;
import org.lwjgl.input.Keyboard;

/** Quality-of-life features that only need public game settings (no coremod / mixin). */
public final class Qol {
    private static boolean fullbrightApplied;
    private static float savedGamma = 1f;
    private static boolean zooming;
    private static boolean boostApplied;
    private static int savedRenderDistance = -1;
    private static int savedParticles = 0;
    private static Boolean lastBob;

    private Qol() {}

    public static void tick(Minecraft mc) {
        if (mc.thePlayer == null) {
            zooming = false;
            return;
        }
        Models.Qol q = FrostState.INSTANCE.qol;

        if (q.fullbright && !fullbrightApplied) {
            savedGamma = mc.gameSettings.gammaSetting;
            mc.gameSettings.gammaSetting = 1000f;
            fullbrightApplied = true;
        } else if (!q.fullbright && fullbrightApplied) {
            mc.gameSettings.gammaSetting = savedGamma;
            fullbrightApplied = false;
        }

        zooming = false;
        if (q.zoom_enabled && mc.currentScreen == null && q.zoom_key != null && !q.zoom_key.isEmpty()) {
            int key = Keyboard.getKeyIndex(q.zoom_key.toUpperCase());
            zooming = key > 0 && Keyboard.isKeyDown(key);
        }

        if (lastBob == null || lastBob != !q.minimal_viewmodel) {
            mc.gameSettings.viewBobbing = !q.minimal_viewmodel;
            lastBob = !q.minimal_viewmodel;
        }

        boolean boost = q.fps_boost || q.chunk_optimizer;
        if (boost && !boostApplied) {
            if (q.chunk_optimizer) {
                savedRenderDistance = mc.gameSettings.renderDistanceChunks;
                int cap = Math.max(2, q.chunk_render_cap);
                if (savedRenderDistance > cap) mc.gameSettings.renderDistanceChunks = cap;
            }
            if (q.fps_boost) {
                savedParticles = mc.gameSettings.particleSetting;
                mc.gameSettings.particleSetting = 1;
            }
            boostApplied = true;
        } else if (!boost && boostApplied) {
            if (savedRenderDistance > 0) mc.gameSettings.renderDistanceChunks = savedRenderDistance;
            mc.gameSettings.particleSetting = savedParticles;
            boostApplied = false;
        }
    }

    public static void fov(FOVUpdateEvent e) {
        Models.Qol q = FrostState.INSTANCE.qol;
        Minecraft mc = Minecraft.getMinecraft();
        if (zooming) {
            e.newfov = e.fov * (Math.max(10f, q.zoom_fov) / Math.max(30f, mc.gameSettings.fovSetting));
        } else if (!q.dynamic_fov) {
            e.newfov = 1.0f;
        }
    }
}
