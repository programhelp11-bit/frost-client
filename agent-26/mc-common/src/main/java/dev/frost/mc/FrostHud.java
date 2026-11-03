package dev.frost.mc;

import dev.frost.core.FrostState;
import dev.frost.core.Models;
import dev.frost.core.Util;
import net.minecraft.client.DeltaTracker;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.Font;
import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.world.entity.EquipmentSlot;
import net.minecraft.world.item.ItemStack;

import java.util.ArrayList;
import java.util.List;

/** Frost-style HUD renderer driven by the layout the launcher pushes (hud_manager.rs). */
public final class FrostHud {
    private static final int TEXT = 0xFFEAF2FF;
    private static final int DIM = 0xFFA0AFC8;
    private static final int ACCENT = 0xFF8FD3FF;

    private static int lastW;
    private static int lastH;

    private FrostHud() {}

    public static void render(GuiGraphicsExtractor g, DeltaTracker dt) {
        Minecraft mc = Minecraft.getInstance();
        StatsCollector.frame(mc);
        if (mc.player == null) return;

        FrostState st = FrostState.INSTANCE;
        Models.Stats s = StatsCollector.latest;
        Models.Qol q = st.qol;
        Font font = mc.font;

        for (Models.Widget w : st.hud.widgets) {
            if (!w.enabled) continue;
            if (w.kind.equals("coordinates") && !q.show_coordinates && !q.show_biome) continue;
            g.pose().pushMatrix();
            g.pose().translate(w.pos[0], w.pos[1]);
            g.pose().scale(w.scale, w.scale);
            lastW = 0;
            lastH = 0;
            draw(g, font, mc, w, s, q);
            if (w.kind.equals("keystrokes")) { lastW = 66; lastH = 58; }
            g.pose().popMatrix();
            st.bounds.put(w.kind, new int[] {Math.round(w.pos[0]), Math.round(w.pos[1]), Math.round(lastW * w.scale), Math.round(lastH * w.scale)});
        }
        if (System.currentTimeMillis() < st.noticeUntil && !st.notice.isEmpty()) {
            int nw = font.width(st.notice) + 16;
            int nx = (mc.getWindow().getGuiScaledWidth() - nw) / 2;
            g.pose().pushMatrix();
            panel(g, nx, 24, nw, 18, 0xC00E204E);
            g.text(font, st.notice, nx + 9, 29, TEXT, false);
            g.pose().popMatrix();
        }
    }

    private static void draw(GuiGraphicsExtractor g, Font font, Minecraft mc, Models.Widget w, Models.Stats s, Models.Qol q) {
        int tint = (Math.max(20, Math.min(240, w.opacity)) << 24) | 0x0E204E;
        switch (w.kind) {
            case "keystrokes" -> keystrokes(g, font, s.keys, tint);
            case "armor_status" -> armor(g, font, mc, tint);
            default -> {
                List<String[]> rows = new ArrayList<>();
                switch (w.kind) {
                    case "cps" -> rows.add(new String[] {"CPS", s.cps_left + " | " + s.cps_right});
                    case "fps" -> rows.add(new String[] {"FPS", String.valueOf(s.fps)});
                    case "ping" -> rows.add(new String[] {"Ping", s.ping_ms + " ms"});
                    case "direction" -> rows.add(new String[] {"Dir", Util.compass(s.yaw)
                        + " " + Math.round(((s.yaw % 360f) + 360f) % 360f) + "°"});
                    case "reach_display" -> rows.add(new String[] {"Reach",
                        s.last_reach == null ? "-" : String.format("%.2f m", s.last_reach)});
                    case "coordinates" -> {
                        if (q.show_coordinates) {
                            rows.add(new String[] {"XYZ", String.format("%.1f %.1f %.1f", s.pos[0], s.pos[1], s.pos[2])});
                        }
                        if (q.show_biome && !s.biome.isEmpty()) {
                            rows.add(new String[] {"Biome", prettyBiome(s.biome)});
                        }
                    }
                    case "bw_resources" -> {
                        rows.add(new String[] {"Iron", String.valueOf(s.bw_iron)});
                        rows.add(new String[] {"Gold", String.valueOf(s.bw_gold)});
                        rows.add(new String[] {"Diamond", String.valueOf(s.bw_diamond)});
                        rows.add(new String[] {"Emerald", String.valueOf(s.bw_emerald)});
                    }
                    case "bw_blocks" -> rows.add(new String[] {"Blocks", String.valueOf(s.bw_blocks)});
                    case "bridge_speed" -> {
                        rows.add(new String[] {"Bridge", String.format("%.2f b/s", s.bridge_bps)});
                        rows.add(new String[] {"Best", String.format("%.2f b/s", s.bridge_best)});
                    }
                    case "potion_status" -> {
                        if (s.potions.isEmpty()) rows.add(new String[] {"Effects", "none"});
                        for (Models.Potion p : s.potions) {
                            rows.add(new String[] {p.name, (p.remaining_secs / 60) + ":" + String.format("%02d", p.remaining_secs % 60)});
                        }
                    }
                    default -> { }
                }
                if (rows.isEmpty()) return;
                int width = 0;
                for (String[] r : rows) width = Math.max(width, font.width(r[0]) + 8 + font.width(r[1]));
                int h = rows.size() * (font.lineHeight + 3) + 5;
                panel(g, 0, 0, width + 22, h, tint);
                int y = 5;
                for (String[] r : rows) {
                    g.text(font, r[0], 9, y, DIM, false);
                    g.text(font, r[1], 9 + font.width(r[0]) + 8, y, TEXT, false);
                    y += font.lineHeight + 3;
                }
            }
        }
    }

