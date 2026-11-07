package dev.frost.forge;

import dev.frost.core.BwScore;
import dev.frost.core.FrostState;
import dev.frost.core.Models;
import dev.frost.core.Spotify;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.ScaledResolution;
import net.minecraft.client.shader.Framebuffer;
import net.minecraft.entity.Entity;
import net.minecraft.entity.item.EntityTNTPrimed;
import net.minecraft.entity.player.EntityPlayer;
import net.minecraft.scoreboard.Score;
import net.minecraft.scoreboard.ScoreObjective;
import net.minecraft.scoreboard.ScorePlayerTeam;
import net.minecraft.scoreboard.Scoreboard;
import net.minecraft.util.Vec3;
import org.lwjgl.input.Keyboard;

import java.awt.image.BufferedImage;
import java.io.File;
import java.util.ArrayList;
import java.util.Collection;
import java.util.List;

/** 1.8.9 glue for the shared Frost features: camera, BedWars scoreboard, TNT, sound radar, clips, hotkeys. */
public final class Features {
    private static int ticks;
    private static boolean spotifyStarted;
    private static boolean captureBroken;
    private static final boolean[] KEY = new boolean[8];

    private Features() {}

    public static void updateView(Minecraft mc, float pt) {
        if (mc.thePlayer == null) return;
        Vec3 eye = mc.thePlayer.getPositionEyes(pt);
        Models.View v = FrostState.INSTANCE.view;
        v.x = eye.xCoord;
        v.y = eye.yCoord;
        v.z = eye.zCoord;
        v.yaw = mc.thePlayer.prevRotationYaw + (mc.thePlayer.rotationYaw - mc.thePlayer.prevRotationYaw) * pt;
        v.pitch = mc.thePlayer.prevRotationPitch + (mc.thePlayer.rotationPitch - mc.thePlayer.prevRotationPitch) * pt;
        v.fov = Math.round(mc.gameSettings.fovSetting);
        ScaledResolution r = new ScaledResolution(mc);
        v.w = r.getScaledWidth();
        v.h = r.getScaledHeight();
        v.valid = true;
    }

    public static void tick(Minecraft mc) {
        FrostState st = FrostState.INSTANCE;
        ticks++;
        if (!spotifyStarted) {
            spotifyStarted = true;
            Spotify.start(st, new Runnable() {
                public void run() {
                    FrostForge.HOST.settingsChanged();
                }
            });
        }
        hotkeys(mc, st);
        if (mc.thePlayer == null || mc.theWorld == null) {
            st.view.valid = false;
            return;
        }
        try {
            int limit = st.fx.particle_limit;
            if (limit < 100 && !st.qol.fps_boost && !st.qol.chunk_optimizer) {
                mc.gameSettings.particleSetting = limit >= 67 ? 0 : (limit >= 34 ? 1 : 2);
            }
            if (ticks % 10 == 0) scoreboard(mc, st);
            tnt(mc, st);
            if (st.fx.sound_radar && ticks % 2 == 0) radar(mc, st);
        } catch (Throwable t) {
            // never take the game down
        }
    }

    private static void scoreboard(Minecraft mc, FrostState st) {
        try {
            Scoreboard sb = mc.theWorld.getScoreboard();
            ScoreObjective obj = sb.getObjectiveInDisplaySlot(1);
            if (obj == null) {
                st.bw = null;
                return;
            }
            Collection<Score> scores = sb.getSortedScores(obj);
            List<String> lines = new ArrayList<String>();
            for (Score sc : scores) {
                String name = sc.getPlayerName();
                if (name == null || name.startsWith("#")) continue;
                lines.add(ScorePlayerTeam.formatPlayerName(sb.getPlayersTeam(name), name));
            }
            BwScore b = BwScore.parse(lines);
            st.bw = b.active ? b : null;
        } catch (Throwable t) {
            st.bw = null;
        }
    }

    private static void tnt(Minecraft mc, FrostState st) {
        if (!st.fx.tnt_timer) {
            st.tnts = java.util.Collections.emptyList();
            return;
        }
        List<double[]> out = new ArrayList<double[]>();
        for (Object o : mc.theWorld.loadedEntityList) {
            if (!(o instanceof EntityTNTPrimed)) continue;
            Entity e = (Entity) o;
            if (e.getDistanceToEntity(mc.thePlayer) > 64) continue;
            out.add(new double[] {e.posX, e.posY + 0.5, e.posZ, ((EntityTNTPrimed) e).fuse / 20.0});
        }
        st.tnts = out;
    }

    private static void radar(Minecraft mc, FrostState st) {
        double range = st.fx.radar_range;
        for (Object o : mc.theWorld.playerEntities) {
            EntityPlayer p = (EntityPlayer) o;
            if (p == mc.thePlayer || p.getDistanceToEntity(mc.thePlayer) > range) continue;
            double mx = p.posX - p.prevPosX, mz = p.posZ - p.prevPosZ;
            boolean moving = mx * mx + mz * mz > 0.0025;
            if (p.hurtTime > 0) st.radar.add("hurt", p.posX, p.posY + 1, p.posZ);
            else if (p.isUsingItem()) st.radar.add("potion", p.posX, p.posY + 1, p.posZ);
            else if (moving && p.onGround && !p.isSneaking()) st.radar.add("step", p.posX, p.posY + 0.2, p.posZ);
        }
    }

