package dev.frost.forge;

import dev.frost.core.FrostState;
import dev.frost.core.Menu;
import dev.frost.core.Models;
import dev.frost.core.Widgets;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.FontRenderer;
import net.minecraft.client.gui.Gui;
import net.minecraft.client.gui.ScaledResolution;
import net.minecraft.client.renderer.GlStateManager;
import net.minecraft.client.renderer.RenderHelper;
import net.minecraft.item.ItemStack;

/** Frost HUD for 1.8.9: the drawing itself lives in core/Widgets, shared with every other version. */
public final class Hud {
    private static final int TEXT = 0xFFEAF2FF;
    private static final int DIM = 0xFFA0AFC8;

    private Hud() {}

    static Menu.Canvas canvas(final FontRenderer font) {
        return new Menu.Canvas() {
            public void fill(int x1, int y1, int x2, int y2, int argb) {
                Gui.drawRect(x1, y1, x2, y2, argb);
            }

            public void text(String s, int x, int y, int argb) {
                font.drawStringWithShadow(s, x, y, argb);
            }

            public int width(String s) {
                return font.getStringWidth(s);
            }

            public int lineHeight() {
                return font.FONT_HEIGHT;
            }
        };
    }

    public static void render(Minecraft mc) {
        if (mc.gameSettings.hideGUI || mc.thePlayer == null) return;
        FrostState st = FrostState.INSTANCE;
        Models.Stats s = Stats.latest;
        FontRenderer font = mc.fontRendererObj;
        Menu.Canvas cv = canvas(font);
        ScaledResolution res = new ScaledResolution(mc);
        int gw = res.getScaledWidth();
        int gh = res.getScaledHeight();
        GlStateManager.enableBlend();
        GlStateManager.tryBlendFuncSeparate(770, 771, 1, 0);
        GlStateManager.disableLighting();
        int cy = 6;
        float boost = (float) (Math.round(Math.max(1.0, Math.min(2.0, 3.0 / Math.max(1, res.getScaleFactor()))) * 2.0) / 2.0);
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
            GlStateManager.pushMatrix();
            GlStateManager.translate(px, py, 0f);
            GlStateManager.scale(sc, sc, 1f);
            int[] wh = w.kind.equals("armor_status") ? armor(mc, font, cv, alpha) : Widgets.draw(w.kind, cv, st, s, alpha);
            GlStateManager.popMatrix();
            if (wh == null || wh[0] <= 0) {
                st.bounds.remove(w.kind);
                continue;
            }
            st.bounds.put(w.kind, new int[] {px, py, Math.round(wh[0] * sc), Math.round(wh[1] * sc)});
            if (!keys && !w.placed) cy += Math.round((wh[1] + 4) * sc) + 4;
        }
        if (!st.editorOpen) Widgets.overlays(cv, st);
        if (System.currentTimeMillis() < st.noticeUntil && !st.notice.isEmpty()) {
            int nw = font.getStringWidth(st.notice) + 24;
            int nx = (gw - nw) / 2;
            Widgets.chip(cv, nx, 24, nw, 20, 0xE8);
            font.drawStringWithShadow(st.notice, nx + 12, 30, 0xFFFFFFFF);
        }
        GlStateManager.enableBlend();
        GlStateManager.color(1f, 1f, 1f, 1f);
    }

    /** Custom crosshair; returns true when it replaced the vanilla one. */
    public static boolean crosshair(Minecraft mc) {
        Models.Features f = FrostState.INSTANCE.fx;
        if (f.crosshair.equals("vanilla")) return false;
        ScaledResolution res = new ScaledResolution(mc);
        GlStateManager.enableBlend();
        GlStateManager.disableTexture2D();
        Widgets.crosshair(canvas(mc.fontRendererObj), f, res.getScaledWidth() / 2, res.getScaledHeight() / 2);
        GlStateManager.enableTexture2D();
        GlStateManager.color(1f, 1f, 1f, 1f);
        return true;
    }

    private static boolean hidden(String kind, FrostState st) {
        if (st.editorOpen) return false;
        if (kind.equals("bw_timers") || kind.equals("bw_events") || kind.equals("bw_beds")) return st.bw == null;
        if (kind.equals("spotify")) return !st.fx.spotify_enabled;
        if (kind.equals("sound_radar")) return !st.fx.sound_radar;
        return false;
    }

    private static int[] armor(Minecraft mc, FontRenderer font, Menu.Canvas cv, int alpha) {
        ItemStack[] items = new ItemStack[5];
        int rows = 0;
        for (int i = 0; i < 4; i++) {
            items[i] = mc.thePlayer.inventory.armorInventory[3 - i];
            rows++;
        }
        items[4] = mc.thePlayer.getHeldItem();
        if (items[4] != null) rows++;
        int h = rows * 18 + 6;
        Widgets.chip(cv, 0, 0, 74, h, alpha);
        int y = 3;
        for (int i = 0; i < 5; i++) {
            ItemStack st = items[i];
            if (i == 4 && st == null) break;
            if (st == null) {
                font.drawStringWithShadow("-", 13, y + 4, DIM);
                y += 18;
                continue;
            }
            RenderHelper.enableGUIStandardItemLighting();
            mc.getRenderItem().renderItemAndEffectIntoGUI(st, 6, y);
            RenderHelper.disableStandardItemLighting();
            GlStateManager.disableLighting();
            GlStateManager.enableBlend();
            if (st.isItemStackDamageable()) {
                int pct = Math.round(100f * (1f - (float) st.getItemDamage() / Math.max(1, st.getMaxDamage())));
                int col = pct > 50 ? 0xFF7DF2A0 : (pct > 20 ? 0xFFFFC85A : 0xFFFF6464);
                font.drawStringWithShadow(pct + "%", 28, y + 4, col);
                Gui.drawRect(28, y + 14, 64, y + 15, 0x40FFFFFF);
                Gui.drawRect(28, y + 14, 28 + 36 * pct / 100, y + 15, col);
            } else if (st.stackSize > 1) {
                font.drawStringWithShadow("x" + st.stackSize, 28, y + 4, TEXT);
            }
            y += 18;
        }
        return new int[] {74, h};
    }
}
