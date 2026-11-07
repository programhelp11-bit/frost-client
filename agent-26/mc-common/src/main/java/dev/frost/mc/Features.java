package dev.frost.mc;

import com.mojang.blaze3d.platform.NativeImage;
import dev.frost.core.BwScore;
import dev.frost.core.FrostState;
import dev.frost.core.Models;
import dev.frost.core.SoundRadar;
import dev.frost.core.Spotify;
import net.minecraft.client.DeltaTracker;
import net.minecraft.client.Minecraft;
import net.minecraft.client.player.AbstractClientPlayer;
import net.minecraft.client.player.LocalPlayer;
import net.minecraft.network.chat.Component;
import net.minecraft.network.chat.TextColor;
import net.minecraft.server.level.ParticleStatus;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.item.PrimedTnt;
import net.minecraft.world.phys.AABB;
import net.minecraft.world.phys.Vec3;
import org.lwjgl.glfw.GLFW;

import java.awt.image.BufferedImage;
import java.io.File;
import java.lang.reflect.Method;
import java.util.ArrayList;
import java.util.List;
import java.util.function.Consumer;

/** Glue between the game and the shared Frost features: camera, BedWars scoreboard, TNT, sound radar, clips, hotkeys. */
public final class Features {
    private static int ticks;
    private static boolean spotifyStarted;
    private static boolean captureBroken;
    private static volatile boolean capturing;
    private static final boolean[] KEY = new boolean[8];

    private Features() {}

    /** Called every frame from the HUD: records the camera used by the AR overlays. */
    public static void updateView(Minecraft mc, DeltaTracker dt) {
        LocalPlayer p = mc.player;
        if (p == null) return;
        float pt = dt.getGameTimeDeltaPartialTick(false);
        Vec3 eye = p.getEyePosition(pt);
        Models.View v = FrostState.INSTANCE.view;
        v.x = eye.x;
        v.y = eye.y;
        v.z = eye.z;
        v.yaw = p.getViewYRot(pt);
        v.pitch = p.getViewXRot(pt);
        v.fov = mc.options.fov().get();
        v.w = mc.getWindow().getGuiScaledWidth();
        v.h = mc.getWindow().getGuiScaledHeight();
        v.valid = true;
    }

    public static void tick(Minecraft mc) {
        FrostState st = FrostState.INSTANCE;
        ticks++;
        if (!spotifyStarted) {
            spotifyStarted = true;
            Spotify.start(st, new Runnable() {
                public void run() {
                    FrostMc.HOST.settingsChanged();
                }
            });
        }
        hotkeys(mc, st);
        if (mc.player == null || mc.level == null) {
            st.view.valid = false;
            return;
        }
        try {
            particles(mc, st);
            if (ticks % 10 == 0) scoreboard(mc, st);
            tnt(mc, st);
            if (st.fx.sound_radar && ticks % 2 == 0) radar(mc, st);
            clip(mc, st);
        } catch (Throwable t) {
            // the features must never take the game down
        }
    }

    // --- particle limiter ---------------------------------------------------------------------------
    private static void particles(Minecraft mc, FrostState st) {
        int limit = st.fx.particle_limit;
        if (limit >= 100 || st.qol.fps_boost || st.qol.chunk_optimizer) return;
        ParticleStatus want = limit >= 67 ? ParticleStatus.ALL : (limit >= 34 ? ParticleStatus.DECREASED : ParticleStatus.MINIMAL);
        if (mc.options.particles().get() != want) mc.options.particles().set(want);
    }

    // --- BedWars scoreboard -------------------------------------------------------------------------
    private static void scoreboard(Minecraft mc, FrostState st) {
        try {
            net.minecraft.world.scores.Scoreboard sb = mc.level.getScoreboard();
            net.minecraft.world.scores.Objective obj = sb.getDisplayObjective(net.minecraft.world.scores.DisplaySlot.SIDEBAR);
            if (obj == null) {
                st.bw = null;
                return;
            }
            List<net.minecraft.world.scores.PlayerScoreEntry> entries = new ArrayList<>(sb.listPlayerScores(obj));
            entries.sort((a, b) -> Integer.compare(b.value(), a.value()));
            List<String> lines = new ArrayList<>();
            for (net.minecraft.world.scores.PlayerScoreEntry e : entries) {
                Component c = e.display();
                if (c == null) {
                    net.minecraft.world.scores.PlayerTeam team = sb.getPlayersTeam(e.owner());
                    c = net.minecraft.world.scores.PlayerTeam.formatNameForTeam(team, Component.literal(e.owner()));
                }
                String plain = c.getString();
                int rgb = firstColor(c);
                lines.add(rgb >= 0 ? "§" + codeOf(rgb) + plain : plain);
            }
            BwScore s = BwScore.parse(lines);
            st.bw = s.active ? s : null;
        } catch (Throwable t) {
            st.bw = null;
        }
    }