    private static String prettyBiome(String registered) {
        String name = registered.contains(":") ? registered.substring(registered.indexOf(':') + 1) : registered;
        StringBuilder sb = new StringBuilder();
        for (String part : name.split("_")) {
            if (part.isEmpty()) continue;
            if (sb.length() > 0) sb.append(' ');
            sb.append(Character.toUpperCase(part.charAt(0))).append(part.substring(1));
        }
        return sb.toString();
    }

    private static void armor(GuiGraphicsExtractor g, Font font, Minecraft mc, int tint) {
        EquipmentSlot[] slots = {EquipmentSlot.HEAD, EquipmentSlot.CHEST, EquipmentSlot.LEGS, EquipmentSlot.FEET};
        panel(g, 0, 0, 68, 4 * 18 + 6, tint);
        for (int i = 0; i < slots.length; i++) {
            ItemStack st = mc.player.getItemBySlot(slots[i]);
            int y = 3 + i * 18;
            if (st.isEmpty()) {
                g.text(font, "-", 13, y + 4, DIM, false);
                continue;
            }
            g.item(st, 8, y);
            if (st.isDamageableItem()) {
                int pct = Math.round(100f * (1f - (float) st.getDamageValue() / Math.max(1, st.getMaxDamage())));
                g.text(font, pct + "%", 28, y + 4, pct > 50 ? TEXT : (pct > 20 ? 0xFFFFC85A : 0xFFFF6464), false);
            }
        }
    }

    private static void keystrokes(GuiGraphicsExtractor g, Font font, Models.Keys k, int tint) {
        int cell = 20;
        int gap = 2;
        key(g, font, cell + gap, 0, cell, cell, "W", k.w);
        key(g, font, 0, cell + gap, cell, cell, "A", k.a);
        key(g, font, cell + gap, cell + gap, cell, cell, "S", k.s);
        key(g, font, 2 * (cell + gap), cell + gap, cell, cell, "D", k.d);
        int half = (3 * cell + 2 * gap - gap) / 2;
        key(g, font, 0, 2 * (cell + gap), half, 14, "LMB", k.lmb);
        key(g, font, half + gap, 2 * (cell + gap), half, 14, "RMB", k.rmb);
    }

    private static void key(GuiGraphicsExtractor g, Font font, int x, int y, int w, int h, String label, boolean down) {
        roundedFill(g, x, y + 1, w, h, 2, 0x50000000);
        roundedFill(g, x, y, w, h, 2, down ? 0xE08FD3FF : 0xA0122A5C);
        if (!down) g.fill(x + 2, y, x + w - 2, y + 1, 0x40BFE6FF);
        int labelColor = down ? 0xFF0B1B3A : TEXT;
        g.text(font, label, x + (w - font.width(label)) / 2, y + (h - font.lineHeight) / 2 + 1, labelColor, false);
    }

    /** Frost panel: soft shadow, rounded body, glossy top edge, accent bar and a tiny snowflake. */
    private static void panel(GuiGraphicsExtractor g, int x, int y, int w, int h, int tint) {
        lastW = w;
        lastH = h;
        roundedFill(g, x, y + 1, w, h, 3, 0x48000000);              // drop shadow
        roundedFill(g, x, y, w, h, 3, tint);                         // body
        g.fill(x + 2, y + 1, x + w - 2, y + 3, 0x16FFFFFF);          // gloss on the top edge
        g.fill(x + 3, y, x + w - 3, y + 1, 0x70BFE6FF);              // bright top line
        g.fill(x + 3, y + h - 1, x + w - 3, y + h, 0x40000000);      // darker bottom line
        g.fill(x + 3, y + 4, x + 5, y + h - 4, ACCENT);              // accent bar
        if (w >= 34) snowflake(g, x + w - 9, y + 3);
    }

    /** 5x5 pixel snowflake, drawn with fills so it does not depend on font glyphs. */
    private static void snowflake(GuiGraphicsExtractor g, int x, int y) {
        final int c = 0xB4BFE6FF;
        int[][] px = {{2,0},{2,1},{2,2},{2,3},{2,4},{0,2},{1,2},{3,2},{4,2},{0,0},{1,1},{3,3},{4,4},{4,0},{3,1},{1,3},{0,4}};
        for (int[] p : px) g.fill(x + p[0], y + p[1], x + p[0] + 1, y + p[1] + 1, c);
    }

    private static void border(GuiGraphicsExtractor g, int x, int y, int w, int h, int color) {
        g.fill(x, y, x + w, y + 1, color);
        g.fill(x, y + h - 1, x + w, y + h, color);
        g.fill(x, y + 1, x + 1, y + h - 1, color);
        g.fill(x + w - 1, y + 1, x + w, y + h - 1, color);
    }

    private static void roundedFill(GuiGraphicsExtractor g, int x, int y, int w, int h, int r, int color) {
        g.fill(x, y + r, x + w, y + h - r, color);
        for (int i = 0; i < r; i++) {
            int inset = r - i;
            g.fill(x + inset, y + i, x + w - inset, y + i + 1, color);
            g.fill(x + inset, y + h - 1 - i, x + w - inset, y + h - i, color);
        }
    }
}
