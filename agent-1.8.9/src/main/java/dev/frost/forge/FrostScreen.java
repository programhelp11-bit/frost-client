package dev.frost.forge;

import dev.frost.core.FrostState;
import dev.frost.core.Menu;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.FontRenderer;
import net.minecraft.client.gui.Gui;
import net.minecraft.client.gui.GuiScreen;
import org.lwjgl.input.Mouse;

/** In-game Frost menu for 1.8.9 (logic shared in core/Menu). */
public class FrostScreen extends GuiScreen {
    private final Menu menu = new Menu(FrostForge.HOST, FrostState.INSTANCE);

    @Override
    public void drawScreen(int mouseX, int mouseY, float partialTicks) {
        final FontRenderer font = Minecraft.getMinecraft().fontRendererObj;
        menu.render(new Menu.Canvas() {
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
        }, this.width, this.height, mouseX, mouseY, Mouse.isButtonDown(0));
    }

    @Override
    public boolean doesGuiPauseGame() {
        return false;
    }
}
