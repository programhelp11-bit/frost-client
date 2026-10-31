package dev.frost.mc;

import dev.frost.core.FrostState;
import dev.frost.core.Models;
import dev.frost.core.Util;
import net.minecraft.client.DeltaTracker;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.Font;
import net.minecraft.client.gui.GuiGraphics;
import net.minecraft.world.entity.EquipmentSlot;
import net.minecraft.world.item.ItemStack;

import java.util.ArrayList;
import java.util.List;

/** Liquid-glass HUD renderer driven by the layout the launcher pushes (hud_manager.rs). */
public final class FrostHud {
    private static final int TEXT = 0xFFEAF2FF;
    private static final int DIM = 0xFFA0AFC8;
    private static final int ACCENT = 0xFF8FD3FF;

    private FrostHud() {}

    public static void render(GuiGraphics g, DeltaTracker dt) {
        Minecraft mc = Minecraft.getInstance();
        StatsCollector.frame(mc);
        if (mc.options.hideGui || mc.player == null) return;

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
            draw(g, font, mc, w, s, q);
            g.pose().popMatrix();
        }
    }

    private static void draw(GuiGraphics g, Font font, Minecraft mc, Models.Widget w, Models.Stats s, Models.Qol q) {
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
                for (String[] r : rows) width = Math.max(width, font.width(r[0]) + 6 + font.width(r[1]));
                int h = rows.size() * (font.lineHeight + 2) + 6;
                panel(g, 0, 0, width + 12, h, tint);
                int y = 4;
                for (String[] r : rows) {
                    g.drawString(font, r[0], 6, y, DIM);
                    g.drawString(font, r[1], 6 + font.width(r[0]) + 6, y, TEXT);
                    y += font.lineHeight + 2;
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

    private static void armor(GuiGraphics g, Font font, Minecraft mc, int tint) {
        EquipmentSlot[] slots = {EquipmentSlot.HEAD, EquipmentSlot.CHEST, EquipmentSlot.LEGS, EquipmentSlot.FEET};
        panel(g, 0, 0, 62, 4 * 18 + 6, tint);
        for (int i = 0; i < slots.length; i++) {
            ItemStack st = mc.player.getItemBySlot(slots[i]);
            int y = 3 + i * 18;
            if (st.isEmpty()) {
                g.drawString(font, "-", 8, y + 4, DIM);
                continue;
            }
            g.renderItem(st, 4, y);
            if (st.isDamageableItem()) {
                int pct = Math.round(100f * (1f - (float) st.getDamageValue() / Math.max(1, st.getMaxDamage())));
                g.drawString(font, pct + "%", 24, y + 4, pct > 50 ? TEXT : (pct > 20 ? 0xFFFFC85A : 0xFFFF6464));
            }
        }
    }

    private static void keystrokes(GuiGraphics g, Font font, Models.Keys k, int tint) {
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

    private static void key(GuiGraphics g, Font font, int x, int y, int w, int h, String label, boolean down) {
        int fill = down ? 0xB48FD3FF : 0x660E204E;
        g.fill(x, y, x + w, y + h, fill);
        border(g, x, y, w, h, down ? 0xFFBFE6FF : 0x906EAAFF);
        g.drawString(font, label, x + (w - font.width(label)) / 2, y + (h - font.lineHeight) / 2 + 1, TEXT);
    }

    /** Flat frost panel: blue tint, 1px blue border and a tiny snowflake in the corner. */
    private static void panel(GuiGraphics g, int x, int y, int w, int h, int tint) {
        g.fill(x, y, x + w, y + h, tint);
        border(g, x, y, w, h, 0x8C6EAAFF);
        if (w >= 26) snowflake(g, x + w - 8, y + 2);
    }

    /** 5x5 pixel snowflake, drawn with fills so it does not depend on font glyphs. */
    private static void snowflake(GuiGraphics g, int x, int y) {
        final int c = 0xB4BFE6FF;
        int[][] px = {{2,0},{2,1},{2,2},{2,3},{2,4},{0,2},{1,2},{3,2},{4,2},{0,0},{1,1},{3,3},{4,4},{4,0},{3,1},{1,3},{0,4}};
        for (int[] p : px) g.fill(x + p[0], y + p[1], x + p[0] + 1, y + p[1] + 1, c);
    }

    private static void border(GuiGraphics g, int x, int y, int w, int h, int color) {
        g.fill(x, y, x + w, y + 1, color);
        g.fill(x, y + h - 1, x + w, y + h, color);
        g.fill(x, y + 1, x + 1, y + h - 1, color);
        g.fill(x + w - 1, y + 1, x + w, y + h - 1, color);
    }

    private static void roundedFill(GuiGraphics g, int x, int y, int w, int h, int r, int color) {
        g.fill(x, y + r, x + w, y + h - r, color);
        for (int i = 0; i < r; i++) {
            int inset = r - i;
            g.fill(x + inset, y + i, x + w - inset, y + i + 1, color);
            g.fill(x + inset, y + h - 1 - i, x + w - inset, y + h - i, color);
        }
    }
}