    /** Called at the end of every rendered frame: hands a frame to the clip buffer when it is due. */
    public static void capture(Minecraft mc) {
        FrostState st = FrostState.INSTANCE;
        if (captureBroken || mc.thePlayer == null || !st.clip.wantFrame(st.fx)) return;
        try {
            Framebuffer fb = mc.getFramebuffer();
            int w = mc.displayWidth, h = mc.displayHeight;
            org.lwjgl.opengl.GL11.glPixelStorei(org.lwjgl.opengl.GL11.GL_PACK_ALIGNMENT, 1);
            org.lwjgl.opengl.GL11.glPixelStorei(org.lwjgl.opengl.GL11.GL_UNPACK_ALIGNMENT, 1);
            if (net.minecraft.client.renderer.OpenGlHelper.isFramebufferEnabled()) {
                w = fb.framebufferTextureWidth;
                h = fb.framebufferTextureHeight;
            }
            java.nio.IntBuffer buf = org.lwjgl.BufferUtils.createIntBuffer(w * h);
            if (net.minecraft.client.renderer.OpenGlHelper.isFramebufferEnabled()) {
                net.minecraft.client.renderer.GlStateManager.bindTexture(fb.framebufferTexture);
                org.lwjgl.opengl.GL11.glGetTexImage(org.lwjgl.opengl.GL11.GL_TEXTURE_2D, 0,
                    org.lwjgl.opengl.GL12.GL_BGRA, org.lwjgl.opengl.GL12.GL_UNSIGNED_INT_8_8_8_8_REV, buf);
            } else {
                org.lwjgl.opengl.GL11.glReadPixels(0, 0, w, h,
                    org.lwjgl.opengl.GL12.GL_BGRA, org.lwjgl.opengl.GL12.GL_UNSIGNED_INT_8_8_8_8_REV, buf);
            }
            int[] px = new int[w * h];
            buf.get(px);
            int vw = Math.min(w, mc.displayWidth), vh = Math.min(h, mc.displayHeight);
            int pct = Math.max(10, Math.min(100, st.fx.clip_scale));
            int ow = Math.max(16, vw * pct / 100) & ~1;
            int oh = Math.max(16, vh * pct / 100) & ~1;
            BufferedImage out = new BufferedImage(ow, oh, BufferedImage.TYPE_INT_RGB);
            for (int y = 0; y < oh; y++) {
                int sy = h - 1 - Math.min(vh - 1, y * vh / oh);   // GL rows start at the bottom
                for (int x = 0; x < ow; x++) out.setRGB(x, y, px[sy * w + Math.min(vw - 1, x * vw / ow)] & 0xFFFFFF);
            }
            Models.Features f = new Models.Features();
            f.clip_fps = st.fx.clip_fps;
            f.clip_quality = st.fx.clip_quality;
            f.clip_seconds = st.fx.clip_seconds;
            f.clip_scale = 100;
            st.clip.submit(out, f);
        } catch (Throwable t) {
            captureBroken = true;
        }
    }

    private static boolean edge(int slot, boolean down) {
        boolean r = down && !KEY[slot];
        KEY[slot] = down;
        return r;
    }

    private static void hotkeys(Minecraft mc, FrostState st) {
        if (mc.currentScreen != null || mc.thePlayer == null) return;
        boolean save = Keyboard.isKeyDown(Keyboard.KEY_F6)
            || (Keyboard.isKeyDown(Keyboard.KEY_LMENU) && Keyboard.isKeyDown(Keyboard.KEY_C));
        if (edge(0, save)) saveClip(st);
        if (st.fx.spotify_enabled) {
            if (edge(1, Keyboard.isKeyDown(Keyboard.KEY_F7))) Spotify.previous(st);
            if (edge(2, Keyboard.isKeyDown(Keyboard.KEY_F8))) Spotify.playPause(st);
            if (edge(3, Keyboard.isKeyDown(Keyboard.KEY_F9))) Spotify.next(st);
        }
    }

    public static void saveClip(final FrostState st) {
        if (st.clip.saving) return;
        final File dir = new File(FrostForge.HOST.gameDir(), "frost-clips");
        st.clip.saving = true;
        Thread t = new Thread(new Runnable() {
            public void run() {
                try {
                    File f = st.clip.save(dir);
                    st.notify(f != null ? "Clip Saved Successfully!" : "Clip buffer is still empty");
                } catch (Throwable e) {
                    st.notify("Clip save failed");
                } finally {
                    st.clip.saving = false;
                }
            }
        }, "frost-clip-save");
        t.setDaemon(true);
        t.start();
    }
}