    private static int firstColor(Component c) {
        TextColor tc = c.getStyle().getColor();
        if (tc != null) return tc.getValue();
        for (Component sib : c.getSiblings()) {
            int r = firstColor(sib);
            if (r >= 0) return r;
        }
        return -1;
    }

    /** Nearest of the sixteen chat colours. */
    private static char codeOf(int rgb) {
        int[] cols = {0x000000, 0x0000AA, 0x00AA00, 0x00AAAA, 0xAA0000, 0xAA00AA, 0xFFAA00, 0xAAAAAA,
            0x555555, 0x5555FF, 0x55FF55, 0x55FFFF, 0xFF5555, 0xFF55FF, 0xFFFF55, 0xFFFFFF};
        char[] ch = "0123456789abcdef".toCharArray();
        int best = 15;
        long bd = Long.MAX_VALUE;
        for (int i = 0; i < cols.length; i++) {
            int dr = ((rgb >> 16) & 255) - ((cols[i] >> 16) & 255);
            int dg = ((rgb >> 8) & 255) - ((cols[i] >> 8) & 255);
            int db = (rgb & 255) - (cols[i] & 255);
            long d = (long) dr * dr + (long) dg * dg + (long) db * db;
            if (d < bd) {
                bd = d;
                best = i;
            }
        }
        return ch[best];
    }

    // --- primed TNT ---------------------------------------------------------------------------------
    private static void tnt(Minecraft mc, FrostState st) {
        if (!st.fx.tnt_timer) {
            st.tnts = java.util.Collections.emptyList();
            return;
        }
        Vec3 pos = mc.player.position();
        AABB box = new AABB(pos.x - 48, pos.y - 32, pos.z - 48, pos.x + 48, pos.y + 32, pos.z + 48);
        List<Entity> found = mc.level.getEntities((Entity) null, box, e -> e instanceof PrimedTnt);
        List<double[]> out = new ArrayList<>();
        for (Entity e : found) out.add(new double[] {e.getX(), e.getY() + 0.5, e.getZ(), ((PrimedTnt) e).getFuse() / 20.0});
        st.tnts = out;
    }

    // --- sound radar (inferred from what nearby players do) -------------------------------------------
    private static void radar(Minecraft mc, FrostState st) {
        double range = st.fx.radar_range;
        LocalPlayer me = mc.player;
        for (AbstractClientPlayer o : mc.level.players()) {
            if (o == me) continue;
            double d = o.distanceTo(me);
            if (d > range) continue;
            double mx = o.getX() - o.xo, mz = o.getZ() - o.zo;
            boolean moving = mx * mx + mz * mz > 0.0025;
            if (o.hurtTime > 0) st.radar.add("hurt", o.getX(), o.getY() + 1, o.getZ());
            else if (o.isUsingItem()) st.radar.add("potion", o.getX(), o.getY() + 1, o.getZ());
            else if (moving && o.onGround() && !o.isShiftKeyDown()) st.radar.add("step", o.getX(), o.getY() + 0.2, o.getZ());
        }
    }

    // --- clip maker ---------------------------------------------------------------------------------
    private static void clip(Minecraft mc, FrostState st) {
        if (captureBroken || capturing || !st.clip.wantFrame(st.fx)) return;
        capturing = true;
        try {
            final Models.Features f = st.fx;
            Method m = null;
            for (Method c : net.minecraft.client.Screenshot.class.getMethods()) {
                if (c.getName().equals("takeScreenshot") && c.getParameterCount() == 2
                    && java.lang.reflect.Modifier.isStatic(c.getModifiers())) m = c;
            }
            if (m == null) throw new NoSuchMethodException("takeScreenshot");
            Consumer<NativeImage> cb = img -> {
                try {
                    st.clip.submit(toImage(img, f.clip_scale), unscaled(f));
                } catch (Throwable t) {
                    captureBroken = true;
                } finally {
                    try {
                        img.close();
                    } catch (Throwable ignored) {
                    }
                    capturing = false;
                }
            };
            m.invoke(null, mc.getMainRenderTarget(), cb);
        } catch (Throwable t) {
            captureBroken = true;
            capturing = false;
        }
    }

