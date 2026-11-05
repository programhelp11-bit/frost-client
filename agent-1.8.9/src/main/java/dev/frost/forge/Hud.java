package dev.frost.forge;

import dev.frost.core.FrostState;
import dev.frost.core.Models;
import dev.frost.core.Util;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.FontRenderer;
import net.minecraft.client.gui.Gui;
import net.minecraft.client.renderer.GlStateManager;
import net.minecraft.client.renderer.RenderHelper;
import net.minecraft.item.ItemStack;

import java.util.ArrayList;
import java.util.List;
import java.util.Locale;

/** Flat blue Frost HUD panels with a tiny snowflake (same look as the Fabric mods). */
public final class Hud {
    private static final int TEXT = 0xFFEAF2FF;
    private static final int DIM = 0xFFA0AFC8;

    private static int lastW;
    private static int lastH;

    private Hud() {}

    public static void render(Minecraft mc) {
        if (mc.gameSettings.hideGUI || mc.thePlayer == null) return;
        FrostState st = FrostState.INSTANCE;
        Models.Stats s = Stats.latest;
        Models.Qol q = st.qol;
        FontRenderer font = mc.fontRendererObj;
        int cy = 6;
        net.minecraft.client.gui.ScaledResolution res = new net.minecraft.client.gui.ScaledResolution(mc);
        for (Models.Widget w : st.hud.widgets) {
            if (!w.enabled) continue;
            if (w.kind.equals("coordinates") && !q.show_coordinates && !q.show_biome) continue;
            boolean keys = w.kind.equals("keystrokes");
            float sc = Math.max(0.5f, Math.min(3f, w.scale));
            int px = keys ? res.getScaledWidth() - Math.round(70 * sc) : 6;
            int py = keys ? res.getScaledHeight() - Math.round(66 * sc) : cy;
            if (w.placed && w.pos != null && w.pos.length >= 2) {
                px = Math.round(w.pos[0]);
                py = Math.round(w.pos[1]);
            }
            GlStateManager.pushMatrix();
            GlStateManager.translate(px, py, 0f);
            GlStateManager.scale(sc, sc, 1f);
            lastW = 0;
            lastH = 0;
            draw(mc, font, w, s, q);
            if (keys) { lastW = 66; lastH = 58; }
            GlStateManager.popMatrix();
            st.bounds.put(w.kind, new int[] {px, py, Math.round(lastW * sc), Math.round(lastH * sc)});
            if (!keys && !w.placed) cy += Math.round(lastH * sc) + 5;
        }
        GlStateManager.enableBlend();
        GlStateManager.color(1f, 1f, 1f, 1f);
    }

    private static void draw(Minecraft mc, FontRenderer font, Models.Widget w, Models.Stats s, Models.Qol q) {
        int tint = (Math.max(110, Math.min(235, w.opacity + 60)) << 24) | 0x0A1A44;
        if (w.kind.equals("keystrokes")) {
            keystrokes(font, s.keys);
            return;
        }
        if (w.kind.equals("armor_status")) {
            armor(mc, font, tint);
            return;
        }
        List<String[]> rows = new ArrayList<String[]>();
        String k = w.kind;
        if (k.equals("cps")) rows.add(new String[] {"CPS", s.cps_left + " | " + s.cps_right});
        else if (k.equals("fps")) rows.add(new String[] {"FPS", String.valueOf(s.fps)});
        else if (k.equals("ping")) rows.add(new String[] {"Ping", s.ping_ms + " ms"});
        else if (k.equals("direction")) rows.add(new String[] {"Dir", Util.compass(s.yaw)
            + " " + Math.round(((s.yaw % 360f) + 360f) % 360f) + "°"});
        else if (k.equals("reach_display")) rows.add(new String[] {"Reach",
            s.last_reach == null ? "-" : String.format(Locale.ROOT, "%.2f m", s.last_reach)});
        else if (k.equals("coordinates")) {
            if (q.show_coordinates) {
                rows.add(new String[] {"XYZ", String.format(Locale.ROOT, "%.1f %.1f %.1f", s.pos[0], s.pos[1], s.pos[2])});
            }
            if (q.show_biome && !s.biome.isEmpty()) rows.add(new String[] {"Biome", s.biome});
        } else if (k.equals("bw_resources")) {
            rows.add(new String[] {"Iron", String.valueOf(s.bw_iron)});
            rows.add(new String[] {"Gold", String.valueOf(s.bw_gold)});
            rows.add(new String[] {"Diamond", String.valueOf(s.bw_diamond)});
            rows.add(new String[] {"Emerald", String.valueOf(s.bw_emerald)});
        } else if (k.equals("bw_blocks")) rows.add(new String[] {"Blocks", String.valueOf(s.bw_blocks)});
        else if (k.equals("bridge_speed")) {
            rows.add(new String[] {"Bridge", String.format(Locale.ROOT, "%.2f b/s", s.bridge_bps)});
            rows.add(new String[] {"Best", String.format(Locale.ROOT, "%.2f b/s", s.bridge_best)});
        } else if (k.equals("potion_status")) {
            if (s.potions.isEmpty()) rows.add(new String[] {"Effects", "none"});
            for (Models.Potion p : s.potions) {
                rows.add(new String[] {p.name, (p.remaining_secs / 60) + ":" + String.format(Locale.ROOT, "%02d", p.remaining_secs % 60)});
            }
        }
        if (rows.isEmpty()) return;
        int width = 0;
        for (String[] r : rows) width = Math.max(width, font.getStringWidth(r[0]) + 8 + font.getStringWidth(r[1]));
        int h = rows.size() * (font.FONT_HEIGHT + 3) + 5;
        panel(0, 0, width + 18, h + 2, tint);
        int y = 5;
        for (String[] r : rows) {
            font.drawStringWithShadow(r[0], 8, y + 1, 0xFF8FD3FF);
            font.drawStringWithShadow(r[1], 8 + font.getStringWidth(r[0]) + 8, y + 1, TEXT);
            y += font.FONT_HEIGHT + 3;
        }
    }

