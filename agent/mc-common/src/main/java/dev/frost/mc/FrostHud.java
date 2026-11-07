package dev.frost.mc;

import dev.frost.core.FrostState;
import dev.frost.core.Menu;
import dev.frost.core.Models;
import dev.frost.core.Widgets;
import net.fabricmc.fabric.api.client.rendering.v1.hud.HudElement;
import net.minecraft.client.DeltaTracker;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.Font;
import net.minecraft.client.gui.GuiGraphics;
import net.minecraft.world.entity.EquipmentSlot;
import net.minecraft.world.item.ItemStack;

/** Frost HUD: lays out the widgets (auto stack or the positions the player set in the HUD editor) and draws them. */
public final class FrostHud {
    private static final int TEXT = 0xFFEAF2FF;
    private static final int DIM = 0xFFA0AFC8;

    private FrostHud() {}

    public static void render(GuiGraphics g, DeltaTracker dt) {
        Minecraft mc = Minecraft.getInstance();
        StatsCollector.frame(mc);
        if (mc.options.hideGui || mc.player == null) return;

        FrostState st = FrostState.INSTANCE;
        Models.Stats s = StatsCollector.latest;
        Menu.Canvas cv = Canv.of(g);
        Features.updateView(mc, dt);

        // Auto layout: widgets stack down the left edge without overlapping, keystrokes sit bottom right.
        int cy = 6;
        double gui = Math.max(1.0, mc.getWindow().getGuiScale());
        float boost = (float) (Math.round(Math.max(1.0, Math.min(2.0, 3.0 / gui)) * 2.0) / 2.0);   // small GUI scales: bigger HUD text
        int gw = mc.getWindow().getGuiScaledWidth();
        int gh = mc.getWindow().getGuiScaledHeight();
        for (Models.Widget w : st.hud.widgets) {
            if (!w.enabled || hidden(w.kind, st)) {
                st.bounds.remove(w.kind);
                continue;
            }
            boolean keys = w.kind.equals("keystrokes");
            float sc = Math.max(0.5f, Math.min(3f, w.scale)) * boost;
            int px = keys ? gw - Math.round(68 * sc) : 6;
            int py = keys ? gh - Math.round(92 * sc) : cy;
            if (w.placed && w.pos != null && w.pos.length >= 2) {
                px = Math.round(w.pos[0]);
                py = Math.round(w.pos[1]);
            }
            int alpha = Math.max(0xC8, Math.min(0xF0, w.opacity + 90));
            g.pose().pushMatrix();
            g.pose().translate(px, py);
            g.pose().scale(sc, sc);
            int[] wh = w.kind.equals("armor_status") ? armor(g, cv, mc, alpha) : Widgets.draw(w.kind, cv, st, s, alpha);
            g.pose().popMatrix();
            if (wh == null || wh[0] <= 0) {
                st.bounds.remove(w.kind);
                continue;
            }
            st.bounds.put(w.kind, new int[] {px, py, Math.round(wh[0] * sc), Math.round(wh[1] * sc)});
            if (!keys && !w.placed) cy += Math.round((wh[1] + 4) * sc) + 4;
        }
        if (!st.editorOpen) Widgets.overlays(cv, st);
        if (System.currentTimeMillis() < st.noticeUntil && !st.notice.isEmpty()) {
            Font font = mc.font;
            int nw = font.width(st.notice) + 24;
            int nx = (gw - nw) / 2;
            g.pose().pushMatrix();
            Widgets.chip(cv, nx, 24, nw, 20, 0xE8);
            g.drawString(font, st.notice, nx + 12, 30, 0xFFFFFFFF);
            g.pose().popMatrix();
        }
    }

    /** BedWars widgets only show during a BedWars game (or while the HUD editor is open). */
    private static boolean hidden(String kind, FrostState st) {
        if (st.editorOpen) return false;
        if (kind.startsWith("bw_") && (kind.equals("bw_timers") || kind.equals("bw_events") || kind.equals("bw_beds"))) return st.bw == null;
        if (kind.equals("spotify")) return !st.fx.spotify_enabled;
        if (kind.equals("sound_radar")) return !st.fx.sound_radar;
        return false;
    }

    /** Custom crosshair; falls back to the vanilla element when the style is "vanilla". */
    public static void crosshair(GuiGraphics g, DeltaTracker dt, HudElement vanilla) {
        Minecraft mc = Minecraft.getInstance();
        Models.Features f = FrostState.INSTANCE.fx;
        if (f.crosshair.equals("vanilla") || mc.options.hideGui) {
            vanilla.render(g, dt);
            return;
        }
        Widgets.crosshair(Canv.of(g), f, mc.getWindow().getGuiScaledWidth() / 2, mc.getWindow().getGuiScaledHeight() / 2);
    }

    private static int[] armor(GuiGraphics g, Menu.Canvas cv, Minecraft mc, int alpha) {
        EquipmentSlot[] slots = {EquipmentSlot.HEAD, EquipmentSlot.CHEST, EquipmentSlot.LEGS, EquipmentSlot.FEET, EquipmentSlot.MAINHAND};
        int rows = 0;
        ItemStack[] items = new ItemStack[slots.length];
        for (int i = 0; i < slots.length; i++) {
            items[i] = mc.player.getItemBySlot(slots[i]);
            if (i < 4 || !items[i].isEmpty()) rows++;
        }
        int h = rows * 18 + 6;
        Widgets.chip(cv, 0, 0, 74, h, alpha);
        Font font = mc.font;
        int y = 3;
        for (int i = 0; i < slots.length; i++) {
            ItemStack st = items[i];
            if (i == 4 && st.isEmpty()) break;
            if (st.isEmpty()) {
                g.drawString(font, "-", 13, y + 4, DIM);
                y += 18;
                continue;
            }
            g.renderItem(st, 6, y);
            if (st.isDamageableItem()) {
                int pct = Math.round(100f * (1f - (float) st.getDamageValue() / Math.max(1, st.getMaxDamage())));
                int col = pct > 50 ? 0xFF7DF2A0 : (pct > 20 ? 0xFFFFC85A : 0xFFFF6464);
                g.drawString(font, pct + "%", 28, y + 4, col);
                int bw = 36 * pct / 100;
                g.fill(28, y + 14, 28 + 36, y + 15, 0x40FFFFFF);
                g.fill(28, y + 14, 28 + bw, y + 15, col);
            } else if (st.getCount() > 1) {
                g.drawString(font, "x" + st.getCount(), 28, y + 4, TEXT);
            }
            y += 18;
        }
        return new int[] {74, h};
    }
}