    private static Models.Features unscaled(Models.Features f) {
        Models.Features c = new Models.Features();
        c.clip_fps = f.clip_fps;
        c.clip_quality = f.clip_quality;
        c.clip_seconds = f.clip_seconds;
        c.clip_scale = 100;
        return c;
    }

    /** Down-samples a screenshot while converting (nearest neighbour) so large windows stay cheap. */
    private static BufferedImage toImage(NativeImage img, int scalePct) throws Exception {
        int w = img.getWidth(), h = img.getHeight();
        int[] px = null;
        for (String n : new String[] {"getPixelsABGR", "getPixelsRGBA", "getPixels"}) {
            try {
                px = (int[]) NativeImage.class.getMethod(n).invoke(img);
                break;
            } catch (NoSuchMethodException ignored) {
            }
        }
        if (px == null) throw new NoSuchMethodException("getPixels");
        int ow = Math.max(16, w * Math.max(10, Math.min(100, scalePct)) / 100) & ~1;
        int oh = Math.max(16, h * Math.max(10, Math.min(100, scalePct)) / 100) & ~1;
        BufferedImage out = new BufferedImage(ow, oh, BufferedImage.TYPE_INT_RGB);
        for (int y = 0; y < oh; y++) {
            int sy = Math.min(h - 1, y * h / oh);
            for (int x = 0; x < ow; x++) {
                int a = px[sy * w + Math.min(w - 1, x * w / ow)];   // ABGR
                out.setRGB(x, y, ((a & 0xFF) << 16) | (a & 0xFF00) | ((a >> 16) & 0xFF));
            }
        }
        return out;
    }

    // --- hotkeys ------------------------------------------------------------------------------------
    private static boolean edge(int slot, boolean down) {
        boolean r = down && !KEY[slot];
        KEY[slot] = down;
        return r;
    }

    private static void hotkeys(Minecraft mc, final FrostState st) {
        long w = GLFW.glfwGetCurrentContext();
        if (w == 0L || !mc.mouseHandler.isMouseGrabbed() || mc.player == null) return;
        boolean alt = GLFW.glfwGetKey(w, GLFW.GLFW_KEY_LEFT_ALT) == GLFW.GLFW_PRESS;
        boolean saveKey = GLFW.glfwGetKey(w, GLFW.GLFW_KEY_F6) == GLFW.GLFW_PRESS
            || (alt && GLFW.glfwGetKey(w, GLFW.GLFW_KEY_C) == GLFW.GLFW_PRESS);
        if (edge(0, saveKey)) saveClip(mc, st);
        if (st.fx.spotify_enabled) {
            if (edge(1, GLFW.glfwGetKey(w, GLFW.GLFW_KEY_F7) == GLFW.GLFW_PRESS)) Spotify.previous(st);
            if (edge(2, GLFW.glfwGetKey(w, GLFW.GLFW_KEY_F8) == GLFW.GLFW_PRESS)) Spotify.playPause(st);
            if (edge(3, GLFW.glfwGetKey(w, GLFW.GLFW_KEY_F9) == GLFW.GLFW_PRESS)) Spotify.next(st);
        }
    }

    public static void saveClip(Minecraft mc, final FrostState st) {
        if (st.clip.saving) return;
        final File dir = new File(FrostMc.HOST.gameDir(), "frost-clips");
        st.clip.saving = true;
        Thread t = new Thread(() -> {
            try {
                File f = st.clip.save(dir);
                st.notify(f != null ? "Clip Saved Successfully!" : "Clip buffer is still empty");
            } catch (Throwable e) {
                st.notify("Clip save failed");
            } finally {
                st.clip.saving = false;
            }
        }, "frost-clip-save");
        t.setDaemon(true);
        t.start();
    }
}