    private static void armor(Minecraft mc, FontRenderer font, int tint) {
        panel(0, 0, 68, 4 * 18 + 6, tint);
        for (int i = 0; i < 4; i++) {
            ItemStack st = mc.thePlayer.inventory.armorInventory[3 - i];
            int y = 3 + i * 18;
            if (st == null) {
                font.drawStringWithShadow("-", 13, y + 4, DIM);
                continue;
            }
            RenderHelper.enableGUIStandardItemLighting();
            mc.getRenderItem().renderItemAndEffectIntoGUI(st, 8, y);
            RenderHelper.disableStandardItemLighting();
            GlStateManager.disableLighting();
            if (st.isItemStackDamageable()) {
                int pct = Math.round(100f * (1f - (float) st.getItemDamage() / Math.max(1, st.getMaxDamage())));
                font.drawStringWithShadow(pct + "%", 28, y + 4, pct > 50 ? TEXT : (pct > 20 ? 0xFFFFC85A : 0xFFFF6464));
            }
        }
    }

    private static void keystrokes(FontRenderer font, Models.Keys k) {
        int cell = 20;
        int gap = 2;
        key(font, cell + gap, 0, cell, cell, "W", k.w);
        key(font, 0, cell + gap, cell, cell, "A", k.a);
        key(font, cell + gap, cell + gap, cell, cell, "S", k.s);
        key(font, 2 * (cell + gap), cell + gap, cell, cell, "D", k.d);
        int half = (3 * cell + 2 * gap - gap) / 2;
        key(font, 0, 2 * (cell + gap), half, 14, "LMB", k.lmb);
        key(font, half + gap, 2 * (cell + gap), half, 14, "RMB", k.rmb);
    }

    private static void key(FontRenderer font, int x, int y, int w, int h, String label, boolean down) {
        roundedFill(x, y + 1, w, h, 3, 0x48000000);
        roundedFill(x, y, w, h, 3, down ? 0xF08FD3FF : 0x468CBEFF);
        roundedFill(x + 1, y + 1, w - 2, h - 2, 2, down ? 0xF0A9E0FF : 0xC00E1F4A);
        if (!down) Gui.drawRect(x + 2, y + 1, x + w - 2, y + 2, 0x40BFE6FF);
        int labelColor = down ? 0xFF0B1B3A : TEXT;
        font.drawStringWithShadow(label, x + (w - font.getStringWidth(label)) / 2, y + (h - font.FONT_HEIGHT) / 2 + 1, labelColor);
    }

    private static void panel(int x, int y, int w, int h, int tint) {
        lastW = w;
        lastH = h;
        int a = (tint >>> 24) & 0xFF;
        roundedFill(x - 1, y - 1, w + 2, h + 2, 4, 0x1A2A56FF);
        roundedFill(x, y, w, h, 3, 0x408CBEFF);
        roundedFill(x + 1, y + 1, w - 2, h - 2, 2, (a << 24) | 0x0A1A44);
        Gui.drawRect(x + 2, y + 1, x + w - 2, y + 1 + Math.max(2, (h - 2) / 2), 0x1A3D6BFF);
        Gui.drawRect(x + 3, y + 1, x + w - 3, y + 2, 0x44DCF0FF);
    }

    private static void roundedFill(int x, int y, int w, int h, int r, int color) {
        Gui.drawRect(x, y + r, x + w, y + h - r, color);
        for (int i = 0; i < r; i++) {
            int inset = r - i;
            Gui.drawRect(x + inset, y + i, x + w - inset, y + i + 1, color);
            Gui.drawRect(x + inset, y + h - 1 - i, x + w - inset, y + h - i, color);
        }
    }

    private static void snowflake(int x, int y) {
        final int c = 0xB4BFE6FF;
        int[][] px = {{2,0},{2,1},{2,2},{2,3},{2,4},{0,2},{1,2},{3,2},{4,2},{0,0},{1,1},{3,3},{4,4},{4,0},{3,1},{1,3},{0,4}};
        for (int[] p : px) Gui.drawRect(x + p[0], y + p[1], x + p[0] + 1, y + p[1] + 1, c);
    }

    private static void border(int x, int y, int w, int h, int color) {
        Gui.drawRect(x, y, x + w, y + 1, color);
        Gui.drawRect(x, y + h - 1, x + w, y + h, color);
        Gui.drawRect(x, y + 1, x + 1, y + h - 1, color);
        Gui.drawRect(x + w - 1, y + 1, x + w, y + h - 1, color);
    }
}
