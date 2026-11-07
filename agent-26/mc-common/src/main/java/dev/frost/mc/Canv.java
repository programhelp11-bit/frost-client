package dev.frost.mc;

import dev.frost.core.Menu;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.Font;
import net.minecraft.client.gui.GuiGraphicsExtractor;

/** Wraps the game's drawing calls into the shared {@link Menu.Canvas} used by the Frost widgets. */
final class Canv {
    private Canv() {}

    static Menu.Canvas of(GuiGraphicsExtractor g) {
        final Font font = Minecraft.getInstance().font;
        final GuiGraphicsExtractor gg = g;
        return new Menu.Canvas() {
            public void fill(int x1, int y1, int x2, int y2, int argb) {
                gg.fill(x1, y1, x2, y2, argb);
            }

            public void text(String s, int x, int y, int argb) {
                gg.text(font, s, x, y, argb, false);
            }

            public int width(String s) {
                return font.width(s);
            }

            public int lineHeight() {
                return font.lineHeight;
            }
        };
    }
}
