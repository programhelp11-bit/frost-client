package dev.frost.mc;

import dev.frost.core.FrostState;
import dev.frost.core.Models;
import net.minecraft.client.CloudStatus;
import net.minecraft.client.Minecraft;
import net.minecraft.client.OptionInstance;
import net.minecraft.server.level.ParticleStatus;
import net.minecraft.client.player.LocalPlayer;
import net.minecraft.world.effect.MobEffectInstance;
import net.minecraft.world.effect.MobEffects;
import org.lwjgl.glfw.GLFW;

import java.util.function.UnaryOperator;

/**
 * Quality-of-life features that only need public option / player APIs (no mixins):
 * fullbright (client-side night vision), zoom, dynamic FOV, minimal view bobbing, FPS boost.
 */
public final class QolApplier {
    private static boolean nightVisionByUs;
    private static boolean zooming;
    private static int savedFov = 70;
    private static Boolean lastDynamicFov;
    private static Boolean lastMinimalView;
    private static boolean boostApplied;
    private static int savedRenderDistance = -1;

    /** Remembers the user's own value of an option while we override it. */
    private static final class Saved<T> {
        boolean active;
        T orig;
    }

    private static final Saved<Boolean> SHADOWS = new Saved<>();
    private static final Saved<Integer> BLEND = new Saved<>();
    private static final Saved<Integer> SIM = new Saved<>();
    private static final Saved<Integer> BLUR = new Saved<>();
    private static final Saved<CloudStatus> CLOUDS = new Saved<>();

    private QolApplier() {}

    /** Applies (or restores) one option; only touches the option when its value actually differs. */
    private static <T> void force(OptionInstance<T> opt, Saved<T> s, boolean want, UnaryOperator<T> target) {
        if (want) {
            if (!s.active) {
                s.orig = opt.get();
                s.active = true;
            }
            T v = target.apply(s.orig);
            if (!v.equals(opt.get())) opt.set(v);
        } else if (s.active) {
            opt.set(s.orig);
            s.active = false;
        }
    }

    private static void performance(Minecraft mc, Models.Qol q) {
        force(mc.options.entityShadows(), SHADOWS, q.no_entity_shadows, v -> Boolean.FALSE);
        force(mc.options.biomeBlendRadius(), BLEND, q.low_biome_blend, v -> Math.min(v, 1));
        final int cap = Math.max(2, q.sim_distance_cap);
        force(mc.options.simulationDistance(), SIM, q.cap_sim_distance, v -> Math.min(v, cap));
        force(mc.options.menuBackgroundBlurriness(), BLUR, q.no_menu_blur, v -> 0);
        force(mc.options.cloudStatus(), CLOUDS, q.no_clouds, v -> CloudStatus.OFF);
    }

    public static void tick(Minecraft mc) {
        LocalPlayer p = mc.player;
        if (p == null) {
            zooming = false;
            return;
        }
        Models.Qol q = FrostState.INSTANCE.qol;

        performance(mc, q);

        // --- fullbright: permanent, invisible, client-side night vision -----------------------------
        if (q.fullbright) {
            if (!p.hasEffect(MobEffects.NIGHT_VISION)) {
                p.addEffect(new MobEffectInstance(MobEffects.NIGHT_VISION, 1_000_000, 0, false, false, false));
                nightVisionByUs = true;
            }
        } else if (nightVisionByUs) {
            p.removeEffect(MobEffects.NIGHT_VISION);
            nightVisionByUs = false;
        }

        // --- zoom (hold the configured key) --------------------------------------------------------
        boolean wantZoom = false;
        if (q.zoom_enabled && mc.mouseHandler.isMouseGrabbed()) {
            long window = GLFW.glfwGetCurrentContext();
            int key = glfwKey(q.zoom_key);
            wantZoom = window != 0L && key > 0 && GLFW.glfwGetKey(window, key) == GLFW.GLFW_PRESS;
        }
        if (wantZoom && !zooming) {
            savedFov = mc.options.fov().get();
            zooming = true;
        }
        if (zooming) {
            mc.options.fov().set(wantZoom ? Math.max(30, Math.round(q.zoom_fov)) : savedFov);
            if (!wantZoom) zooming = false;
        }

        // --- dynamic FOV (sprint / speed FOV change) ------------------------------------------------
        if (lastDynamicFov == null || lastDynamicFov != q.dynamic_fov) {
            mc.options.fovEffectScale().set(q.dynamic_fov ? 1.0 : 0.0);
            lastDynamicFov = q.dynamic_fov;
        }

        // --- minimal viewmodel: no hand bobbing -----------------------------------------------------
        if (lastMinimalView == null || lastMinimalView != q.minimal_viewmodel) {
            mc.options.bobView().set(!q.minimal_viewmodel);
            lastMinimalView = q.minimal_viewmodel;
        }

        // --- FPS boost / chunk optimizer --------------------------------------------------------------
        boolean boost = q.fps_boost || q.chunk_optimizer;
        if (boost && !boostApplied) {
            if (q.chunk_optimizer) {
                savedRenderDistance = mc.options.renderDistance().get();
                int cap = Math.max(2, q.chunk_render_cap);
                if (savedRenderDistance > cap) mc.options.renderDistance().set(cap);
            }
            if (q.fps_boost) {
                mc.options.particles().set(ParticleStatus.DECREASED);
                mc.options.entityDistanceScaling().set(Math.min(0.75, mc.options.entityDistanceScaling().get()));
            }
            boostApplied = true;
        } else if (!boost && boostApplied) {
            if (savedRenderDistance > 0) mc.options.renderDistance().set(savedRenderDistance);
            boostApplied = false;
        }
    }

    /** "C" -> GLFW_KEY_C. Only single letters/digits are supported. */
    static int glfwKey(String name) {
        if (name == null || name.isEmpty()) return -1;
        char c = Character.toUpperCase(name.charAt(0));
        if (c >= 'A' && c <= 'Z') return GLFW.GLFW_KEY_A + (c - 'A');
        if (c >= '0' && c <= '9') return GLFW.GLFW_KEY_0 + (c - '0');
        return -1;
    }
}
